use super::*;
use crate::{
    filters::{compile, Clock},
    rules::OriginFilter,
    test_support::{database, Scratch},
};
use std::time::Duration;

/// An origin, a destination and the database of one rule.
struct Bench {
    origin: Scratch,
    destination: Scratch,
    db: SqlitePool,
    cancel: Arc<AtomicBool>,
}

impl Bench {
    async fn new() -> Self {
        let db = database().await;
        sqlx::query(
            "INSERT INTO backup_rules (id, rule_type, created_at, name) VALUES ('rule', 'scheduled', 'x', 'Rule')",
        )
        .execute(&db)
        .await
        .expect("rule");
        Self {
            origin: Scratch::new("sync-origin"),
            destination: Scratch::new("sync-destination"),
            db,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    /// One run on the whole origin, and what it did.
    async fn run(&self) -> (Result<(), Stop>, Progress) {
        self.run_with(None, true).await
    }

    async fn run_with(
        &self,
        rename: Option<&BatchRenameConfiguration>,
        mirror: bool,
    ) -> (Result<(), Stop>, Progress) {
        let filters = Filters::default();
        let progress = Progress::default();
        let outcome = {
            let mut sync = self.sync(&filters, &progress);
            sync.rename = rename;
            sync.mirror = mirror;
            sync.tree("").await
        };
        (outcome, progress)
    }

    fn sync<'a>(&'a self, filters: &'a Filters, progress: &'a Progress) -> Sync<'a> {
        Sync {
            db: &self.db,
            rule_id: "rule",
            origin: self.origin.path(),
            destination: self.destination.path(),
            filters,
            rename: None,
            mirror: true,
            resolution: DuplicateResolution::Overwrite,
            verify: true,
            cancel: &self.cancel,
            progress,
            trash: None,
            removable: u64::MAX,
        }
    }

    fn copy(&self, relative: &str) -> Option<String> {
        fs::read_to_string(self.destination.path().join(relative)).ok()
    }

    async fn records(&self) -> Vec<(String, String, String)> {
        sqlx::query_as("SELECT parent, name, destination FROM rule_files ORDER BY parent, name")
            .fetch_all(&self.db)
            .await
            .expect("records")
    }
}

fn copied(progress: &Progress) -> u64 {
    progress.copied.load(Ordering::Relaxed)
}

#[tokio::test]
async fn copies_the_origin_once_and_keeps_its_times() {
    let bench = Bench::new().await;
    let photo = bench.origin.write("Photos/2026/a.jpg", "a");
    bench.origin.write("notes.txt", "notes");
    bench.origin.write(".nas-backup-123.tmp", "half");
    bench.origin.write(".nas-trash/old.jpg", "deleted");
    std::os::unix::fs::symlink("/etc/hostname", bench.origin.path().join("link")).expect("link");

    let (outcome, progress) = bench.run().await;
    assert_eq!(outcome, Ok(()));
    assert_eq!(copied(&progress), 2);
    assert_eq!(bench.copy("Photos/2026/a.jpg").as_deref(), Some("a"));
    assert_eq!(bench.copy("notes.txt").as_deref(), Some("notes"));
    assert_eq!(bench.copy(".nas-backup-123.tmp"), None);
    assert_eq!(bench.copy(".nas-trash/old.jpg"), None);
    assert_eq!(bench.copy("link"), None);
    let original = fs::metadata(&photo)
        .expect("original")
        .modified()
        .expect("time");
    let copy = fs::metadata(bench.destination.path().join("Photos/2026/a.jpg"))
        .expect("copy")
        .modified()
        .expect("time");
    assert_eq!(original, copy);
    assert_eq!(
        bench.records().await,
        [
            (
                "".to_owned(),
                "notes.txt".to_owned(),
                "notes.txt".to_owned()
            ),
            (
                "Photos/2026".to_owned(),
                "a.jpg".to_owned(),
                "Photos/2026/a.jpg".to_owned()
            ),
        ]
    );

    // Nothing changed: nothing is copied again.
    let (outcome, progress) = bench.run().await;
    assert_eq!(outcome, Ok(()));
    assert_eq!(copied(&progress), 0);
}

