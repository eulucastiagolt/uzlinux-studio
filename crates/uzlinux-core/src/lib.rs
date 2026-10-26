pub mod models;
pub mod storage;

pub use models::*;
pub use storage::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_load_real_opendeck_settings() {
        let opendeck_dir = PathBuf::from("/home/lucastiago/.var/app/me.amankhanna.opendeck/config/opendeck");
        if opendeck_dir.exists() {
            let cm = ConfigManager::new(&opendeck_dir);
            let settings = cm.load_settings().expect("Should load real settings");
            assert_eq!(settings.version, "2.14.0");
            assert_eq!(settings.brightness, 21);
            assert_eq!(settings.language, "pt_BR");
        }
    }

    #[test]
    fn test_load_real_opendeck_applications() {
        let opendeck_dir = PathBuf::from("/home/lucastiago/.var/app/me.amankhanna.opendeck/config/opendeck");
        if opendeck_dir.exists() {
            let cm = ConfigManager::new(&opendeck_dir);
            let apps = cm.load_applications().expect("Should load real applications");
            let obs_profile = apps.get_profile_for_app("com.obsproject.Studio", "e9-02C53A015U3671380");
            assert_eq!(obs_profile, Some("Live"));
            let vlc_profile = apps.get_profile_for_app("vlc", "e9-02C53A015U3671380");
            assert_eq!(vlc_profile, Some("Player"));
        }
    }

    #[test]
    fn test_load_real_opendeck_profile() {
        let opendeck_dir = PathBuf::from("/home/lucastiago/.var/app/me.amankhanna.opendeck/config/opendeck");
        if opendeck_dir.exists() {
            let cm = ConfigManager::new(&opendeck_dir);
            let device_id = "e9-02C53A015U3671380";
            let profiles = cm.list_profiles(device_id).expect("Should list profiles");
            assert!(profiles.contains(&"Default".to_string()));
            assert!(profiles.contains(&"Player".to_string()));
            assert!(profiles.contains(&"Live".to_string()));

            let default_profile = cm.load_profile(device_id, "Default").expect("Should load Default profile");
            assert_eq!(default_profile.keys.len(), 15);
            let key0 = default_profile.keys[0].as_ref().expect("Key 0 should be present");
            assert_eq!(key0.context, "Keypad.0.0");
            assert_eq!(key0.action.name, "Switch Profile");
        }
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
        let cm = ConfigManager::new(temp_dir.path());
        cm.ensure_directories().expect("Failed to ensure directories");

        let mut settings = AppSettings::default();
        settings.brightness = 95;
        settings.language = "pt_BR".to_string();
        cm.save_settings(&settings).expect("Failed to save settings");

        let loaded = cm.load_settings().expect("Failed to load settings");
        assert_eq!(loaded.brightness, 95);
        assert_eq!(loaded.language, "pt_BR");
    }

    #[test]
    fn test_opendeck_scan_and_selective_import() {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
        let cm = ConfigManager::new(temp_dir.path());
        if ConfigManager::find_opendeck_dir().is_some() {
            let scan = cm.scan_opendeck().expect("Should scan opendeck");
            assert!(scan.found);
            assert!(!scan.profiles.is_empty());
            assert!(!scan.plugins.is_empty());

            // Test selective import of only 1 profile
            let target_profile = &scan.profiles[0];
            let rep = cm.import_from_opendeck_custom(&OpenDeckImportOptions {
                import_all: false,
                import_profiles: true,
                selected_profiles: vec![target_profile.rel_path.clone()],
                import_plugins: false,
                selected_plugins: vec![],
                import_applications: false,
                import_settings: false,
                import_images: false,
            }).expect("Selective import should succeed");

            assert_eq!(rep.imported_profiles, 1);
            assert_eq!(rep.imported_plugins, 0);
            assert!(cm.profiles_dir().join(&target_profile.rel_path).exists());
        }
    }
}
