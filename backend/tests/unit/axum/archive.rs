use super::*;
use crate::test_support::Scratch;

struct Bench {
    origin: Scratch,
    destination: Scratch,
    work: Scratch,
    cancel: Arc<AtomicBool>,
    copied: AtomicU64,
    filters: Filters,
    options: ArchiveConfiguration,
}

impl Bench {
    fn new() -> Self {
        Self {
            origin: Scratch::new("archive-origin"),
            destination: Scratch::new("archive-destination"),
            work: Scratch::new("archive-work"),
            cancel: Arc::new(AtomicBool::new(false)),
            copied: AtomicU64::new(0),
            filters: Filters::default(),
            options: ArchiveConfiguration {
                archive_name: "photos".to_owned(),
                password: "s3cret pass".to_owned(),
                encrypt_file_info: true,
                verify_archive: true,
            },
        }
    }

    fn archive<'a>(&'a self, seven_zip: &'a str, links: &'a Path) -> Archive<'a> {
        Archive {
            seven_zip,
            origin: self.origin.path(),
            destination: self.destination.path(),
            links,
            filters: &self.filters,
            rename: None,
            options: &self.options,
            mirror: true,
            resolution: DuplicateResolution::Overwrite,
            verify: false,
            previous: None,
            fingerprint: None,
            stamp: "20261010-1830",
            cancel: &self.cancel,
            copied: &self.copied,
        }
    }

    /// The names inside an archive, read with its password.
    fn listing(&self, archive: &str) -> Vec<String> {
        let output = std::process::Command::new("7z")
            .args(["l", "-ba", "-slt"])
            .arg(format!("-p{}", self.options.password))
            .arg(self.destination.path().join(archive))
            .output()
            .expect("7z");
        let mut names: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| line.strip_prefix("Path = "))
            .map(str::to_owned)
            .filter(|path| !self.origin.path().join(path).is_dir())
            .collect();
        names.sort();
        names
    }
}

