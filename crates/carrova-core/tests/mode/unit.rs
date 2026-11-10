use super::*;

const DEFAULT: ModePolicy = ModePolicy {
    keep_group_write: false,
};
const KEEP: ModePolicy = ModePolicy {
    keep_group_write: true,
};

#[test]
fn group_write_kept_only_on_opt_in() {
    assert_eq!(DEFAULT.final_mode(0o664, None), 0o644);
    assert_eq!(KEEP.final_mode(0o664, None), 0o664);
    assert_eq!(KEEP.final_mode(0o666, None), 0o664);
    assert_eq!(KEEP.final_mode(0o6777, None), 0o775);
}

#[test]
fn existing_dir_is_never_loosened() {
    assert_eq!(KEEP.final_mode(0o775, Some(0o755)), 0o755);
    assert_eq!(DEFAULT.final_mode(0o700, Some(0o40755)), 0o700);
}

#[test]
fn dropped_bits_are_classified() {
    let mut d = DroppedBits::default();
    d.add(0o664, 0o644);
    d.add(0o644, 0o644);
    assert_eq!((d.paths, d.classes()), (1, vec!["group-write"]));
    assert!(d.group_write_only());
    d.add(0o4777, 0o700);
    assert_eq!(
        d.classes(),
        vec![
            "group-write",
            "world-write",
            "setuid",
            "other bits (existing directory kept)"
        ]
    );
    assert!(!d.group_write_only());
}
