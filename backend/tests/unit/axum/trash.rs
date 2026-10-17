use super::*;

fn scratch(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!("backup-trash-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&folder);
    fs::create_dir_all(folder.join("Backup/2026")).unwrap();
    folder
}

#[test]
fn a_removed_copy_goes_to_the_trash_of_its_disk_keeping_every_version() {
    let disk = scratch("versions");
    let trash = Trash::new(disk.clone());
    let copy = disk.join("Backup/2026/a.txt");
    fs::write(&copy, "first").unwrap();
    trash.discard(&copy).unwrap();
    assert!(!copy.exists());
    assert_eq!(
        fs::read_to_string(disk.join(".nas-trash/Backup/2026/a.txt")).unwrap(),
        "first"
    );
    assert_eq!(
        fs::metadata(disk.join(".nas-trash/Backup"))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o2770
    );
    fs::write(&copy, "second").unwrap();
    trash.discard(&copy).unwrap();
    assert_eq!(
        fs::read_to_string(disk.join(".nas-trash/Backup/2026/a.txt")).unwrap(),
        "second"
    );
    assert_eq!(
        fs::read_to_string(disk.join(".nas-trash/Backup/2026/a (1).txt")).unwrap(),
        "first"
    );
    fs::remove_dir_all(&disk).unwrap();
}

#[test]
fn with_the_trash_off_a_copy_is_removed_and_nothing_leaves_the_disk() {
    let disk = scratch("off");
    let trash = Trash::new(disk.clone());
    let copy = disk.join("Backup/b.txt");
    fs::write(&copy, "x").unwrap();
    fs::write(disk.join(".nas-trash-off"), "").unwrap();
    trash.discard(&copy).unwrap();
    assert!(!copy.exists());
    assert!(!disk.join(".nas-trash").exists());
    assert!(trash.discard(Path::new("/etc/passwd")).is_err());
    assert!(trash.discard(&disk).is_err());
    fs::remove_dir_all(&disk).unwrap();
}

#[test]
fn a_link_in_the_trash_is_refused() {
    let disk = scratch("link");
    let trash = Trash::new(disk.clone());
    std::os::unix::fs::symlink("/tmp", disk.join(".nas-trash")).unwrap();
    let copy = disk.join("Backup/c.txt");
    fs::write(&copy, "x").unwrap();
    assert!(trash.discard(&copy).is_err());
    assert!(copy.exists());
    fs::remove_dir_all(&disk).unwrap();
}

#[test]
fn a_taken_name_gets_the_first_free_number_beside_it() {
    let disk = scratch("numbers");
    let taken = disk.join("report.pdf");
    fs::write(&taken, "x").unwrap();
    assert_eq!(numbered(&taken), disk.join("report (1).pdf"));
    fs::write(disk.join("report (1).pdf"), "x").unwrap();
    assert_eq!(numbered(&taken), disk.join("report (2).pdf"));
    fs::remove_dir_all(&disk).unwrap();
}
