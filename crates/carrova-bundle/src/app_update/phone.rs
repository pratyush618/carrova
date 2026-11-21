//! What an app update needs of the phone, through the [`Platform`].

use std::path::Path;

use carrova_platform::{AppPackage, Platform, PlatformError, UpdateTap};

/// The phone's side of an update: network, notification taps, package
/// facts and the installer. Every [`Platform`] is one.
pub trait Phone {
    /// Whether it is on Wi-Fi (or Ethernet) now.
    fn on_wifi(&self) -> bool;
    /// The user's last tap on the update notification, taken once.
    fn tap(&self) -> Option<UpdateTap>;
    /// [`Platform::inspect_package`].
    ///
    /// # Errors
    /// As [`Platform::inspect_package`].
    fn inspect(&self, apk: &Path) -> Result<AppPackage, PlatformError>;
    /// [`Platform::installed_package`].
    ///
    /// # Errors
    /// As [`Platform::installed_package`].
    fn installed(&self) -> Result<AppPackage, PlatformError>;
    /// [`Platform::can_install_packages`].
    ///
    /// # Errors
    /// As [`Platform::can_install_packages`].
    fn can_install(&self) -> Result<bool, PlatformError>;
    /// [`Platform::install_package`].
    ///
    /// # Errors
    /// As [`Platform::install_package`].
    fn install(&self, apk: &Path) -> Result<(), PlatformError>;
}

impl<P: Platform + ?Sized> Phone for P {
    /// Unknown counts as off Wi-Fi: no download on mobile data by mistake.
    fn on_wifi(&self) -> bool {
        self.on_local_network().unwrap_or(false)
    }

    fn tap(&self) -> Option<UpdateTap> {
        self.take_update_tap()
    }

    fn inspect(&self, apk: &Path) -> Result<AppPackage, PlatformError> {
        self.inspect_package(apk)
    }

    fn installed(&self) -> Result<AppPackage, PlatformError> {
        self.installed_package()
    }

    fn can_install(&self) -> Result<bool, PlatformError> {
        self.can_install_packages()
    }

    fn install(&self, apk: &Path) -> Result<(), PlatformError> {
        self.install_package(apk)
    }
}
