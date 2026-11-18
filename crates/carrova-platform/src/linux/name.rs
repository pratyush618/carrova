//! This computer's human name: the pretty hostname the user set (GNOME
//! Settings › About › Device Name), else a hostname someone chose, else
//! the machine's model, else whatever the hostname is.
//!
//! The pretty hostname is read from `/etc/machine-info` (machine-info(5)),
//! the file `systemd-hostnamed` itself keeps it in: no bus, no service to
//! start, and readable where hostnamed is absent (containers, minimal
//! installs).

use crate::device::sanitize_device_name;

/// The model, from `/sys/class/dmi/id`.
const DMI: &str = "/sys/class/dmi/id";

/// Hostnames nobody chose: install and kernel defaults.
const PLACEHOLDER_HOSTS: [&str; 4] = ["localhost", "localhost.localdomain", "linux", "(none)"];

/// DMI strings firmware leaves unset (compared case-insensitively).
const OEM_PLACEHOLDERS: [&str; 12] = [
    "to be filled by o.e.m.",
    "system product name",
    "system manufacturer",
    "system version",
    "default string",
    "not applicable",
    "not specified",
    "none",
    "o.e.m.",
    "oem",
    "type1productconfigid",
    "all series",
];

/// How each vendor reads in a name, by the lowercase start of its DMI
/// `sys_vendor`. Vendors not listed are left out of the name: their DMI
/// strings ("Micro-Star International Co., Ltd.") read badly.
const VENDORS: [(&str, &str); 22] = [
    ("lenovo", "Lenovo"),
    ("dell", "Dell"),
    ("hewlett-packard", "HP"),
    ("hp", "HP"),
    ("asustek", "ASUS"),
    ("asus", "ASUS"),
    ("acer", "Acer"),
    ("micro-star", "MSI"),
    ("msi", "MSI"),
    ("apple", "Apple"),
    ("framework", "Framework"),
    ("microsoft", "Microsoft"),
    ("samsung", "Samsung"),
    ("toshiba", "Toshiba"),
    ("dynabook", "Dynabook"),
    ("fujitsu", "Fujitsu"),
    ("gigabyte", "Gigabyte"),
    ("razer", "Razer"),
    ("huawei", "Huawei"),
    ("system76", "System76"),
    ("tuxedo", "TUXEDO"),
    ("google", "Google"),
];

/// What the machine says about its name, raw as read.
#[derive(Debug, Default, Clone, Copy)]
struct MachineNames<'a> {
    /// `PRETTY_HOSTNAME` from `/etc/machine-info`.
    pretty: Option<&'a str>,
    /// The kernel hostname.
    host: &'a str,
    /// `ID` from os-release (`ubuntu`): a hostname installers default to.
    os_id: Option<&'a str>,
    /// DMI `sys_vendor`.
    vendor: Option<&'a str>,
    /// DMI `product_family`.
    family: Option<&'a str>,
    /// DMI `product_name`.
    product: Option<&'a str>,
}

/// The name to show for this machine (sanitised): the pretty hostname,
/// else the hostname unless it is a placeholder nobody chose, else the
/// model ([`model`]), else the hostname anyway.
fn machine_name(m: &MachineNames<'_>) -> Option<String> {
    let host = || sanitize_device_name(m.host);
    m.pretty
        .and_then(sanitize_device_name)
        .or_else(|| host().filter(|h| !placeholder_host(h, m.os_id)))
        .or_else(|| model(m))
        .or_else(host)
}

/// Whether `host` is a default no one picked: a known placeholder or the
/// distro's own id.
fn placeholder_host(host: &str, os_id: Option<&str>) -> bool {
    PLACEHOLDER_HOSTS
        .iter()
        .chain(os_id.as_slice())
        .any(|p| host.eq_ignore_ascii_case(p.trim()))
}

/// The machine's model, its vendor first where that reads naturally
/// ("Lenovo IdeaPad Gaming 3 15ACH6"). The product name, unless it is a
/// bare type code with no space ("82K2", "20XW0055US"), then the family.
fn model(m: &MachineNames<'_>) -> Option<String> {
    let product = set(m.product);
    let model = product
        .filter(|p| p.contains(' '))
        .or_else(|| set(m.family))
        .or(product)?;
    let vendor = set(m.vendor).and_then(vendor_name).filter(|v| {
        !model
            .get(..v.len())
            .is_some_and(|start| start.eq_ignore_ascii_case(v))
    });
    sanitize_device_name(&match vendor {
        Some(v) => format!("{v} {model}"),
        None => model.to_owned(),
    })
}

/// `raw` trimmed, unless firmware left it unset.
fn set(raw: Option<&str>) -> Option<&str> {
    raw.map(str::trim).filter(|s| !oem_placeholder(s))
}

fn oem_placeholder(s: &str) -> bool {
    s.is_empty() || OEM_PLACEHOLDERS.iter().any(|p| s.eq_ignore_ascii_case(p))
}

fn vendor_name(raw: &str) -> Option<&'static str> {
    let raw = raw.to_lowercase();
    VENDORS
        .iter()
        .find(|(start, _)| raw.starts_with(start))
        .map(|&(_, name)| name)
}

/// The value of `key` in an env-style file (os-release(5),
/// machine-info(5)): quotes removed, backslash escapes undone.
pub(super) fn env_value(text: &str, key: &str) -> Option<String> {
    let raw = text.lines().find_map(|l| {
        l.trim_start()
            .strip_prefix(key)?
            .strip_prefix('=')
            .map(str::trim)
    })?;
    if let Some(single) = raw.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')) {
        return Some(single.to_owned());
    }
    let raw = raw
        .strip_prefix('"')
        .and_then(|r| r.strip_suffix('"'))
        .unwrap_or(raw);
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        out.push(if c == '\\' {
            chars.next().unwrap_or(c)
        } else {
            c
        });
    }
    Some(out)
}

/// [`machine_name`] for this machine.
pub(super) fn device_name() -> Option<String> {
    let read = |path: &str| std::fs::read_to_string(path).ok();
    let dmi = |field: &str| read(&format!("{DMI}/{field}"));
    let pretty = read("/etc/machine-info").and_then(|t| env_value(&t, "PRETTY_HOSTNAME"));
    let os_id = read("/etc/os-release")
        .or_else(|| read("/usr/lib/os-release"))
        .and_then(|t| env_value(&t, "ID"));
    let (vendor, family, product) = (
        dmi("sys_vendor"),
        dmi("product_family"),
        dmi("product_name"),
    );
    machine_name(&MachineNames {
        pretty: pretty.as_deref(),
        host: &crate::netops::host_name(),
        os_id: os_id.as_deref(),
        vendor: vendor.as_deref(),
        family: family.as_deref(),
        product: product.as_deref(),
    })
}

#[cfg(test)]
#[path = "../../tests/linux/name/unit.rs"]
mod tests;
