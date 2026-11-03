use super::{linux_files, NativeSystem, System};
use crate::{config::ENV_KEY, files::Change};
use std::{
    path::Path,
    process::{Command, Stdio},
};
use zeroize::Zeroizing;

fn manager_environment(bytes: &[u8]) -> Result<Option<String>, String> {
    #[derive(serde::Deserialize)]
    struct Property {
        r#type: String,
        data: Vec<String>,
    }
    let property: Property =
        serde_json::from_slice(bytes).map_err(|_| "Không đọc được môi trường systemd user")?;
    if property.r#type != "as" {
        return Err("Dữ liệu môi trường systemd không hợp lệ".into());
    }
    let values = Zeroizing::new(property.data);
    Ok(values
        .iter()
        .find_map(|v| v.strip_prefix("HAC_API_KEY=").map(str::to_owned)))
}

impl System for NativeSystem {
    fn supported(&self) -> bool {
        std::env::var_os("WSL_DISTRO_NAME").is_none()
            && !std::fs::read_to_string("/proc/sys/kernel/osrelease")
                .unwrap_or_default()
                .to_lowercase()
                .contains("microsoft")
    }
    fn key(&self) -> Result<Option<String>, String> {
        super::key()
    }
    fn save_key(&self, key: Option<&str>) -> Result<(), String> {
        super::save_key(key)
    }
    fn environment(&self) -> Result<Option<String>, String> {
        let output = Command::new("busctl")
            .args([
                "--user",
                "--json=short",
                "get-property",
                "org.freedesktop.systemd1",
                "/org/freedesktop/systemd1",
                "org.freedesktop.systemd1.Manager",
                "Environment",
            ])
            .stderr(Stdio::null())
            .output()
            .map_err(|_| "Cần busctl và phiên systemd --user đang hoạt động")?;
        let bytes = Zeroizing::new(output.stdout);
        if !output.status.success() {
            return Err(
                "Không truy cập được systemd --user; chạy HAC trong phiên desktop của bạn".into(),
            );
        }
        manager_environment(&bytes)
    }
    fn set_environment(&self, key: Option<&str>) -> Result<(), String> {
        let mut command = Command::new("systemctl");
        command.arg("--user");
        if let Some(key) = key {
            // Import from the child environment so the key never appears in argv.
            command
                .args(["import-environment", ENV_KEY])
                .env(ENV_KEY, key);
        } else {
            command.args(["unset-environment", ENV_KEY]);
        }
        let status = command
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|_| "Không chạy được systemctl")?;
        if status.success() {
            Ok(())
        } else {
            Err("Không cập nhật được môi trường systemd user".into())
        }
    }
    fn changes(&self, home: &Path) -> Result<Vec<Change>, String> {
        let ready = Command::new("/bin/sh")
            .args([
                "-c",
                "command -v secret-tool && command -v systemctl && command -v busctl",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|_| "Không kiểm tra được công cụ Linux")?;
        if !ready.success() {
            return Err("Cần secret-tool (libsecret-tools), systemctl và busctl trên Linux".into());
        }
        linux_files::changes(home)
    }
    fn activate(&self, _: &Path) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_manager_json_without_shell_unescaping_or_echoing_secrets() {
        let value = "key'\"$`\\=unicode-é";
        let json =
            serde_json::json!({"type":"as", "data":["OTHER=x",format!("HAC_API_KEY={value}")]});
        assert_eq!(
            manager_environment(&serde_json::to_vec(&json).unwrap()).unwrap(),
            Some(value.into())
        );
        assert_eq!(
            manager_environment(br#"{"type":"as","data":["OTHER=x"]}"#).unwrap(),
            None
        );
        assert!(manager_environment(br#"{"type":"s","data":["secret"]}"#).is_err());
        assert!(!manager_environment(b"secret invalid json")
            .unwrap_err()
            .contains("secret"));
    }
}
