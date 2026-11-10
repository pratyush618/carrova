use super::*;

#[test]
fn symlinks_stay_inside_root_or_home() {
    let link = Path::new("/home/u/.config/app/l");
    let root = Path::new("/home/u/.config");
    let home = Some(Path::new("/home/u"));
    assert!(!symlink_escapes("x", link, root, home));
    assert!(!symlink_escapes("../../docs", link, root, home));
    assert!(symlink_escapes("/etc/passwd", link, root, home));
    assert!(symlink_escapes("../../../other", link, root, None));
}
