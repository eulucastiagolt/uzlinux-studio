use std::path::Path;
use std::process::Command;

pub const UDEV_RULE_FILE: &str = "/etc/udev/rules.d/60-ulanzi-d200.rules";

pub const UDEV_RULE_CONTENT: &str = r#"# Ulanzi D200 / D200X / D200H Stream Controller Udev Rules
# Automatically installed and configured by Uzlinux Studio

# Operational Composite HID interface (Rockchip 2207:0019)
KERNEL=="hidraw*", SUBSYSTEM=="hidraw", ATTRS{idVendor}=="2207", ATTRS{idProduct}=="0019", MODE="0666", TAG+="uaccess"
SUBSYSTEM=="usb", ATTRS{idVendor}=="2207", ATTRS{idProduct}=="0019", MODE="0666", TAG+="uaccess"

# Ulanzi D200 Recovery/ADB Mode (Google/Zkswe 18d1:d002)
SUBSYSTEM=="usb", ATTRS{idVendor}=="18d1", ATTRS{idProduct}=="d002", MODE="0666", TAG+="uaccess"
"#;

/// Checks if an Ulanzi udev rule exists in either /etc/udev/rules.d or /usr/lib/udev/rules.d
pub fn is_udev_rule_installed() -> bool {
    let candidates = [
        UDEV_RULE_FILE,
        "/etc/udev/rules.d/99-ulanzi.rules",
        "/etc/udev/rules.d/60-ulanzi-d200x.rules",
        "/usr/lib/udev/rules.d/60-ulanzi-d200.rules",
    ];

    for path in &candidates {
        if Path::new(path).exists() {
            return true;
        }
    }
    false
}

/// Automatically installs the udev rule using pkexec (Polkit desktop GUI prompt).
/// This eliminates the need for the user to open a terminal or manually configure the system.
pub fn install_udev_rules_pkexec() -> Result<(), String> {
    let script = format!(
        "cat << 'EOF' > {}\n{}\nEOF\nudevadm control --reload-rules && udevadm trigger\n",
        UDEV_RULE_FILE,
        UDEV_RULE_CONTENT
    );

    let output = Command::new("pkexec")
        .arg("sh")
        .arg("-c")
        .arg(&script)
        .output()
        .map_err(|e| format!("Falha ao invocar pkexec: {}. Verifique se o Polkit está instalado.", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&output.stderr);
        let out = String::from_utf8_lossy(&output.stdout);
        let msg = if !err.trim().is_empty() {
            err.trim()
        } else if !out.trim().is_empty() {
            out.trim()
        } else {
            "Operação cancelada ou negada pelo usuário."
        };
        Err(format!("Falha na autorização: {}", msg))
    }
}
