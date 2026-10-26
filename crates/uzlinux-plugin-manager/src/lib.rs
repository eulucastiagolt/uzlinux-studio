pub mod discovery;
pub mod process;
pub mod installer;

pub use discovery::{PluginDiscovery, InstalledPlugin};
pub use process::ProcessManager;
pub use installer::PluginInstaller;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_discover_real_plugins() {
        let plugins_dir = PathBuf::from("/home/lucastiago/.var/app/me.amankhanna.opendeck/config/opendeck/plugins");
        if plugins_dir.exists() {
            let plugins = PluginDiscovery::scan_directory(&plugins_dir).expect("Should scan real plugins dir");
            assert!(!plugins.is_empty(), "Should discover installed plugins");

            let uuids: Vec<String> = plugins.iter().map(|p| p.uuid.clone()).collect();
            assert!(uuids.iter().any(|u| u.contains("starterpack")), "Should discover starterpack plugin");
            assert!(uuids.iter().any(|u| u.contains("ulanzi")), "Should discover ulanzi plugin");

            // Check executable resolution
            let starterpack = plugins.iter().find(|p| p.uuid.contains("starterpack")).unwrap();
            let exec = starterpack.manifest.get_linux_executable();
            assert!(exec.is_some(), "Linux executable should be resolved");
        }
    }
}
