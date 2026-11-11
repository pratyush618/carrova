//! Start at sign-in on macOS: a per-user LaunchAgent that runs the
//! launcher with `--background` when the user logs in.

use std::path::{Path, PathBuf};

use super::MacPlatform;
use super::file_manager::{ignore_missing, xml_escape};
use crate::{Platform, PlatformError};

/// Our agent, named after the app id; nothing else in the folder is touched.
const AGENT: &str = "Library/LaunchAgents/org.byteveda.Carrova.plist";

impl MacPlatform {
    pub(super) fn launch_agent_on(&self) -> Result<bool, PlatformError> {
        Ok(std::fs::symlink_metadata(self.agent_path()?).is_ok())
    }

    pub(super) fn write_launch_agent(
        &self,
        launcher: &Path,
        on: bool,
    ) -> Result<bool, PlatformError> {
        let path = self.agent_path()?;
        if on {
            let exe = launcher
                .to_str()
                .filter(|l| l.starts_with('/') && !l.chars().any(char::is_control))
                .ok_or(PlatformError::Unsupported("launcher path"))?;
            self.write_atomic(&path, &agent(exe))?;
        } else {
            ignore_missing(self.remove_file(&path))?;
        }
        self.launch_agent_on()
    }

    fn agent_path(&self) -> Result<PathBuf, PlatformError> {
        Ok(self.home()?.join(AGENT))
    }
}

fn agent(launcher: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>org.byteveda.Carrova</string>
	<key>ProgramArguments</key>
	<array>
		<string>{}</string>
		<string>--background</string>
	</array>
	<key>RunAtLoad</key>
	<true/>
</dict>
</plist>
"#,
        xml_escape(launcher)
    )
}
