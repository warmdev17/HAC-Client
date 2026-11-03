use crate::files::Change;
use std::path::Path;

fn desktop_exec(path: &Path) -> Result<String, String> {
    let path = path.to_str().ok_or("Đường dẫn Linux phải là UTF-8")?;
    if path.chars().any(char::is_control) {
        return Err("Đường dẫn Linux không được chứa ký tự điều khiển".into());
    }
    // Exec quoting is decoded once by Desktop Entry and again by its argument parser.
    let quoted = path
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('`', "\\`")
        .replace('$', "\\$")
        .replace('%', "%%");
    Ok(format!("/bin/sh \"{quoted}\"").replace('\\', "\\\\"))
}

pub fn changes(home: &Path) -> Result<Vec<Change>, String> {
    let (config, zsh) = super::shell::directories(home);
    changes_at(home, &config, &zsh)
}

pub(crate) fn changes_at(home: &Path, config: &Path, zsh: &Path) -> Result<Vec<Change>, String> {
    let lookup = format!(
        "secret-tool lookup service '{}' username '{}' target default",
        super::SERVICE,
        super::ACCOUNT
    );
    let shell = format!("if [ -z \"${{HAC_API_KEY:-}}\" ]; then\n  HAC_API_KEY=$({lookup} 2>/dev/null)\n  if [ -n \"$HAC_API_KEY\" ]; then export HAC_API_KEY; else unset HAC_API_KEY; fi\nfi");
    let fish = format!("if not set -q HAC_API_KEY; or test -z \"$HAC_API_KEY\"\n  set -l hac_key ({lookup} 2>/dev/null)\n  if test -n \"$hac_key\"\n    set -gx HAC_API_KEY \"$hac_key\"\n  end\nend");
    let helper = config.join("hac/environment.sh");
    let script = format!("#!/bin/sh\n# Loads the credential at desktop login; never contains a key.\nHAC_API_KEY=$({lookup} 2>/dev/null) || exit 1\n[ -n \"$HAC_API_KEY\" ] || exit 1\nexport HAC_API_KEY\nsystemctl --user import-environment HAC_API_KEY >/dev/null 2>&1\n");
    let desktop = format!("[Desktop Entry]\nType=Application\nName=HAC environment\nExec={}\nTerminal=false\nNoDisplay=true\n", desktop_exec(&helper)?);
    let mut result = vec![
        Change::text(helper, |_| Ok(script))?,
        Change::text(config.join("autostart/hac-environment.desktop"), |_| {
            Ok(desktop)
        })?,
    ];
    result.extend(super::shell::changes(home, config, zsh, &shell, &fish)?);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn linux_files_preserve_shell_content_and_restore_exact_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home with spaces");
        fs::create_dir(&home).unwrap();
        let config = home.join("config dir");
        let original = "# user\nalias ll='ls -l'\n";
        fs::write(home.join(".bashrc"), original).unwrap();
        let changes = changes_at(&home, &config, &home).unwrap();
        assert_eq!(changes.len(), 6);
        for change in &changes {
            let content = String::from_utf8_lossy(&change.after);
            assert!(!content.contains("sk-secret"));
            change.apply().unwrap();
        }
        assert!(fs::read_to_string(home.join(".bashrc"))
            .unwrap()
            .starts_with(original));
        assert!(changes_at(&home, &config, &home)
            .unwrap()
            .iter()
            .all(|c| !c.changed()));
        for change in changes.iter().rev() {
            change.rollback().unwrap();
        }
        assert_eq!(fs::read_to_string(home.join(".bashrc")).unwrap(), original);
        assert!(!config.join("autostart/hac-environment.desktop").exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn shell_and_login_helper_load_literal_key_without_exposing_it_in_arguments() {
        use std::os::unix::fs::PermissionsExt;
        use std::process::Command;
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let config = home.join(".config");
        let bin = home.join("bin");
        fs::create_dir(&bin).unwrap();
        for (name, script) in [
            ("secret-tool", "#!/bin/sh\n[ \"$*\" = 'lookup service com.hac.codexmanager username HAC_API_KEY target default' ] || exit 2\nprintf '%s' \"$TEST_KEY\"\n"),
            ("systemctl", "#!/bin/sh\n[ \"$*\" = '--user import-environment HAC_API_KEY' ] || exit 2\nprintf '%s' \"$HAC_API_KEY\" > \"$TEST_OUTPUT\"\n"),
        ] {
            let file = bin.join(name);
            fs::write(&file, script).unwrap();
            fs::set_permissions(file, fs::Permissions::from_mode(0o700)).unwrap();
        }
        for c in changes_at(home, &config, home).unwrap() {
            c.apply().unwrap();
        }
        let key = "fake-'\"$`\\=é";
        let output = Command::new("/bin/sh")
            .args(["-c", ". \"$1\"; printf '%s' \"$HAC_API_KEY\"", "test"])
            .arg(home.join(".bashrc"))
            .env("PATH", &bin)
            .env("TEST_KEY", key)
            .env_remove("HAC_API_KEY")
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, key.as_bytes());
        let capture = home.join("environment");
        let run = |value: &str| {
            Command::new("/bin/sh")
                .arg(config.join("hac/environment.sh"))
                .env("PATH", &bin)
                .env("TEST_KEY", value)
                .env("TEST_OUTPUT", &capture)
                .status()
                .unwrap()
        };
        assert!(run(key).success());
        assert_eq!(fs::read(&capture).unwrap(), key.as_bytes());
        fs::remove_file(&capture).unwrap();
        assert!(!run("").success());
        assert!(!capture.exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "Requires Python PyGObject and a session bus; uses only temporary files"]
    fn desktop_entry_launches_quoted_helper_path() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("A '$`%\\\" home");
        fs::create_dir(&home).unwrap();
        let config = home.join("config dir");
        let changes = changes_at(&home, &config, &home).unwrap();
        for change in &changes {
            change.apply().unwrap();
        }
        let output = home.join("launched");
        fs::write(
            config.join("hac/environment.sh"),
            "#!/bin/sh\nprintf ok > \"$HAC_TEST_OUTPUT\"\n",
        )
        .unwrap();
        let status = std::process::Command::new("python")
            .args(["-c", "from gi.repository import Gio; import sys; app=Gio.DesktopAppInfo.new_from_filename(sys.argv[1]); assert app; assert app.launch([], None)"])
            .arg(config.join("autostart/hac-environment.desktop"))
            .env("HAC_TEST_OUTPUT", &output).status().unwrap();
        assert!(status.success());
        for _ in 0..50 {
            if output.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(fs::read_to_string(output).unwrap(), "ok");
    }

    #[test]
    fn linux_rejects_broken_blocks_and_control_characters_in_paths() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(".bashrc"),
            "# >>> HAC managed environment >>>",
        )
        .unwrap();
        assert!(changes_at(dir.path(), &dir.path().join(".config"), dir.path()).is_err());
        assert!(desktop_exec(std::path::Path::new("/tmp/new\nline")).is_err());
    }
}
