pub mod config;
pub mod files;
pub mod platform;
pub mod uninstall;

use files::Change;
use platform::{NativeSystem, System};
use serde::{Deserialize, Serialize};
use std::{
    env,
    fs::OpenOptions,
    path::{Path, PathBuf},
};
use zeroize::Zeroizing;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupStatus {
    pub supported: bool,
    pub platform: String,
    pub config_ready: bool,
    pub profile_ready: bool,
    pub keychain_ready: bool,
    pub environment_ready: bool,
    pub persistence_ready: bool,
    pub codex_home: String,
    pub issues: Vec<String>,
}

// Input only: deliberately no Debug or Serialize implementation for secrets.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupRequest {
    pub api_key: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupResult {
    pub backups: Vec<String>,
    pub codex_home: String,
}

fn codex_home() -> Result<PathBuf, String> {
    resolve_home(
        env::var_os("CODEX_HOME").map(PathBuf::from),
        dirs::home_dir(),
    )
}

fn resolve_home(custom: Option<PathBuf>, user: Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(path) = custom {
        if !path.is_absolute() {
            return Err("CODEX_HOME phải là đường dẫn tuyệt đối, không được để trống.".into());
        }
        return Ok(path);
    }
    user.map(|h| h.join(".codex"))
        .ok_or_else(|| "Không tìm được thư mục home của người dùng.".into())
}

fn validate_key(key: &str) -> Result<(), String> {
    if key.is_empty() {
        return Err("Nhập API key trước khi lưu.".into());
    }
    if key.len() > 4096 || key.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(
            "API key không được chứa khoảng trắng, ký tự điều khiển hoặc dài hơn 4096 byte.".into(),
        );
    }
    Ok(())
}

fn inspect(system: &impl System, codex: &Path, user: &Path) -> SetupStatus {
    let mut issues = vec![];
    let mut check = |name: &str, profile: bool| match files::read(&codex.join(name)) {
        Ok(Some(bytes)) => match std::str::from_utf8(&bytes) {
            Ok(text) => config::matches(text, profile),
            Err(_) => {
                issues.push(format!("{name} không phải UTF-8."));
                false
            }
        },
        Ok(None) => false,
        Err(error) => {
            issues.push(error);
            false
        }
    };
    let config_ready = check("config.toml", false);
    let profile_ready = check("hac.config.toml", true);
    let mut status = SetupStatus {
        supported: system.supported(),
        platform: env::consts::OS.into(),
        config_ready,
        profile_ready,
        keychain_ready: false,
        environment_ready: false,
        persistence_ready: false,
        codex_home: codex.display().to_string(),
        issues,
    };
    if !system.supported() {
        return status;
    }
    let key = match system.key() {
        Ok(key) => key.map(Zeroizing::new),
        Err(error) => {
            status.issues.push(error);
            None
        }
    };
    status.keychain_ready = key.as_ref().is_some_and(|k| !k.is_empty());
    match system.environment() {
        Ok(value) => {
            let value = value.map(Zeroizing::new);
            status.environment_ready = status.keychain_ready && value == key;
        }
        Err(error) => status.issues.push(error),
    }
    match system.persistence_ready(user) {
        Ok(ready) => status.persistence_ready = ready,
        Err(error) => status.issues.push(error),
    }
    status
}

pub async fn get_status() -> Result<SetupStatus, String> {
    let work = || {
        let user = dirs::home_dir().ok_or("Không tìm được thư mục home")?;
        Ok(inspect(&NativeSystem, &codex_home()?, &user))
    };
    #[cfg(feature = "desktop")]
    {
        tauri::async_runtime::spawn_blocking(work)
            .await
            .map_err(|_| "Không kiểm tra được trạng thái")?
    }
    #[cfg(not(feature = "desktop"))]
    {
        work()
    }
}

