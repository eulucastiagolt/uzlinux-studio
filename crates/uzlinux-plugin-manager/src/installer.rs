use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result, anyhow};
use log::info;
use zip::ZipArchive;

pub struct PluginInstaller;

impl PluginInstaller {
    /// Installs a `.streamDeckPlugin` (zip format) into the specified plugins destination directory.
    pub fn install_package<P: AsRef<Path>, D: AsRef<Path>>(
        package_path: P,
        dest_dir: D,
    ) -> Result<PathBuf> {
        let package_path = package_path.as_ref();
        let dest_dir = dest_dir.as_ref();

        let file = fs::File::open(package_path)
            .with_context(|| format!("Failed to open plugin package {:?}", package_path))?;
        let mut archive = ZipArchive::new(file)
            .with_context(|| "Failed to read zip archive")?;

        // Determine target root directory name inside dest_dir
        // Plugins in zip usually have a top-level dir like `com.vendor.plugin.sdPlugin/`
        let mut root_dir_name: Option<String> = None;

        for i in 0..archive.len() {
            let file = archive.by_index(i)?;
            let enclosed = file.enclosed_name().ok_or_else(|| {
                anyhow!("Zip contains unsafe path traversal: {}", file.name())
            })?;

            if let Some(first_comp) = enclosed.components().next() {
                let name = first_comp.as_os_str().to_string_lossy().to_string();
                if name.ends_with(".sdPlugin") {
                    root_dir_name = Some(name);
                    break;
                }
            }
        }

        let target_dir = if let Some(ref name) = root_dir_name {
            dest_dir.join(name)
        } else {
            // Fallback: derive from package filename
            let stem = package_path.file_stem().unwrap_or_default().to_string_lossy();
            let name = if stem.ends_with(".sdPlugin") {
                stem.to_string()
            } else {
                format!("{}.sdPlugin", stem)
            };
            dest_dir.join(name)
        };

        fs::create_dir_all(&target_dir)?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let enclosed = match file.enclosed_name() {
                Some(p) => p.to_owned(),
                None => continue,
            };

            // Calculate extraction path relative to root_dir_name or directly
            let outpath = if let Some(ref root_name) = root_dir_name {
                if let Ok(rel) = enclosed.strip_prefix(root_name) {
                    target_dir.join(rel)
                } else {
                    target_dir.join(&enclosed)
                }
            } else {
                target_dir.join(&enclosed)
            };

            if file.name().ends_with('/') {
                fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        fs::create_dir_all(p)?;
                    }
                }
                let mut outfile = fs::File::create(&outpath)?;
                io::copy(&mut file, &mut outfile)?;

                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Some(mode) = file.unix_mode() {
                        let _ = fs::set_permissions(&outpath, fs::Permissions::from_mode(mode));
                    }
                }
            }
        }

        info!("Successfully installed plugin to {:?}", target_dir);
        Ok(target_dir)
    }

    /// Uninstalls (deletes) a plugin directory.
    pub fn uninstall<P: AsRef<Path>>(plugin_dir: P) -> Result<()> {
        let p = plugin_dir.as_ref();
        if p.exists() {
            fs::remove_dir_all(p)
                .with_context(|| format!("Failed to delete plugin dir {:?}", p))?;
        }
        Ok(())
    }
}
