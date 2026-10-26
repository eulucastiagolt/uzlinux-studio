use std::collections::VecDeque;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use anyhow::{Context, Result, anyhow};
use dashmap::DashMap;
use log::{error, info, warn};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use uzlinux_protocol::PluginInfo;

pub struct ProcessManager {
    running: Arc<DashMap<String, Arc<Mutex<Child>>>>,
    logs: Arc<DashMap<String, VecDeque<String>>>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            running: Arc::new(DashMap::new()),
            logs: Arc::new(DashMap::new()),
        }
    }

    pub fn is_running(&self, plugin_uuid: &str) -> bool {
        if self.running.contains_key(plugin_uuid) {
            return true;
        }
        if let Some(bare) = plugin_uuid.strip_suffix(".sdPlugin") {
            if self.running.contains_key(bare) {
                return true;
            }
        } else {
            let with_ext = format!("{}.sdPlugin", plugin_uuid);
            if self.running.contains_key(&with_ext) {
                return true;
            }
        }
        false
    }

    pub fn list_running(&self) -> Vec<String> {
        self.running.iter().map(|kv| kv.key().clone()).collect()
    }

    pub fn get_logs(&self, plugin_uuid: &str) -> Vec<String> {
        if let Some(entry) = self.logs.get(plugin_uuid) {
            return entry.iter().cloned().collect();
        }
        if let Some(bare) = plugin_uuid.strip_suffix(".sdPlugin") {
            if let Some(entry) = self.logs.get(bare) {
                return entry.iter().cloned().collect();
            }
        } else {
            let with_ext = format!("{}.sdPlugin", plugin_uuid);
            if let Some(entry) = self.logs.get(&with_ext) {
                return entry.iter().cloned().collect();
            }
        }
        Vec::new()
    }

    fn push_log(logs: &Arc<DashMap<String, VecDeque<String>>>, uuid: &str, line: String) {
        let mut entry = logs.entry(uuid.to_string()).or_insert_with(VecDeque::new);
        if entry.len() >= 100 {
            entry.pop_front();
        }
        entry.push_back(line);
    }

    /// Spawns a plugin executable with the Elgato arguments.
    pub async fn start_plugin(
        &self,
        plugin_uuid: &str,
        plugin_dir: &Path,
        executable_rel_path: &str,
        port: u16,
        info_json: &PluginInfo,
    ) -> Result<()> {
        if self.is_running(plugin_uuid) {
            info!("Plugin '{}' is already running", plugin_uuid);
            return Ok(());
        }

        let exec_path = plugin_dir.join(executable_rel_path);
        if !exec_path.exists() {
            return Err(anyhow!("Plugin executable does not exist at {:?}", exec_path));
        }

        // On Unix, ensure the executable bit is set
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = std::fs::metadata(&exec_path) {
                let mut perms = metadata.permissions();
                let mode = perms.mode();
                if mode & 0o111 == 0 {
                    perms.set_mode(mode | 0o755);
                    let _ = std::fs::set_permissions(&exec_path, perms);
                }
            }
        }

        let info_str = serde_json::to_string(info_json)
            .context("Failed to serialize PluginInfo")?;

        info!("Starting plugin '{}': {:?} with port {}", plugin_uuid, exec_path, port);

        let mut cmd = if exec_path.extension().and_then(|s| s.to_str()) == Some("js") {
            let mut c = Command::new("node");
            c.arg(&exec_path);
            c
        } else {
            Command::new(&exec_path)
        };

        cmd.current_dir(plugin_dir)
            .arg("-port")
            .arg(port.to_string())
            .arg("-pluginUUID")
            .arg(plugin_uuid)
            .arg("-registerEvent")
            .arg("registerPlugin")
            .arg("-info")
            .arg(&info_str)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn()
            .with_context(|| format!("Failed to spawn plugin executable {:?}", exec_path))?;

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let uuid_stdout = plugin_uuid.to_string();
        let logs_stdout = self.logs.clone();
        if let Some(stdout) = stdout {
            tokio::spawn(async move {
                let mut reader = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    info!("[Plugin: {}] {}", uuid_stdout, line);
                    Self::push_log(&logs_stdout, &uuid_stdout, format!("[out] {}", line));
                }
            });
        }

        let uuid_stderr = plugin_uuid.to_string();
        let logs_stderr = self.logs.clone();
        if let Some(stderr) = stderr {
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    warn!("[Plugin: {}] {}", uuid_stderr, line);
                    Self::push_log(&logs_stderr, &uuid_stderr, format!("[err] {}", line));
                }
            });
        }

        let child_arc = Arc::new(Mutex::new(child));
        self.running.insert(plugin_uuid.to_string(), child_arc.clone());
        if plugin_uuid.ends_with(".sdPlugin") {
            let bare = plugin_uuid.trim_end_matches(".sdPlugin").to_string();
            self.running.insert(bare, child_arc.clone());
        } else {
            let with_ext = format!("{}.sdPlugin", plugin_uuid);
            self.running.insert(with_ext, child_arc.clone());
        }

        // Spawn a watcher to detect when the process exits
        let running_map = self.running.clone();
        let logs_watch = self.logs.clone();
        let uuid_watch = plugin_uuid.to_string();
        tokio::spawn(async move {
            let mut locked = child_arc.lock().await;
            match locked.wait().await {
                Ok(status) => {
                    info!("Plugin '{}' exited with status: {}", uuid_watch, status);
                    Self::push_log(&logs_watch, &uuid_watch, format!("Process exited with status: {}", status));
                }
                Err(e) => {
                    error!("Error waiting for plugin '{}': {}", uuid_watch, e);
                    Self::push_log(&logs_watch, &uuid_watch, format!("Process error: {}", e));
                }
            }
            running_map.remove(&uuid_watch);
            if uuid_watch.ends_with(".sdPlugin") {
                let bare = uuid_watch.trim_end_matches(".sdPlugin");
                running_map.remove(bare);
            } else {
                let with_ext = format!("{}.sdPlugin", uuid_watch);
                running_map.remove(&with_ext);
            }
        });

        Ok(())
    }

    /// Stops a running plugin process.
    pub async fn stop_plugin(&self, plugin_uuid: &str) -> Result<()> {
        let child_opt = self.running.remove(plugin_uuid)
            .or_else(|| {
                plugin_uuid.strip_suffix(".sdPlugin").and_then(|bare| self.running.remove(bare))
            })
            .or_else(|| {
                let with_ext = format!("{}.sdPlugin", plugin_uuid);
                self.running.remove(&with_ext)
            });

        if let Some((_, child_arc)) = child_opt {
            let mut child = child_arc.lock().await;
            info!("Stopping plugin '{}'...", plugin_uuid);
            let _ = child.kill().await;
            Self::push_log(&self.logs, plugin_uuid, "Process stopped by user".to_string());
        }
        if plugin_uuid.ends_with(".sdPlugin") {
            let bare = plugin_uuid.trim_end_matches(".sdPlugin");
            self.running.remove(bare);
        } else {
            let with_ext = format!("{}.sdPlugin", plugin_uuid);
            self.running.remove(&with_ext);
        }
        Ok(())
    }

    /// Stops all running plugins.
    pub async fn stop_all(&self) {
        let keys: Vec<String> = self.running.iter().map(|kv| kv.key().clone()).collect();
        for k in keys {
            let _ = self.stop_plugin(&k).await;
        }
    }
}