fn apply(
    system: &impl System,
    codex: &Path,
    user: &Path,
    key: Zeroizing<String>,
) -> Result<SetupResult, String> {
    validate_key(&key)?;
    if !system.supported() {
        return Err("Bản HAC hiện hỗ trợ macOS và Windows native.".into());
    }
    // Validate all files, including shell startup files, before touching the credential store.
    let mut changes = vec![
        Change::text(codex.join("config.toml"), |s| config::render(s, false))?,
        Change::text(codex.join("hac.config.toml"), |s| config::render(s, true))?,
    ];
    changes.extend(system.changes(user)?);
    // Do not invent a pre-install baseline when upgrading an installation without a journal.
    if files::read(&codex.join(".hac-restore.json"))?.is_none()
        && changes
            .first()
            .and_then(|c| c.before.as_ref())
            .is_some_and(|b| {
                std::str::from_utf8(b).is_ok_and(|s| {
                    s.parse::<toml_edit::DocumentMut>().is_ok_and(|d| {
                        d.get("model_providers")
                            .and_then(|p| p.get("hac"))
                            .is_some()
                            || d.get("model_provider").and_then(toml_edit::Item::as_str)
                                == Some("hac")
                    })
                })
            })
    {
        return Err("Cấu hình HAC cũ chưa có nhật ký. Dùng Gỡ thiết lập HAC trước khi cài lại để không mất backup gốc.".into());
    }
    let old_key = system.key()?.map(Zeroizing::new);
    let old_env = system.environment()?.map(Zeroizing::new);
    let was_persistent = system.persistence_ready(user)?;
    let mut backups = vec![];
    for change in &changes {
        if let Some(path) = change.backup()? {
            backups.push(path.display().to_string());
        }
    }
    changes.push(uninstall::journal_change(codex, &changes, &backups)?);
    let mut applied = 0;
    let mut key_attempted = false;
    let mut env_attempted = false;
    let mut activation_attempted = false;
    let outcome: Result<(), String> = (|| {
        for change in &changes {
            change.apply()?;
            applied += 1;
        }
        key_attempted = true;
        system.save_key(Some(&key))?;
        env_attempted = true;
        system.set_environment(Some(&key))?;
        activation_attempted = true;
        system.activate(user)?;
        // Verify actual stored key and OS environment, not merely a successful command exit.
        let stored = system.key()?.map(Zeroizing::new);
        let environment = system.environment()?.map(Zeroizing::new);
        if stored.as_deref() != Some(&*key)
            || environment.as_deref() != Some(&*key)
            || !system.persistence_ready(user)?
        {
            return Err(
                "Thiết lập chưa được hệ điều hành xác nhận. Đã thử khôi phục trạng thái trước đó."
                    .into(),
            );
        }
        Ok(())
    })();
    if let Err(error) = outcome {
        let mut restored = true;
        if activation_attempted {
            restored &= system.deactivate(user).is_ok();
        }
        if key_attempted {
            restored &= system
                .save_key(old_key.as_deref().map(String::as_str))
                .is_ok();
        }
        if env_attempted {
            restored &= system
                .set_environment(old_env.as_deref().map(String::as_str))
                .is_ok();
        }
        for change in changes[..applied].iter().rev() {
            restored &= change.rollback().is_ok();
        }
        if activation_attempted && was_persistent {
            restored &= system.activate(user).is_ok();
        }
        return Err(format!(
            "{error} {}",
            if restored {
                "Các thay đổi đã được khôi phục. Có thể kiểm tra và thử lại."
            } else {
                "Chưa khôi phục đầy đủ. Kiểm tra kho khóa, biến môi trường và các file .hac-backup-* cạnh file gốc trước khi thử lại."
            }
        ));
    }
    Ok(SetupResult {
        backups,
        codex_home: codex.display().to_string(),
    })
}

pub async fn setup(request: SetupRequest) -> Result<SetupResult, String> {
    let work = move || {
        let key = Zeroizing::new(request.api_key);
        validate_key(&key)?;
        if !NativeSystem.supported() {
            return Err("Bản HAC hiện hỗ trợ macOS và Windows native.".into());
        }
        let codex = codex_home()?;
        let user = dirs::home_dir().ok_or("Không tìm được thư mục home")?;
        let _lock = setup_lock(&user)?;
        apply(&NativeSystem, &codex, &user, key)
    };
    #[cfg(feature = "desktop")]
    {
        tauri::async_runtime::spawn_blocking(work)
            .await
            .map_err(|_| "Thiết lập bị gián đoạn. Kiểm tra cấu hình và bản sao lưu.")?
    }
    #[cfg(not(feature = "desktop"))]
    {
        work()
    }
}

fn setup_lock(user: &Path) -> Result<std::fs::File, String> {
    // One writer per user, even if multiple HAC processes or CODEX_HOME directories exist.
    let lock_path = user.join(".hac-setup.lock");
    let _ = files::read(&lock_path)?; // Reject symlinks before opening the lock file.
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock = options
        .open(lock_path)
        .map_err(|_| "Không khóa được phiên thiết lập HAC")?;
    lock.try_lock()
        .map_err(|_| "Một phiên HAC khác đang thiết lập. Hãy đợi rồi thử lại.")?;
    Ok(lock)
}

