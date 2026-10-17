#![allow(dead_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirmwareUpdateResponse {
    pub code: Option<i32>,
    pub succ: Option<bool>,
    pub message: Option<String>,
    pub data: Option<FirmwareUpdateData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirmwareUpdateData {
    #[serde(rename = "needUpdate")]
    pub need_update: Option<bool>,
    pub pid: Option<String>,
    pub version: Option<String>,
    #[serde(rename = "versionInfo")]
    pub version_info: Option<String>,
    #[serde(rename = "downloadUrl")]
    pub download_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FirmwareCheckResult {
    pub need_update: bool,
    pub latest_version: String,
    pub changelog: String,
    pub download_url: String,
    pub message: String,
}

/// Checks the official Ulanzi OTA server for firmware updates for the given device.
pub fn check_for_firmware_update(sn: &str, pid: &str, current_ver: &str) -> Result<FirmwareCheckResult, String> {
    let clean_sn = if sn.trim().is_empty() { "D200" } else { sn.trim() };
    let clean_pid = if pid.trim().is_empty() { "d200" } else { pid.trim() };
    let clean_ver = if current_ver.trim().is_empty() { "1.0.0" } else { current_ver.trim() };

    // Official Ulanzi cloud OTA endpoint discovered in UlanziDeck.exe reverse engineering
    let url = format!(
        "https://api.ulanzistudio.com/api/vibekey/firmware/checkUpdate?deviceSn={}&pid={}&ver={}&lang=pt-BR",
        clean_sn, clean_pid, clean_ver
    );

    let output = std::process::Command::new("curl")
        .arg("-s")
        .arg("--max-time")
        .arg("8")
        .arg(&url)
        .output()
        .map_err(|e| format!("Falha ao conectar aos servidores da Ulanzi: {}", e))?;

    if !output.status.success() {
        return Err("Falha na requisição HTTP de verificação de firmware".to_string());
    }

    let json_str = String::from_utf8_lossy(&output.stdout);
    if json_str.trim().is_empty() {
        return Err("Servidor da Ulanzi não retornou resposta".to_string());
    }

    let resp: FirmwareUpdateResponse = serde_json::from_str(&json_str)
        .map_err(|e| format!("Resposta de firmware inválida ({})", e))?;

    if let Some(data) = resp.data {
        let need_update = data.need_update.unwrap_or(false);
        let latest = data.version.unwrap_or_else(|| clean_ver.to_string());
        let changelog = data.version_info.unwrap_or_default();
        let download = data.download_url.unwrap_or_default();

        let msg = if need_update {
            format!("Nova versão disponível: v{}", latest)
        } else {
            format!("O firmware está atualizado (v{})", latest)
        };

        Ok(FirmwareCheckResult {
            need_update,
            latest_version: latest,
            changelog,
            download_url: download,
            message: msg,
        })
    } else {
        let msg = resp.message.unwrap_or_else(|| "Nenhuma atualização encontrada".to_string());
        Ok(FirmwareCheckResult {
            need_update: false,
            latest_version: clean_ver.to_string(),
            changelog: String::new(),
            download_url: String::new(),
            message: msg,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_firmware_update_endpoint() {
        let res = check_for_firmware_update("D200-TEST", "d200", "1.0.0");
        assert!(res.is_ok(), "Expected valid response from Ulanzi OTA API: {:?}", res.err());
        let info = res.unwrap();
        println!("Firmware check result: {:?}", info);
        assert!(!info.latest_version.is_empty());
    }
}
