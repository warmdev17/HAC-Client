use crate::files::Change;
use std::path::Path;

pub const SERVICE: &str = "com.hac.codexmanager";
pub const ACCOUNT: &str = "HAC_API_KEY";

#[cfg(any(target_os = "macos", test))]
pub mod mac_files;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

pub trait System {
    fn supported(&self) -> bool;
    fn key(&self) -> Result<Option<String>, String>;
    fn save_key(&self, key: Option<&str>) -> Result<(), String>;
    fn environment(&self) -> Result<Option<String>, String>;
    fn set_environment(&self, key: Option<&str>) -> Result<(), String>;
    fn changes(&self, home: &Path) -> Result<Vec<Change>, String>;
    fn activate(&self, home: &Path) -> Result<(), String>;
    fn persistence_ready(&self, home: &Path) -> Result<bool, String> {
        Ok(self.changes(home)?.iter().all(|change| !change.changed()))
    }
    fn deactivate(&self, _home: &Path) -> Result<(), String> {
        Ok(())
    }
}

pub struct NativeSystem;

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, ACCOUNT)
        .map_err(|_| "Không mở được kho khóa của hệ điều hành".into())
}
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn key() -> Result<Option<String>, String> {
    match entry()?.get_password() {
        Ok(key) => Ok(Some(key)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(
            "Không đọc được kho khóa. Hãy mở khóa Keychain/Credential Manager và thử lại.".into(),
        ),
    }
}
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn save_key(key: Option<&str>) -> Result<(), String> {
    let entry = entry()?;
    let result = match key {
        Some(key) => entry.set_password(key),
        None => entry.delete_credential(),
    };
    match result {
        Ok(()) | Err(keyring::Error::NoEntry) if key.is_none() => Ok(()),
        Ok(()) => Ok(()),
        Err(_) => Err(
            "Không lưu được API key. Kiểm tra quyền truy cập Keychain/Credential Manager.".into(),
        ),
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
impl System for NativeSystem {
    fn supported(&self) -> bool {
        false
    }
    fn key(&self) -> Result<Option<String>, String> {
        Err("Bản này hỗ trợ macOS và Windows native.".into())
    }
    fn save_key(&self, _: Option<&str>) -> Result<(), String> {
        Err("Hệ điều hành chưa được hỗ trợ".into())
    }
    fn environment(&self) -> Result<Option<String>, String> {
        self.key()
    }
    fn set_environment(&self, _: Option<&str>) -> Result<(), String> {
        Err("Hệ điều hành chưa được hỗ trợ".into())
    }
    fn changes(&self, _: &Path) -> Result<Vec<Change>, String> {
        Ok(vec![])
    }
    fn activate(&self, _: &Path) -> Result<(), String> {
        Ok(())
    }
}