pub async fn uninstall() -> Result<String, String> {
    let work = || {
        let user = dirs::home_dir().ok_or("Không tìm được thư mục home")?;
        let _lock = setup_lock(&user)?;
        uninstall::remove(&NativeSystem, &codex_home()?, &user)
    };
    #[cfg(feature = "desktop")]
    {
        tauri::async_runtime::spawn_blocking(work)
            .await
            .map_err(|_| "Gỡ bị gián đoạn; kiểm tra trạng thái trước khi thử lại")?
    }
    #[cfg(not(feature = "desktop"))]
    {
        work()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::fs;

    #[derive(Default)]
    struct MockSystem {
        key: RefCell<Option<String>>,
        env: RefCell<Option<String>>,
        fail: RefCell<Option<&'static str>>,
        calls: RefCell<Vec<&'static str>>,
    }
    impl MockSystem {
        fn step(&self, name: &'static str) -> Result<(), String> {
            self.calls.borrow_mut().push(name);
            if *self.fail.borrow() == Some(name) {
                self.fail.replace(None);
                return Err(format!("Lỗi giả lập: {name}"));
            }
            Ok(())
        }
    }
    impl System for MockSystem {
        fn supported(&self) -> bool {
            true
        }
        fn key(&self) -> Result<Option<String>, String> {
            Ok(self.key.borrow().clone())
        }
        fn save_key(&self, key: Option<&str>) -> Result<(), String> {
            self.step("credential")?;
            self.key.replace(key.map(str::to_owned));
            Ok(())
        }
        fn environment(&self) -> Result<Option<String>, String> {
            Ok(self.env.borrow().clone())
        }
        fn set_environment(&self, key: Option<&str>) -> Result<(), String> {
            self.step("environment")?;
            self.env.replace(key.map(str::to_owned));
            Ok(())
        }
        fn changes(&self, user: &Path) -> Result<Vec<Change>, String> {
            Ok(vec![Change::text(user.join("startup"), |_| {
                Ok("load key from OS store".into())
            })?])
        }
        fn activate(&self, _: &Path) -> Result<(), String> {
            self.step("activate")
        }
    }
    #[test]
    fn first_install_rotation_and_idempotent_reapply() {
        let dir = tempfile::tempdir().unwrap();
        let codex = dir.path().join(".codex");
        let system = MockSystem::default();
        let result = apply(
            &system,
            &codex,
            dir.path(),
            Zeroizing::new("sk-secret".into()),
        )
        .unwrap();
        assert!(result.backups.is_empty());
        let status = inspect(&system, &codex, dir.path());
        assert!(
            status.config_ready
                && status.profile_ready
                && status.keychain_ready
                && status.environment_ready
                && status.persistence_ready
        );
        assert!(!serde_json::to_string(&status)
            .unwrap()
            .contains("sk-secret"));
        assert!(!fs::read_to_string(codex.join("config.toml"))
            .unwrap()
            .contains("sk-secret"));
        let result = apply(
            &system,
            &codex,
            dir.path(),
            Zeroizing::new("new-secret".into()),
        )
        .unwrap();
        assert!(result.backups.is_empty());
        assert_eq!(system.env.borrow().as_deref(), Some("new-secret"));
    }
    #[test]
    fn each_os_failure_restores_files_key_and_environment() {
        for failure in ["credential", "environment", "activate"] {
            let dir = tempfile::tempdir().unwrap();
            let codex = dir.path().join(".codex");
            fs::create_dir(&codex).unwrap();
            fs::write(codex.join("config.toml"), "# original\nmodel = 'old'\n").unwrap();
            let system = MockSystem::default();
            system.key.replace(Some("old-key".into()));
            system.env.replace(Some("old-env".into()));
            system.fail.replace(Some(failure));
            let error = apply(
                &system,
                &codex,
                dir.path(),
                Zeroizing::new("secret-never-echo".into()),
            )
            .unwrap_err();
            assert!(!error.contains("secret-never-echo"));
            assert_eq!(
                fs::read_to_string(codex.join("config.toml")).unwrap(),
                "# original\nmodel = 'old'\n"
            );
            assert!(!codex.join("hac.config.toml").exists());
            assert!(!dir.path().join("startup").exists());
            assert_eq!(system.key.borrow().as_deref(), Some("old-key"));
            assert_eq!(system.env.borrow().as_deref(), Some("old-env"));
            assert!(fs::read_dir(&codex).unwrap().any(|p| p
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains("hac-backup")));
        }
    }
    #[test]
    fn invalid_profile_prevents_any_credential_or_config_write() {
        let dir = tempfile::tempdir().unwrap();
        let codex = dir.path();
        fs::write(codex.join("hac.config.toml"), "SECRET = [").unwrap();
        let system = MockSystem::default();
        assert!(apply(&system, codex, codex, Zeroizing::new("key".into())).is_err());
        assert!(system.calls.borrow().is_empty());
        assert!(!codex.join("config.toml").exists());
    }
    #[test]
    fn presence_alone_is_not_environment_readiness() {
        let dir = tempfile::tempdir().unwrap();
        let system = MockSystem::default();
        system.key.replace(Some("key-a".into()));
        system.env.replace(Some("key-b".into()));
        assert!(!inspect(&system, dir.path(), dir.path()).environment_ready);
    }
    #[test]
    fn invalid_keys_and_relative_codex_home_are_rejected() {
        for key in ["", "a\nb", "a b", "a\0b"] {
            assert!(validate_key(key).is_err());
        }
        assert!(resolve_home(Some(PathBuf::from("relative")), None).is_err());
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            resolve_home(Some(dir.path().to_path_buf()), None).unwrap(),
            dir.path()
        );
    }
    #[test]
    fn frontend_camel_case_request_is_accepted() {
        let request: SetupRequest = serde_json::from_str(r#"{"apiKey":"fake-key"}"#).unwrap();
        assert_eq!(request.api_key, "fake-key");
    }

    #[test]
    fn uninstall_restores_original_after_key_rotation_and_removes_backups() {
        let dir = tempfile::tempdir().unwrap();
        let codex = dir.path().join(".codex");
        fs::create_dir(&codex).unwrap();
        let original = "# user config\nmodel = 'original'\n[memories]\nuse_memories = false\n";
        fs::write(codex.join("config.toml"), original).unwrap();
        let system = MockSystem::default();
        apply(&system, &codex, dir.path(), Zeroizing::new("first".into())).unwrap();
        apply(&system, &codex, dir.path(), Zeroizing::new("second".into())).unwrap();
        uninstall::remove(&system, &codex, dir.path()).unwrap();
        assert_eq!(
            fs::read_to_string(codex.join("config.toml")).unwrap(),
            original
        );
        assert_eq!(fs::read_dir(&codex).unwrap().count(), 1);
        assert!(!dir.path().join("startup").exists());
        assert!(system.key.borrow().is_none());
        assert!(system.env.borrow().is_none());
    }

    #[test]
    fn uninstall_refuses_new_user_edits_without_removing_key() {
        let dir = tempfile::tempdir().unwrap();
        let codex = dir.path().join(".codex");
        let system = MockSystem::default();
        apply(&system, &codex, dir.path(), Zeroizing::new("secret".into())).unwrap();
        let path = codex.join("config.toml");
        let updated = format!("{}\n# new user edit\n", fs::read_to_string(&path).unwrap());
        fs::write(&path, &updated).unwrap();
        assert!(uninstall::remove(&system, &codex, dir.path()).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), updated);
        assert_eq!(system.key.borrow().as_deref(), Some("secret"));
    }

    #[test]
    fn failed_uninstall_restores_credentials_and_can_retry() {
        let dir = tempfile::tempdir().unwrap();
        let codex = dir.path().join(".codex");
        let system = MockSystem::default();
        apply(&system, &codex, dir.path(), Zeroizing::new("secret".into())).unwrap();
        system.fail.replace(Some("credential"));
        assert!(uninstall::remove(&system, &codex, dir.path()).is_err());
        assert_eq!(system.env.borrow().as_deref(), Some("secret"));
        assert!(codex.join(".hac-restore.json").exists());
        uninstall::remove(&system, &codex, dir.path()).unwrap();
        assert!(!codex.exists());
    }

    #[test]
    fn legacy_install_restores_single_backup_but_rejects_ambiguous_backups() {
        let dir = tempfile::tempdir().unwrap();
        let codex = dir.path();
        let original = "model = 'before-hac'\n";
        fs::write(
            codex.join("config.toml"),
            config::render(original, false).unwrap(),
        )
        .unwrap();
        fs::write(codex.join("config.toml.hac-backup-one"), original).unwrap();
        fs::write(codex.join("config.toml.hac-backup-two"), original).unwrap();
        let system = MockSystem::default();
        assert!(uninstall::remove(&system, codex, codex).is_err());
        fs::remove_file(codex.join("config.toml.hac-backup-two")).unwrap();
        uninstall::remove(&system, codex, codex).unwrap();
        assert_eq!(
            fs::read_to_string(codex.join("config.toml")).unwrap(),
            original
        );
    }
}