/// A 7z standing in for the real one, kept with the tests: never written while they run.
fn fake(name: &str) -> String {
    format!(
        "{}/../tests/unit/axum/fixtures/{name}-7z",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn has_seven_zip() -> bool {
    std::process::Command::new("7z")
        .arg("i")
        .output()
        .is_ok_and(|output| output.status.success())
}

#[tokio::test]
async fn a_mirror_keeps_one_archive_in_line_with_the_origin() {
    if !has_seven_zip() {
        eprintln!("7z is not installed: the archive is not tried");
        return;
    }
    let bench = Bench::new();
    bench.origin.write("a.txt", "a");
    bench.origin.write("sub/b.txt", "b");
    let links = links_of(bench.work.path(), "rule/1");
    let rename = BatchRenameConfiguration {
        filename: "x".to_owned(),
        separator: "_".to_owned(),
        prefix: "bak".to_owned(),
        position: "prefix".to_owned(),
    };

    let mut archive = bench.archive("7z", &links);
    archive.rename = Some(&rename);
    archive.verify = true;
    let first = archive.run(false).await.expect("archived");
    assert_eq!(first.path, "photos.7z");
    assert!(!first.unchanged);
    assert_eq!(bench.listing("photos.7z"), ["bak_a.txt", "sub/bak_b.txt"]);
    assert_eq!(bench.copied.load(Ordering::Relaxed), 2);
    // Without the password, nothing is readable, not even the names.
    let locked = std::process::Command::new("7z")
        .args(["l", "-pwrong"])
        .arg(bench.destination.path().join("photos.7z"))
        .output()
        .expect("7z");
    assert!(!locked.status.success());

    // Nothing changed: nothing written.
    archive.previous = Some(&first.path);
    archive.fingerprint = Some(&first.fingerprint);
    let same = archive.run(false).await.expect("checked");
    assert!(same.unchanged);

    // A file gone and one new: the archive follows.
    fs::remove_file(bench.origin.path().join("sub/b.txt")).expect("gone");
    bench.origin.write("c.txt", "c");
    let updated = archive.run(false).await.expect("updated");
    assert!(!updated.unchanged);
    assert_eq!(updated.path, "photos.7z");
    assert_eq!(bench.listing("photos.7z"), ["bak_a.txt", "bak_c.txt"]);
}

#[tokio::test]
async fn a_copy_writes_a_new_archive_and_resolves_one_in_the_way() {
    if !has_seven_zip() {
        return;
    }
    let bench = Bench::new();
    bench.origin.write("a.txt", "a");
    let links = links_of(bench.work.path(), "rule");
    let mut archive = bench.archive("7z", &links);
    archive.mirror = false;
    let made = archive.run(false).await.expect("archived");
    assert_eq!(made.path, "photos-20261010-1830.7z");

    archive.resolution = DuplicateResolution::Copy;
    let beside = archive.run(false).await.expect("beside");
    assert_eq!(beside.path, "photos-20261010-1830 (1).7z");

    archive.resolution = DuplicateResolution::Ignore;
    let left = archive.run(false).await.expect_err("left alone");
    assert!(matches!(left, Stop::Failed(reason) if reason.contains("leaves it alone")));

    fs::remove_file(bench.origin.path().join("a.txt")).expect("emptied");
    let empty = archive.run(false).await.expect_err("nothing");
    assert!(matches!(empty, Stop::Failed(reason) if reason.contains("nothing to archive")));
}

#[tokio::test]
async fn the_password_goes_on_the_input_never_on_the_command_line() {
    let bench = Bench::new();
    bench.origin.write("a.txt", "a");
    let links = bench.work.path().join("links");
    let recording = fake("recording");
    let made = bench
        .archive(&recording, &links)
        .run(false)
        .await
        .expect("archived by the fake");
    assert_eq!(made.path, "photos.7z");

    let calls = fs::read_to_string(bench.destination.path().join(".calls")).expect("calls");
    assert!(calls.contains("args: a -bd -t7z -mx=5 -ms=off -mmt=2 -p -mhe=on"));
    assert!(calls.contains("args: t -bd"));
    let arguments: Vec<&str> = calls
        .lines()
        .filter(|line| line.starts_with("args:"))
        .collect();
    assert!(arguments.iter().all(|line| !line.contains("s3cret")));
    assert!(calls.lines().any(|line| line == "s3cret pass"));
}

#[tokio::test]
async fn says_why_7z_failed_or_could_not_start() {
    let bench = Bench::new();
    bench.origin.write("a.txt", "a");
    let links = bench.work.path().join("links");
    let failing = fake("failing");
    let failed = bench
        .archive(&failing, &links)
        .run(false)
        .await
        .expect_err("failed");
    assert!(
        matches!(&failed, Stop::Failed(reason) if reason.contains("disk full")),
        "{failed:?}"
    );
    assert_eq!(
        fs::read_dir(bench.destination.path())
            .expect("folder")
            .count(),
        0,
        "no temporary archive is left"
    );

    let missing = bench
        .archive("/nonexistent/7z", &links)
        .run(false)
        .await
        .expect_err("missing");
    assert!(matches!(missing, Stop::Failed(reason) if reason.contains("cannot be started")));
}

#[tokio::test]
async fn a_stop_kills_7z() {
    let bench = Bench::new();
    bench.origin.write("a.txt", "a");
    let links = bench.work.path().join("links");
    let slow = fake("slow");
    let cancel = Arc::clone(&bench.cancel);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        cancel.store(true, Ordering::Relaxed);
    });
    let started = std::time::Instant::now();
    let stopped = bench.archive(&slow, &links).run(false).await;
    assert_eq!(stopped, Err(Stop::Interrupted));
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn links_stand_for_the_files_and_leave_the_archives_out() {
    let bench = Bench::new();
    bench.origin.write("a.txt", "a");
    bench.origin.write("photos.7z", "own archive");
    bench.origin.write("photos-20261010-1830.7z", "own copy");
    bench.origin.write("sub/photos.7z", "not at the top");
    let links = bench.work.path().join("links");
    let cancel = AtomicBool::new(false);

    let (count, fingerprint) = link(
        bench.origin.path(),
        &links,
        &bench.filters,
        None,
        "photos",
        true,
        &cancel,
    )
    .expect("linked");
    assert_eq!(count, 2);
    assert!(fs::symlink_metadata(links.join("a.txt"))
        .expect("link")
        .file_type()
        .is_symlink());
    assert!(!links.join("photos.7z").exists());
    assert!(links.join("sub/photos.7z").exists());
    let (again, same) = link(
        bench.origin.path(),
        &links,
        &bench.filters,
        None,
        "photos",
        true,
        &cancel,
    )
    .expect("linked again");
    assert_eq!((again, &same), (2, &fingerprint));
    let (all, _) = link(
        bench.origin.path(),
        &links,
        &bench.filters,
        None,
        "photos",
        false,
        &cancel,
    )
    .expect("all");
    assert_eq!(all, 4);

    cancel.store(true, Ordering::Relaxed);
    assert_eq!(
        link(
            bench.origin.path(),
            &links,
            &bench.filters,
            None,
            "photos",
            true,
            &cancel
        ),
        Err(Stop::Interrupted)
    );
    assert!(is_own(".nas-backup-1.7z", "photos"));
    assert!(is_own("photos.7z.tmp", "photos"));
    assert!(!is_own("photos-notes.txt", "photos"));
    assert!(links_of(Path::new("/state"), "a/b c").ends_with("links/a_b_c"));
}
