use std::path::{Path, PathBuf};
use std::fs;
use anyhow::{Context, Result};
use log::{info, warn};
use uzlinux_core::models::PluginManifest;

#[derive(Debug, Clone)]
pub struct InstalledPlugin {
    pub dir_path: PathBuf,
    pub manifest: PluginManifest,
    pub uuid: String,
}

pub struct PluginDiscovery;

impl PluginDiscovery {
    /// Discovers all `.sdPlugin` directories in the given root plugins directory.
    pub fn scan_directory<P: AsRef<Path>>(plugins_dir: P) -> Result<Vec<InstalledPlugin>> {
        let plugins_dir = plugins_dir.as_ref();
        if !plugins_dir.exists() {
            return Ok(Vec::new());
        }

        let mut installed = Vec::new();
        for entry in fs::read_dir(plugins_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let dir_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if dir_name.ends_with(".sdPlugin") {
                    let manifest_path = path.join("manifest.json");
                    if manifest_path.exists() {
                        match Self::read_manifest(&manifest_path) {
                            Ok(manifest) => {
                                // In OpenDeck, plugin UUID corresponds to the .sdPlugin directory name
                                let uuid = dir_name.to_string();

                                info!("Discovered plugin '{}' ({}) at {:?}", manifest.name, uuid, path);
                                installed.push(InstalledPlugin {
                                    dir_path: path,
                                    manifest,
                                    uuid,
                                });
                            }
                            Err(e) => {
                                warn!("Failed to parse manifest at {:?}: {}", manifest_path, e);
                            }
                        }
                    }
                }
            }
        }

        Ok(installed)
    }

    pub fn read_manifest<P: AsRef<Path>>(manifest_path: P) -> Result<PluginManifest> {
        let content = fs::read_to_string(manifest_path.as_ref())
            .with_context(|| format!("Could not read manifest file {:?}", manifest_path.as_ref()))?;
        let manifest: PluginManifest = serde_json::from_str(&content)
            .with_context(|| format!("Could not parse manifest JSON {:?}", manifest_path.as_ref()))?;
        Ok(manifest)
    }
}
