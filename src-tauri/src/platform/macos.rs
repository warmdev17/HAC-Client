use super::{mac_files, NativeSystem, System};
use crate::{config::ENV_KEY, files::Change};
use std::{
    path::Path,
    process::{Command, Stdio},
};

fn launch(args: &[&str]) -> Result<(), String> {
    Command::new("/usr/bin/launchctl")
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| "Không chạy được launchctl")?
        .success()
        .then_some(())
        .ok_or_else(|| {
            "launchctl thất bại. Hãy chạy HAC trong phiên đăng nhập desktop của bạn.".into()
        })
}
fn domain() -> Result<String, String> {
    let out = Command::new("/usr/bin/id")
        .arg("-u")
        .output()
        .map_err(|_| "Không lấy được user ID")?;
    let id = String::from_utf8(out.stdout).map_err(|_| "User ID không hợp lệ")?;
    let uid: u32 = id.trim().parse().map_err(|_| "User ID không hợp lệ")?;
    Ok(format!("gui/{uid}"))
}
impl System for NativeSystem {
    fn supported(&self) -> bool {
        true
    }
    fn key(&self) -> Result<Option<String>, String> {
        super::key()
    }
    fn save_key(&self, key: Option<&str>) -> Result<(), String> {
        super::save_key(key)
    }
    fn environment(&self) -> Result<Option<String>, String> {
        let out = Command::new("/usr/bin/launchctl")
            .args(["getenv", ENV_KEY])
            .output()
            .map_err(|_| "Không đọc được môi trường launchctl")?;
        // launchctl returns nonzero when the variable is absent.
        if !out.status.success() {
            return Ok(None);
        }
        let key = String::from_utf8(out.stdout).map_err(|_| "Biến môi trường không hợp lệ")?;
        let key = key.trim_end_matches(['\r', '\n']).to_owned();
        Ok((!key.is_empty()).then_some(key))
    }
    fn set_environment(&self, key: Option<&str>) -> Result<(), String> {
        match key {
            Some(key) => launch(&["setenv", ENV_KEY, key]),
            None => launch(&["unsetenv", ENV_KEY]),
        }
    }
    fn changes(&self, home: &Path) -> Result<Vec<Change>, String> {
        mac_files::changes(home)
    }
    fn persistence_ready(&self, home: &Path) -> Result<bool, String> {
        if mac_files::changes(home)?.iter().any(Change::changed) {
            return Ok(false);
        }
        let service = format!("{}/com.hac.codexmanager.environment", domain()?);
        Ok(launch(&["print", &service]).is_ok())
    }
    fn activate(&self, home: &Path) -> Result<(), String> {
        let domain = domain()?;
        let path = mac_files::agent_path(home);
        let path = path.to_str().ok_or("Đường dẫn LaunchAgent không hợp lệ")?;
        // Unload an earlier registration if present, then load the current file.
        let _ = launch(&["bootout", &domain, path]);
        launch(&["bootstrap", &domain, path])
    }
    fn deactivate(&self, home: &Path) -> Result<(), String> {
        let domain = domain()?;
        let path = mac_files::agent_path(home);
        let _ = launch(&[
            "bootout",
            &domain,
            path.to_str().ok_or("Đường dẫn không hợp lệ")?,
        ]);
        Ok(())
    }
}
