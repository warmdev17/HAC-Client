use super::{NativeSystem, System};
use crate::{config::ENV_KEY, files::Change};
use std::path::Path;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
};
use winreg::{
    enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE},
    RegKey,
};

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
        let root = RegKey::predef(HKEY_CURRENT_USER);
        let env = match root.open_subkey_with_flags("Environment", KEY_READ) {
            Ok(env) => env,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("Không đọc được biến môi trường user trong Registry".into()),
        };
        match env.get_value::<String, _>(ENV_KEY) {
            Ok(key) => Ok(Some(key)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err("HAC_API_KEY trong Registry không phải chuỗi hợp lệ".into()),
        }
    }
    fn set_environment(&self, key: Option<&str>) -> Result<(), String> {
        let root = RegKey::predef(HKEY_CURRENT_USER);
        let (env, _) = root
            .create_subkey_with_flags("Environment", KEY_SET_VALUE)
            .map_err(|_| "Không mở được Registry để ghi biến môi trường user")?;
        match key {
            Some(key) => env
                .set_value(ENV_KEY, &key)
                .map_err(|_| "Không lưu được HAC_API_KEY trong Registry")?,
            None => match env.delete_value(ENV_KEY) {
                Ok(()) => (),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(_) => return Err("Không khôi phục được biến môi trường trong Registry".into()),
            },
        }
        let name: Vec<u16> = "Environment\0".encode_utf16().collect();
        // Notify Explorer; existing terminals and Codex processes still need a restart.
        unsafe {
            SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                0,
                name.as_ptr() as isize,
                SMTO_ABORTIFHUNG,
                5000,
                std::ptr::null_mut(),
            );
        }
        Ok(())
    }
    fn changes(&self, _: &Path) -> Result<Vec<Change>, String> {
        Ok(vec![])
    }
    fn activate(&self, _: &Path) -> Result<(), String> {
        Ok(())
    }
}
