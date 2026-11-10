use carrova_platform::OsKind;
use carrova_view::lan::InventoryView;

use crate::render::lan::inventory;

#[test]
fn inventory_names_the_other_machine_when_it_announced_a_name() {
    let v = InventoryView {
        os: OsKind::Linux,
        carrova_version: "0.1",
        apps: Vec::new(),
        free_space: None,
        device_name: Some("ana-laptop".into()),
    };
    assert!(inventory(&v).starts_with("Other machine: ana-laptop — Linux"));
}
