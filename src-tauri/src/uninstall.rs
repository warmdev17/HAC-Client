//! Only restore files whose current bytes still match what HAC wrote.
use crate::{
    config,
    files::{self, Change},
    platform::System,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Serialize, Deserialize)]
struct Entry {
    path: PathBuf,
    before: Option<Vec<u8>>,
    after: Vec<u8>,
}
#[derive(Default, Serialize, Deserialize)]
struct Journal {
    entries: Vec<Entry>,
    backups: Vec<PathBuf>,
    #[serde(default)]
    directories: Vec<PathBuf>,
}
fn journal_path(codex: &Path) -> PathBuf {
    codex.join(".hac-restore.json")
}
fn decode(bytes: &[u8]) -> Result<Journal, String> {
    serde_json::from_slice(bytes)
        .map_err(|_| "Nhật ký khôi phục HAC không hợp lệ. Chưa gỡ dữ liệu.".into())
}

pub fn journal_change(
    codex: &Path,
    changes: &[Change],
    backups: &[String],
) -> Result<Change, String> {
    let path = journal_path(codex);
    let before = files::read(&path)?;
    let mut journal = match &before {
        Some(b) => decode(b)?,
        None => Journal::default(),
    };
    for change in changes {
        if let Some(entry) = journal.entries.iter_mut().find(|e| e.path == change.path) {
            if change.before.as_deref() != Some(entry.after.as_slice()) {
                return Err(format!(
                    "File đã được sửa ngoài HAC: {}. Khôi phục bản HAC trước khi thiết lập lại.",
                    change.path.display()
                ));
            }
            entry.after = change.after.clone();
        } else if change.changed() {
            journal.entries.push(Entry {
                path: change.path.clone(),
                before: change.before.clone(),
                after: change.after.clone(),
            });
        }
    }
    for change in changes {
        let mut parent = change.path.parent();
        while let Some(path) = parent {
            if path.exists() {
                break;
            }
            if !journal.directories.iter().any(|d| d == path) {
                journal.directories.push(path.to_path_buf());
            }
            parent = path.parent();
        }
    }
    journal.backups.extend(backups.iter().map(PathBuf::from));
    let after = serde_json::to_vec(&journal).map_err(|_| "Không tạo được nhật ký HAC")?;
    Ok(Change {
        path,
        before,
        after,
    })
}

fn backup_candidates(path: &Path) -> Result<Vec<PathBuf>, String> {
    let parent = path.parent().ok_or("Đường dẫn không hợp lệ")?;
    if !parent.exists() {
        return Ok(vec![]);
    }
    let prefix = format!(
        "{}.hac-backup-",
        path.file_name()
            .ok_or("Tên file không hợp lệ")?
            .to_string_lossy()
    );
    let mut result = vec![];
    for entry in fs::read_dir(parent).map_err(|_| "Không đọc được thư mục backup")? {
        let entry = entry.map_err(|_| "Không đọc được backup")?;
        if entry.file_name().to_string_lossy().starts_with(&prefix) {
            result.push(entry.path());
        }
    }
    Ok(result)
}

fn strip_config(bytes: &[u8], profile: bool) -> Result<Option<Vec<u8>>, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "Cấu hình không phải UTF-8")?;
    let mut doc: toml_edit::DocumentMut = text
        .parse()
        .map_err(|_| "TOML không hợp lệ; chưa gỡ dữ liệu")?;
    if doc.get("model_provider").and_then(toml_edit::Item::as_str) == Some("hac") {
        for key in [
            "model",
            "model_provider",
            "model_reasoning_effort",
            "forced_login_method",
        ] {
            doc.remove(key);
        }
    }
    if !profile {
        if let Some(providers) = doc
            .get_mut("model_providers")
            .and_then(toml_edit::Item::as_table_mut)
        {
            providers.remove("hac");
            if providers.is_empty() {
                doc.remove("model_providers");
            }
        }
        // Without a pre-install snapshot these values may belong to the user. Keep them.
    }
    let output = doc.to_string();
    Ok((!output.trim().is_empty()).then(|| output.into_bytes()))
}