#[tokio::test]
async fn a_mirror_removes_only_the_copies_whose_original_is_gone() {
    let bench = Bench::new().await;
    bench.origin.write("keep.txt", "keep");
    bench.origin.write("old/gone.txt", "gone");
    bench.origin.write("filtered.log", "log");
    bench.destination.write("theirs.txt", "not the rule's");
    assert_eq!(bench.run().await.0, Ok(()));
    assert_eq!(bench.copy("filtered.log").as_deref(), Some("log"));

    fs::remove_dir_all(bench.origin.path().join("old")).expect("gone");
    std::thread::sleep(Duration::from_millis(20));
    bench.origin.write("keep.txt", "changed");
    let filters = compile(
        &[OriginFilter {
            id: 1,
            action: "exclude".to_owned(),
            file_type: "*".to_owned(),
            status: "name".to_owned(),
            operator: None,
            value: "*.log".to_owned(),
        }],
        &Clock {
            today: 0,
            yesterday: 0,
            last_week: 0,
            last_month: 0,
            this_month: 0,
        },
    )
    .expect("filters");
    let progress = Progress::default();
    let outcome = bench.sync(&filters, &progress).tree("").await;

    assert_eq!(outcome, Ok(()));
    assert_eq!(bench.copy("keep.txt").as_deref(), Some("changed"));
    assert_eq!(bench.copy("old/gone.txt"), None);
    assert!(
        !bench.destination.path().join("old").exists(),
        "an emptied folder goes"
    );
    assert_eq!(progress.removed.load(Ordering::Relaxed), 1);
    // Left out by a filter is not gone: its copy stays. Never the rule's: never touched.
    assert_eq!(bench.copy("filtered.log").as_deref(), Some("log"));
    assert_eq!(bench.copy("theirs.txt").as_deref(), Some("not the rule's"));
}

#[tokio::test]
async fn what_a_mirror_removes_or_replaces_goes_to_the_trash_of_the_disk() {
    let bench = Bench::new().await;
    bench.origin.write("a.txt", "first");
    bench.origin.write("b.txt", "b");
    assert_eq!(bench.run().await.0, Ok(()));
    std::thread::sleep(Duration::from_millis(20));
    bench.origin.write("a.txt", "second");
    fs::remove_file(bench.origin.path().join("b.txt")).expect("gone");

    // The destination stands for its disk.
    let trash = Trash::new(bench.destination.path().to_path_buf());
    let filters = Filters::default();
    let progress = Progress::default();
    let mut sync = bench.sync(&filters, &progress);
    sync.trash = Some(&trash);
    assert_eq!(sync.tree("").await, Ok(()));
    assert_eq!(bench.copy("a.txt").as_deref(), Some("second"));
    assert_eq!(bench.copy("b.txt"), None);
    assert_eq!(bench.copy(".nas-trash/a.txt").as_deref(), Some("first"));
    assert_eq!(bench.copy(".nas-trash/b.txt").as_deref(), Some("b"));
}

#[tokio::test]
async fn an_automatic_run_stops_before_a_burst_of_removals() {
    let bench = Bench::new().await;
    for name in ["a.txt", "b.txt", "c.txt"] {
        bench.origin.write(name, name);
    }
    assert_eq!(bench.run().await.0, Ok(()));
    for name in ["a.txt", "b.txt", "c.txt"] {
        fs::remove_file(bench.origin.path().join(name)).expect("gone");
    }
    let filters = Filters::default();
    let progress = Progress::default();
    let mut sync = bench.sync(&filters, &progress);
    sync.removable = 2;
    let outcome = sync.tree("").await;
    assert!(matches!(outcome, Err(Stop::Failed(reason)) if reason.contains("more than 2 files")));
    assert_eq!(progress.removed.load(Ordering::Relaxed), 0);
    for name in ["a.txt", "b.txt", "c.txt"] {
        assert_eq!(bench.copy(name).as_deref(), Some(name));
    }
    // By hand there is no limit.
    assert_eq!(bench.run().await.0, Ok(()));
    assert_eq!(bench.copy("a.txt"), None);
}

