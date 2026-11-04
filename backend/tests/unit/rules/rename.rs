use super::*;

fn rename(text: &str, position: &str) -> BatchRenameConfiguration {
    BatchRenameConfiguration {
        filename: "example.txt".to_owned(),
        separator: "_".to_owned(),
        prefix: text.to_owned(),
        position: position.to_owned(),
    }
}

#[test]
fn adds_the_text_before_the_name_or_before_the_extension() {
    let before = rename(" backup ", "prefix");
    let after = rename("2026", "suffix");
    assert_eq!(renamed("report.pdf", &before), "backup_report.pdf");
    assert_eq!(renamed("report.pdf", &after), "report_2026.pdf");
    assert_eq!(renamed("archive.tar.gz", &after), "archive.tar_2026.gz");
    assert_eq!(renamed("README", &after), "README_2026");
    assert_eq!(renamed(".profile", &after), ".profile_2026");
}

#[test]
fn never_renames_a_file_twice() {
    let before = rename("backup", "prefix");
    let after = rename("2026", "suffix");
    assert!(carries("backup_report.pdf", &before));
    assert!(!carries("backupreport.pdf", &before));
    assert_eq!(renamed("backup_report.pdf", &before), "backup_report.pdf");
    assert!(carries("report_2026.pdf", &after));
    assert_eq!(renamed("report_2026.pdf", &after), "report_2026.pdf");
    assert!(!carries("report.pdf", &after));
}
