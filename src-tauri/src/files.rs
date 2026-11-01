use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;

pub struct Change {
    pub path: PathBuf,
    pub before: Option<Vec<u8>>,
    pub after: Vec<u8>,
}

pub fn read(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_file() => Err(format!(
            "Không ghi đè liên kết hoặc thư mục: {}",
            path.display()
        )),
        Ok(_) => fs::read(path)
            .map(Some)
            .map_err(|_| format!("Không đọc được {}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(format!("Không truy cập được {}", path.display())),
    }
}

impl Change {
    pub fn text(
        path: PathBuf,
        transform: impl FnOnce(&str) -> Result<String, String>,
    ) -> Result<Self, String> {
        let before = read(&path)?;
        let source = std::str::from_utf8(before.as_deref().unwrap_or_default())
            .map_err(|_| format!("File không phải UTF-8: {}", path.display()))?;
        let after = transform(source)?.into_bytes();
        Ok(Self {
            path,
            before,
            after,
        })
    }
    pub fn changed(&self) -> bool {
        self.before.as_deref() != Some(self.after.as_slice())
    }
    pub fn backup(&self) -> Result<Option<PathBuf>, String> {
        if !self.changed() {
            return Ok(None);
        }
        let Some(bytes) = &self.before else {
            return Ok(None);
        };
        let parent = self.path.parent().ok_or("Đường dẫn không hợp lệ")?;
        let name = self
            .path
            .file_name()
            .ok_or("Tên file không hợp lệ")?
            .to_string_lossy();
        let mut file = tempfile::Builder::new()
            .prefix(&format!("{name}.hac-backup-"))
            .tempfile_in(parent)
            .map_err(|_| format!("Không tạo được bản sao lưu: {}", self.path.display()))?;
        restrict(file.path())?;
        file.write_all(bytes)
            .and_then(|_| file.as_file().sync_all())
            .map_err(|_| "Không ghi được bản sao lưu")?;
        let (_, path) = file.keep().map_err(|_| "Không giữ được bản sao lưu")?;
        Ok(Some(path))
    }
    pub fn apply(&self) -> Result<(), String> {
        if read(&self.path)? != self.before {
            return Err("File vừa được ứng dụng khác thay đổi. Kiểm tra rồi thử lại.".into());
        }
        if self.changed() {
            atomic_write(&self.path, &self.after)?;
        }
        Ok(())
    }
    pub fn rollback(&self) -> Result<(), String> {
        if !self.changed() {
            return Ok(());
        }
        // Do not overwrite a concurrent edit while recovering.
        if read(&self.path)?.as_deref() != Some(self.after.as_slice()) {
            return Err(
                "File đã thay đổi trong lúc khôi phục. Dùng bản sao lưu để khôi phục thủ công."
                    .into(),
            );
        }
        match &self.before {
            Some(bytes) => atomic_write(&self.path, bytes),
            None => fs::remove_file(&self.path)
                .map_err(|_| "Không xóa được file mới khi khôi phục".into()),
        }
    }
}

fn restrict(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|_| "Không đặt được quyền file")?;
    }
    let _ = path;
    Ok(())
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("Đường dẫn không hợp lệ")?;
    if !parent.exists() {
        fs::create_dir_all(parent)
            .map_err(|_| format!("Không tạo được thư mục {}", parent.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
                .map_err(|_| "Không đặt được quyền thư mục")?;
        }
    }
    let mut file = NamedTempFile::new_in(parent).map_err(|_| "Không tạo được file tạm")?;
    restrict(file.path())?;
    file.write_all(bytes)
        .and_then(|_| file.as_file().sync_all())
        .map_err(|_| "Không ghi được file cấu hình")?;
    file.persist(path)
        .map_err(|_| format!("Không thay thế được {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn backup_update_and_rollback_preserve_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, b"original\r\n").unwrap();
        let change = Change::text(path.clone(), |_| Ok("updated".into())).unwrap();
        let backup = change.backup().unwrap().unwrap();
        assert_eq!(fs::read(backup).unwrap(), b"original\r\n");
        change.apply().unwrap();
        change.rollback().unwrap();
        assert_eq!(fs::read(path).unwrap(), b"original\r\n");
    }
    #[test]
    fn refuses_concurrent_edits() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let change = Change::text(path.clone(), |_| Ok("new".into())).unwrap();
        fs::write(path, "external edit").unwrap();
        assert!(change.apply().is_err());
    }
    #[cfg(unix)]
    #[test]
    fn refuses_symlinks_and_uses_private_permissions() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        atomic_write(&path, b"new").unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let link = dir.path().join("link");
        symlink(&path, &link).unwrap();
        assert!(Change::text(link, |_| Ok("bad".into())).is_err());
    }
}