fn strip_shell(bytes: &[u8]) -> Result<Option<Vec<u8>>, String> {
    const START: &str = "# >>> HAC managed environment >>>";
    const END: &str = "# <<< HAC managed environment <<<";
    let text = std::str::from_utf8(bytes).map_err(|_| "Shell config không phải UTF-8")?;
    if !text.contains(START) && !text.contains(END) {
        return Ok(Some(bytes.to_vec()));
    }
    if text.matches(START).count() != 1 || text.matches(END).count() != 1 {
        return Err("Dấu đánh dấu HAC bị lỗi; chưa gỡ shell config".into());
    }
    let start = text.find(START).unwrap();
    let end = text.find(END).unwrap();
    if end < start {
        return Err("Dấu đánh dấu HAC bị lỗi".into());
    }
    let tail = &text[end + END.len()..];
    let output = format!(
        "{}{}",
        &text[..start],
        tail.strip_prefix('\n').unwrap_or(tail)
    );
    Ok((!output.trim().is_empty()).then(|| output.into_bytes()))
}

struct Restore {
    path: PathBuf,
    current: Option<Vec<u8>>,
    original: Option<Vec<u8>>,
}
fn write_state(path: &Path, bytes: &Option<Vec<u8>>) -> Result<(), String> {
    match bytes {
        Some(b) => files::atomic_write(path, b),
        None => match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(format!("Không xóa được {}", path.display())),
        },
    }
}