#[tokio::test]
async fn an_identical_file_in_the_destination_is_taken_as_its_copy() {
    let bench = Bench::new().await;
    let original = bench.origin.write("a.txt", "same");
    // A copy made before a crash wrote it down: same bytes, same time.
    let copy = bench.destination.write("a.txt", "same");
    let time = fs::metadata(&original)
        .expect("original")
        .modified()
        .expect("time");
    File::options()
        .write(true)
        .open(&copy)
        .expect("copy")
        .set_modified(time)
        .expect("time kept");
    let filters = Filters::default();
    let progress = Progress::default();
    let mut sync = bench.sync(&filters, &progress);
    sync.resolution = DuplicateResolution::Copy;
    assert_eq!(sync.tree("").await, Ok(()));
    assert_eq!(copied(&progress), 0);
    assert!(!bench.destination.path().join("a (1).txt").exists());
    assert_eq!(bench.records().await.len(), 1);
}

#[tokio::test]
async fn the_folders_under_one_are_read_through_the_index() {
    let bench = Bench::new().await;
    bench.origin.write("a/x.txt", "x");
    bench.origin.write("a/b/y.txt", "y");
    bench.origin.write("ab/z.txt", "z");
    assert_eq!(bench.run().await.0, Ok(()));
    let filters = Filters::default();
    let progress = Progress::default();
    let sync = bench.sync(&filters, &progress);
    let mut under_a = sync.parents_under("a").await.expect("parents");
    under_a.sort();
    assert_eq!(under_a, ["a", "a/b"]);
    assert_eq!(sync.parents_under("").await.expect("parents").len(), 3);
}

#[tokio::test]
async fn copying_only_never_removes_and_resolves_the_files_in_the_way() {
    let bench = Bench::new().await;
    bench.origin.write("a.txt", "new a");
    bench.origin.write("b.txt", "new b");
    bench.origin.write("c.txt", "new c");
    bench.destination.write("a.txt", "their a");
    bench.destination.write("b.txt", "their b");
    bench.destination.write("c.txt", "their c");

    for (name, resolution) in [
        ("a.txt", DuplicateResolution::Overwrite),
        ("b.txt", DuplicateResolution::Ignore),
        ("c.txt", DuplicateResolution::Copy),
    ] {
        let filters = compile(
            &[OriginFilter {
                id: 1,
                action: "include".to_owned(),
                file_type: "*".to_owned(),
                status: "name".to_owned(),
                operator: None,
                value: name.to_owned(),
            }],
            &Clock {
                today: 0,
                yesterday: 0,
                last_week: 0,
                last_month: 0,
                this_month: 0,
            },
        )
        .expect("filters");
        let progress = Progress::default();
        let mut sync = bench.sync(&filters, &progress);
        sync.mirror = false;
        sync.resolution = resolution;
        assert_eq!(sync.tree("").await, Ok(()));
    }
    assert_eq!(bench.copy("a.txt").as_deref(), Some("new a"));
    assert_eq!(bench.copy("b.txt").as_deref(), Some("their b"));
    assert_eq!(bench.copy("c.txt").as_deref(), Some("their c"));
    assert_eq!(bench.copy("c (1).txt").as_deref(), Some("new c"));

    // Gone from the origin: a copy only keeps what it copied.
    fs::remove_file(bench.origin.path().join("a.txt")).expect("gone");
    let (outcome, _) = bench.run_with(None, false).await;
    assert_eq!(outcome, Ok(()));
    assert_eq!(bench.copy("a.txt").as_deref(), Some("new a"));
}

