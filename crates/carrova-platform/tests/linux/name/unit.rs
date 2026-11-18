use super::*;

/// The laptop that showed up as "linux": no pretty hostname, Ubuntu.
fn ideapad<'a>() -> MachineNames<'a> {
    MachineNames {
        pretty: None,
        host: "linux",
        os_id: Some("ubuntu"),
        vendor: Some("LENOVO\n"),
        family: Some("IdeaPad Gaming 3 15ACH6\n"),
        product: Some("82K2\n"),
    }
}

fn name(m: MachineNames<'_>) -> Option<String> {
    machine_name(&m)
}

#[test]
fn pretty_hostname_wins() {
    let m = MachineNames {
        pretty: Some(" Ezio’s Laptop\n"),
        host: "ezio-box",
        ..ideapad()
    };
    assert_eq!(name(m).as_deref(), Some("Ezio’s Laptop"));
}

#[test]
fn a_chosen_hostname_beats_the_model() {
    let m = MachineNames {
        host: "ezio-box",
        ..ideapad()
    };
    assert_eq!(name(m).as_deref(), Some("ezio-box"));
}

#[test]
fn a_placeholder_hostname_gives_the_vendor_and_model() {
    for host in [
        "linux",
        "localhost",
        "LOCALHOST.localdomain",
        "(none)",
        "ubuntu",
    ] {
        let m = MachineNames { host, ..ideapad() };
        assert_eq!(
            name(m).as_deref(),
            Some("Lenovo IdeaPad Gaming 3 15ACH6"),
            "{host}"
        );
    }
}

#[test]
fn the_distro_id_is_a_placeholder_only_for_that_distro() {
    let m = MachineNames {
        host: "fedora",
        ..ideapad()
    };
    assert_eq!(name(m).as_deref(), Some("fedora"));
}

#[test]
fn a_blank_pretty_hostname_is_ignored() {
    let m = MachineNames {
        pretty: Some(" \u{202E}"),
        ..ideapad()
    };
    assert_eq!(name(m).as_deref(), Some("Lenovo IdeaPad Gaming 3 15ACH6"));
}

#[test]
fn a_descriptive_product_name_beats_the_family() {
    let m = MachineNames {
        vendor: Some("Dell Inc."),
        family: Some("XPS"),
        product: Some("XPS 13 9310"),
        ..ideapad()
    };
    assert_eq!(name(m).as_deref(), Some("Dell XPS 13 9310"));
}

#[test]
fn the_vendor_is_not_repeated() {
    let m = MachineNames {
        vendor: Some("HP"),
        family: Some("103C_5336AN HP EliteBook"),
        product: Some("HP EliteBook 840 G8 Notebook PC"),
        ..ideapad()
    };
    assert_eq!(name(m).as_deref(), Some("HP EliteBook 840 G8 Notebook PC"));
}

#[test]
fn an_unknown_vendor_is_left_out() {
    let m = MachineNames {
        vendor: Some("Notebook"),
        family: None,
        product: Some("NH5x_7xRCx,RDx"),
        ..ideapad()
    };
    assert_eq!(name(m).as_deref(), Some("NH5x_7xRCx,RDx"));
}

#[test]
fn oem_placeholders_fall_back_to_the_hostname() {
    let m = MachineNames {
        vendor: Some("To Be Filled By O.E.M."),
        family: Some("To be filled by O.E.M."),
        product: Some("System Product Name"),
        ..ideapad()
    };
    assert_eq!(name(m).as_deref(), Some("linux"));
    let m = MachineNames {
        vendor: Some("Default string"),
        family: Some("Default string"),
        product: Some("Default string"),
        ..ideapad()
    };
    assert_eq!(name(m).as_deref(), Some("linux"));
}

#[test]
fn no_dmi_falls_back_to_the_hostname() {
    let m = MachineNames {
        vendor: None,
        family: None,
        product: None,
        ..ideapad()
    };
    assert_eq!(name(m).as_deref(), Some("linux"));
}

#[test]
fn nothing_at_all_is_none() {
    assert_eq!(name(MachineNames::default()), None);
}

#[test]
fn env_values_are_unquoted_and_unescaped() {
    let text = "# comment\nPRETTY_HOSTNAME=\"Zoë's \\\"big\\\" box\"\nICON_NAME=computer-laptop\nID_LIKE=debian\nID=ubuntu\nNAME='Ubuntu'\n";
    assert_eq!(
        env_value(text, "PRETTY_HOSTNAME").as_deref(),
        Some("Zoë's \"big\" box")
    );
    assert_eq!(
        env_value(text, "ICON_NAME").as_deref(),
        Some("computer-laptop")
    );
    assert_eq!(env_value(text, "ID").as_deref(), Some("ubuntu"));
    assert_eq!(env_value(text, "NAME").as_deref(), Some("Ubuntu"));
    assert_eq!(env_value(text, "MISSING"), None);
}
