use super::numbered;

#[test]
fn copies_number_before_the_extension() {
    assert_eq!(numbered("report.txt", 1), "report.txt");
    assert_eq!(numbered("report.txt", 2), "report (2).txt");
    assert_eq!(numbered("archive.tar.gz", 3), "archive.tar (3).gz");
    assert_eq!(numbered("README", 2), "README (2)");
    assert_eq!(numbered(".bashrc", 2), ".bashrc (2)");
}