#[tokio::test]
async fn renames_the_copies_and_follows_a_mirror_of_renamed_files() {
    let bench = Bench::new().await;
    bench.origin.write("docs/report.pdf", "r");
    let rename = BatchRenameConfiguration {
        filename: "x".to_owned(),
        separator: "_".to_owned(),
        prefix: "bak".to_owned(),
        position: "suffix".to_owned(),
    };
    let (outcome, _) = bench.run_with(Some(&rename), true).await;
    assert_eq!(outcome, Ok(()));
    assert_eq!(bench.copy("docs/report_bak.pdf").as_deref(), Some("r"));

    fs::remove_file(bench.origin.path().join("docs/report.pdf")).expect("gone");
    let (outcome, progress) = bench.run_with(Some(&rename), true).await;
    assert_eq!(outcome, Ok(()));
    assert_eq!(progress.removed.load(Ordering::Relaxed), 1);
    assert_eq!(bench.copy("docs/report_bak.pdf"), None);
}

#[tokio::test]
async fn a_stopped_run_leaves_no_half_file() {
    let bench = Bench::new().await;
    bench.origin.write("a.txt", "a");
    bench.cancel.store(true, Ordering::Relaxed);
    let (outcome, progress) = bench.run().await;
    assert_eq!(outcome, Err(Stop::Interrupted));
    assert_eq!(copied(&progress), 0);
    assert_eq!(
        fs::read_dir(bench.destination.path())
            .expect("folder")
            .count(),
        0
    );

    let source = bench.origin.write("b.txt", "b");
    let target = bench.destination.path().join("b.txt");
    assert_eq!(
        copy_file(&source, &target, false, &bench.cancel, None),
        Err(Stop::Interrupted)
    );
    assert!(!target.exists());
    bench.cancel.store(false, Ordering::Relaxed);
    let missing = copy_file(
        &bench.origin.path().join("none"),
        &target,
        false,
        &bench.cancel,
        None,
    );
    assert!(matches!(missing, Err(Stop::Failed(reason)) if reason.contains("cannot be read")));
    assert_eq!(
        fs::read_dir(bench.destination.path())
            .expect("folder")
            .count(),
        0
    );
}

#[tokio::test]
async fn follows_the_changes_of_one_folder_and_the_folders_gone_under_it() {
    let bench = Bench::new().await;
    bench.origin.write("top.txt", "top");
    bench.origin.write("a/one.txt", "1");
    bench.origin.write("a/b/two.txt", "2");
    bench.origin.write("a/c/three.txt", "3");
    assert_eq!(bench.run().await.0, Ok(()));

    fs::remove_dir_all(bench.origin.path().join("a/b")).expect("gone");
    bench.origin.write("a/new.txt", "new");
    let filters = Filters::default();
    let progress = Progress::default();
    let sync = bench.sync(&filters, &progress);
    assert_eq!(sync.folder("a").await, Ok(()));
    assert_eq!(sync.folder("not/there").await, Ok(()));

    assert_eq!(bench.copy("a/new.txt").as_deref(), Some("new"));
    assert_eq!(bench.copy("a/b/two.txt"), None);
    assert_eq!(bench.copy("a/c/three.txt").as_deref(), Some("3"));
    assert_eq!(bench.copy("top.txt").as_deref(), Some("top"));
}

#[tokio::test]
async fn never_removes_what_is_under_a_folder_it_cannot_read() {
    let bench = Bench::new().await;
    bench.origin.write("locked/a.txt", "a");
    assert_eq!(bench.run().await.0, Ok(()));
    let locked = bench.origin.path().join("locked");
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("locked");
    if fs::read_dir(&locked).is_ok() {
        // Root reads every folder: nothing to prove here.
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).expect("unlocked");
        return;
    }
    let (outcome, progress) = bench.run().await;
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).expect("unlocked");
    assert_eq!(outcome, Ok(()));
    assert_eq!(progress.errors().len(), 1);
    assert_eq!(bench.copy("locked/a.txt").as_deref(), Some("a"));
}