pub fn remove(system: &impl System, codex: &Path, user: &Path) -> Result<String, String> {
    if !system.supported() {
        return Err("Chỉ hỗ trợ macOS và Windows native.".into());
    }
    let mut known = vec![
        Change::text(codex.join("config.toml"), |s| config::render(s, false))?,
        Change::text(codex.join("hac.config.toml"), |s| config::render(s, true))?,
    ];
    known.extend(system.changes(user)?);
    let journal_file = journal_path(codex);
    let journal_bytes = files::read(&journal_file)?;
    let mut restores = vec![];
    let mut legacy = false;
    let mut directories = vec![];
    if let Some(bytes) = &journal_bytes {
        let journal = decode(bytes)?;
        for directory in &journal.directories {
            if !known.iter().any(|c| c.path.starts_with(directory))
                || directory == user
                || directory.parent().is_none()
            {
                return Err("Thư mục trong nhật ký không hợp lệ".into());
            }
        }
        directories = journal.directories;
        for entry in journal.entries {
            if !known.iter().any(|c| c.path == entry.path) {
                return Err("Đường dẫn trong nhật ký không khớp môi trường hiện tại. Mở HAC với CODEX_HOME/ZDOTDIR/XDG_CONFIG_HOME đã dùng khi thiết lập.".into());
            }
            let current = files::read(&entry.path)?;
            if current.as_deref() != Some(entry.after.as_slice()) && current != entry.before {
                return Err(format!("File đã thay đổi sau khi thiết lập: {}. Chưa gỡ; đối chiếu backup để không mất chỉnh sửa mới.", entry.path.display()));
            }
            restores.push(Restore {
                path: entry.path,
                current,
                original: entry.before,
            });
        }
        for backup in journal.backups {
            if !known.iter().any(|c| {
                backup.parent() == c.path.parent()
                    && backup.file_name().is_some_and(|n| {
                        n.to_string_lossy().starts_with(&format!(
                            "{}.hac-backup-",
                            c.path.file_name().unwrap().to_string_lossy()
                        ))
                    })
            }) {
                return Err("Đường dẫn backup trong nhật ký không hợp lệ".into());
            }
            let current = files::read(&backup)?;
            restores.push(Restore {
                path: backup,
                current,
                original: None,
            });
        }
        restores.push(Restore {
            path: journal_file,
            current: journal_bytes,
            original: None,
        });
    } else {
        legacy = true;
        // Older versions have no ownership journal. Restore only unambiguous backups.
        for (index, change) in known.iter().enumerate() {
            let Some(current) = &change.before else {
                continue;
            };
            let backups = backup_candidates(&change.path)?;
            if backups.len() > 1 {
                return Err(format!("Có nhiều backup cho {}. Chọn và khôi phục bản trước HAC thủ công trước khi gỡ; không tự đoán.", change.path.display()));
            }
            let original = if let Some(backup) = backups.first() {
                let before = files::read(backup)?.ok_or("Backup đã biến mất")?;
                let before_text =
                    std::str::from_utf8(&before).map_err(|_| "Backup không phải UTF-8")?;
                let expected = if index < 2 {
                    config::render(before_text, index == 1)?.into_bytes()
                } else {
                    // A full backup may only replace an untouched managed shell block.
                    if change.path.extension().is_some_and(|e| e == "plist")
                        || change
                            .path
                            .file_name()
                            .is_some_and(|n| n == "environment.sh")
                    {
                        change.after.clone()
                    } else {
                        if strip_shell(current)? != strip_shell(&before)? {
                            return Err(format!(
                                "Shell config đã thay đổi ngoài HAC: {}",
                                change.path.display()
                            ));
                        }
                        change.after.clone()
                    }
                };
                if &expected != current {
                    return Err(format!(
                        "{} đã được chỉnh sửa; chưa gỡ để tránh mất dữ liệu.",
                        change.path.display()
                    ));
                }
                restores.push(Restore {
                    path: backup.clone(),
                    current: Some(before.clone()),
                    original: None,
                });
                Some(before)
            } else if index < 2 {
                strip_config(current, index == 1)?
            } else if change.path.extension().is_some_and(|e| e == "plist")
                || change
                    .path
                    .file_name()
                    .is_some_and(|n| n == "environment.sh")
            {
                if current != &change.after {
                    return Err("File hỗ trợ HAC đã được sửa; chưa xóa".into());
                }
                None
            } else {
                strip_shell(current)?
            };
            restores.push(Restore {
                path: change.path.clone(),
                current: Some(current.clone()),
                original,
            });
        }
    }
    let key = system.key()?.map(zeroize::Zeroizing::new);
    let env = system.environment()?.map(zeroize::Zeroizing::new);
    let persistent = system.persistence_ready(user)?;
    let mut written = 0;
    let result: Result<(), String> = (|| {
        system.deactivate(user)?;
        system.set_environment(None)?;
        system.save_key(None)?;
        for restore in &restores {
            if files::read(&restore.path)? != restore.current {
                return Err("File vừa thay đổi trong lúc gỡ".into());
            }
            write_state(&restore.path, &restore.original)?;
            written += 1;
        }
        if system.key()?.is_some() || system.environment()?.is_some() {
            return Err("Hệ điều hành chưa xác nhận đã xóa key/môi trường".into());
        }
        Ok(())
    })();
    if let Err(error) = result {
        let mut ok = true;
        for restore in restores[..written].iter().rev() {
            ok &= write_state(&restore.path, &restore.current).is_ok();
        }
        ok &= system.save_key(key.as_deref().map(String::as_str)).is_ok();
        ok &= system
            .set_environment(env.as_deref().map(String::as_str))
            .is_ok();
        if persistent {
            ok &= system.activate(user).is_ok();
        }
        return Err(format!(
            "{error}. {}",
            if ok {
                "Đã khôi phục trạng thái trước khi gỡ."
            } else {
                "Khôi phục chưa đầy đủ; kiểm tra file và kho khóa trước khi thử lại."
            }
        ));
    }
    // Only remove empty directories recorded as created by HAC; never recurse.
    directories.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    for directory in directories {
        let _ = fs::remove_dir(directory);
    }
    Ok(if legacy {
        "Đã gỡ key, biến môi trường và các mục HAC xác định được. Bản cũ thiếu nhật ký: cài đặt memories không có backup được giữ lại; không thể xác minh khôi phục toàn bộ trạng thái trước HAC. Mở lại Codex và terminal."
    } else {
        "Đã gỡ key, biến môi trường, khôi phục file và xóa backup/nhật ký do phiên HAC này quản lý. Mở lại Codex và terminal."
    }.into())
}
