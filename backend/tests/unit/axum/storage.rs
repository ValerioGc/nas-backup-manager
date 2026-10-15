use super::*;
use crate::test_support::Scratch;

#[test]
fn reads_the_disks_of_the_registry() {
    let volumes = parse_registry(
        "abc\tPhotos\t/srv/storage/abc\tdevice\nnet\tShare\t/mnt/net\nn-1\tOffice\t/srv/storage/n-1\tnetwork\n\n",
    )
    .expect("registry");
    assert_eq!(
        volumes,
        [
            Volume {
                id: "abc".to_owned(),
                name: "Photos".to_owned(),
                path: PathBuf::from("/srv/storage/abc"),
                network: false,
            },
            Volume {
                id: "net".to_owned(),
                name: "Share".to_owned(),
                path: PathBuf::from("/mnt/net"),
                network: false,
            },
            Volume {
                id: "n-1".to_owned(),
                name: "Office".to_owned(),
                path: PathBuf::from("/srv/storage/n-1"),
                network: true,
            },
        ]
    );
    assert!(parse_registry("only\ttwo").is_err());
    assert!(parse_registry("a\t\t/path").is_err());
}

#[test]
fn resolves_a_folder_on_its_disk_and_never_out_of_it() {
    let disk = Scratch::new("storage-root");
    disk.write("Photos/2026/a.jpg", "a");
    let outside = Scratch::new("storage-outside");
    std::os::unix::fs::symlink(outside.path(), disk.path().join("escape")).expect("link");
    let storage = Storage::Root(disk.path().to_path_buf());

    let photos = storage.resolve("/Photos/2026", false).expect("resolved");
    assert_eq!(photos.path, disk.path().join("Photos/2026"));
    assert_eq!(photos.volume, disk.path());
    assert!(storage.resolve("/Missing", false).is_err());
    let made = storage.resolve("/Backup/new", true).expect("created");
    assert!(made.path.is_dir());
    for refused in ["/Photos/../..", "/escape", "/Photos/2026/a.jpg"] {
        assert!(storage.resolve(refused, false).is_err(), "{refused}");
    }
}

#[test]
fn takes_only_a_disk_of_the_registry_that_is_mounted() {
    // The root of the machine is always a mount point; a scratch folder never is.
    let unmounted = Scratch::new("storage-unmounted");
    let storage = Storage::Volumes(vec![
        Volume {
            id: "system".to_owned(),
            name: "System".to_owned(),
            path: PathBuf::from("/"),
            network: true,
        },
        Volume {
            id: "gone".to_owned(),
            name: "Gone".to_owned(),
            path: unmounted.path().to_path_buf(),
            network: false,
        },
    ]);

    let folder = storage
        .resolve("/system/tmp", false)
        .expect("on a mounted disk");
    assert_eq!(folder.volume, PathBuf::from("/"));
    assert!(folder.network);
    let refused = storage.resolve("/gone", false).expect_err("not mounted");
    assert!(refused.contains("not mounted"), "{refused}");
    assert!(storage
        .resolve("/other/x", false)
        .expect_err("unknown")
        .contains("not connected"));
    assert!(storage
        .resolve("/", false)
        .expect_err("no disk")
        .contains("names no disk"));
}

#[test]
fn reads_the_paths_of_the_mount_table_and_relative_paths() {
    assert_eq!(unescape("/srv/my\\040disk\\134x"), "/srv/my disk\\x");
    assert!(is_mounted(Path::new("/")));
    assert!(nested(Path::new("/a/b"), Path::new("/a")));
    assert!(!nested(Path::new("/ab"), Path::new("/a")));
    assert_eq!(relative(Path::new("a/b/c.txt")), "a/b/c.txt");
}

#[test]
fn takes_the_disks_from_the_environment() {
    let registry = Scratch::new("storage-env");
    let file = registry.write("volumes.tsv", "abc\tPhotos\t/srv/abc\n");
    // The variables of the launcher, set and taken away within this test only.
    unsafe { std::env::set_var("NAS_STORAGE_REGISTRY", &file) };
    let granted = Storage::from_env();
    unsafe { std::env::remove_var("NAS_STORAGE_REGISTRY") };
    assert!(matches!(granted, Ok(Storage::Volumes(volumes)) if volumes.len() == 1));
    assert!(matches!(Storage::from_env(), Ok(Storage::Root(_))));
}

#[test]
fn names_the_disks_for_the_page() {
    let storage = Storage::Volumes(
        parse_registry("abc\tPhotos\t/srv/storage/abc\tdevice\n").expect("registry"),
    );
    assert_eq!(
        storage.names(),
        [VolumeName {
            id: "abc".to_owned(),
            name: "Photos".to_owned()
        }]
    );
    assert!(Storage::Root(PathBuf::from("/tmp")).names().is_empty());
}

#[tokio::test]
async fn a_network_folder_says_whether_it_holds_files() {
    let share = Scratch::new("storage-share");
    assert_eq!(holds_files(share.path()).await, Ok(false));
    share.write("a.txt", "a");
    assert_eq!(holds_files(share.path()).await, Ok(true));
    assert!(holds_files(&share.path().join("missing"))
        .await
        .expect_err("missing")
        .contains("cannot be read"));
}