#[tokio::test]
async fn renames_in_place_once() {
    let bench = Bench::new().await;
    bench.origin.write("a.txt", "a");
    bench.origin.write("sub/b.txt", "b");
    bench.origin.write("bak_c.txt", "already");
    bench.origin.write("d.txt", "d");
    bench.origin.write("bak_d.txt", "taken");
    let rename = BatchRenameConfiguration {
        filename: "x".to_owned(),
        separator: "_".to_owned(),
        prefix: "bak".to_owned(),
        position: "prefix".to_owned(),
    };
    let filters = Filters::default();
    let progress = Progress::default();
    let mut sync = bench.sync(&filters, &progress);
    sync.destination = bench.origin.path();
    sync.rename = Some(&rename);
    sync.resolution = DuplicateResolution::Copy;
    assert_eq!(sync.rename_in_place("", true).await, Ok(()));
    assert_eq!(sync.rename_in_place("", true).await, Ok(()));

    let mut names: Vec<String> = fs::read_dir(bench.origin.path())
        .expect("folder")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "bak_a.txt",
            "bak_c.txt",
            "bak_d (1).txt",
            "bak_d.txt",
            "sub"
        ]
    );
    assert!(bench.origin.path().join("sub/bak_b.txt").is_file());
    assert_eq!(progress.copied.load(Ordering::Relaxed), 3);

    let without = bench.sync(&filters, &progress);
    assert!(matches!(
        without.rename_in_place("", true).await,
        Err(Stop::Failed(_))
    ));
}

#[test]
fn finds_the_folders_a_change_touches() {
    let origin = Scratch::new("sync-affected");
    origin.write("a/b/file.txt", "x");
    let changed = |paths: &[&str]| {
        affected(
            origin.path(),
            &paths
                .iter()
                .map(|path| (*path).to_owned())
                .collect::<Vec<_>>(),
        )
    };
    assert_eq!(changed(&[""]), None);
    assert_eq!(
        changed(&["a/b/file.txt", "a/gone.txt", "top.txt", "a/b", "a"]),
        Some((vec!["a".to_owned()], vec!["".to_owned()]))
    );
    assert_eq!(
        changed(&["a/b", "a/b/file.txt", "x/y.txt"]),
        Some((vec!["a/b".to_owned()], vec!["x".to_owned()]))
    );
    assert_eq!(
        relative_to(Path::new("/srv/a"), Path::new("/srv/a/b/c.txt")).as_deref(),
        Some("b/c.txt")
    );
    assert_eq!(relative_to(Path::new("/srv/a"), Path::new("/srv/b")), None);
}

#[test]
fn names_paths_and_free_copies() {
    let folder = Scratch::new("sync-names");
    folder.write("report.pdf", "1");
    folder.write("report (1).pdf", "2");
    assert_eq!(free_name(folder.path(), "report.pdf"), "report (2).pdf");
    assert_eq!(free_name(folder.path(), "README"), "README (1)");
    assert_eq!(join("", "a"), "a");
    assert_eq!(join("a", "b"), "a/b");
    assert!(under("", "anything"));
    assert!(under("a", "a/b"));
    assert!(!under("a", "ab"));
    assert_eq!(top_under("a", "a/b/c"), "a/b");
    assert_eq!(top_under("", "x/y"), "x");
}

#[test]
fn a_copy_that_differs_from_its_original_is_refused() {
    let folder = Scratch::new("sync-compare");
    let cancel = AtomicBool::new(false);
    let one = folder.write("one", "same");
    let two = folder.write("two", "same");
    let three = folder.write("three", "other");
    let four = folder.write("four", "same, longer");
    assert_eq!(same_bytes(&one, &two, &cancel), Ok(true));
    assert_eq!(same_bytes(&one, &three, &cancel), Ok(false));
    assert_eq!(same_bytes(&one, &four, &cancel), Ok(false));
    assert!(same_bytes(&one, &folder.path().join("none"), &cancel).is_err());
    cancel.store(true, Ordering::Relaxed);
    assert_eq!(same_bytes(&one, &two, &cancel), Err(Stop::Interrupted));
}
