//! Facts about this Carrova and this machine.

use carrova_core::about::device_name;
use carrova_platform::fake::FakePlatform;

#[test]
fn device_name_is_the_platforms_sanitised_name() {
    let platform = FakePlatform::with_home("/home/user").with_device_name("  dev-laptop\n");
    assert_eq!(device_name(&platform).as_deref(), Some("dev-laptop"));
}

#[test]
fn device_name_is_none_when_the_platform_has_none() {
    assert_eq!(device_name(&FakePlatform::with_home("/home/user")), None);
}
