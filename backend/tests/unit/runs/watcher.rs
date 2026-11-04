use super::*;

#[test]
fn leaves_out_the_rules_own_writing() {
    assert!(ignored(".nas-backup-1.tmp", None));
    assert!(ignored("sub/.nas-backup-1.tmp", None));
    assert!(ignored(".nas-trash/old.jpg", None));
    assert!(ignored(".nas-trash-off", None));
    assert!(!ignored("my.nas-trash.txt", None));
    assert!(ignored("photos.7z", Some("photos")));
    assert!(ignored("photos-20261010-1830.7z", Some("photos")));
    assert!(!ignored("sub/photos.7z", Some("photos")));
    assert!(!ignored("photos.7z", None));
    assert!(!ignored("holiday.jpg", Some("photos")));
}
