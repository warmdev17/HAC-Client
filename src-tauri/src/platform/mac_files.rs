use crate::files::Change;
use std::path::{Path, PathBuf};

const START: &str = "# >>> HAC managed environment >>>";
const END: &str = "# <<< HAC managed environment <<<";
const SERVICE: &str = super::SERVICE;
const ACCOUNT: &str = super::ACCOUNT;

pub fn helper_path(home: &Path) -> PathBuf {
    home.join("Library/Application Support/HAC/environment.sh")
}
pub fn agent_path(home: &Path) -> PathBuf {
    home.join("Library/LaunchAgents/com.hac.codexmanager.environment.plist")
}

fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn managed(input: &str, block: &str) -> Result<String, String> {
    let starts: Vec<_> = input.match_indices(START).collect();
    let ends: Vec<_> = input.match_indices(END).collect();
    match (starts.as_slice(), ends.as_slice()) {
        ([], []) => Ok(format!("{}{}{}\n{}\n{}\n", input, if input.is_empty() || input.ends_with('\n') { "" } else { "\n" }, START, block, END)),
        ([(start, _)], [(end, _)]) if start < end => Ok(format!("{}{}\n{}\n{}{}", &input[..*start], START, block, END, &input[*end + END.len()..])),
        _ => Err("Đoạn môi trường HAC trong shell config không hợp lệ. Sửa các dấu đánh dấu HAC rồi thử lại.".into()),
    }
}

pub fn changes(home: &Path) -> Result<Vec<Change>, String> {
    // Never interpolate a key into a script, plist, shell config, or command line here.
    let shell = format!("if [ -z \"${{HAC_API_KEY:-}}\" ]; then\n  HAC_API_KEY=$(/usr/bin/security find-generic-password -s '{SERVICE}' -a '{ACCOUNT}' -w 2>/dev/null)\n  if [ -n \"$HAC_API_KEY\" ]; then export HAC_API_KEY; else unset HAC_API_KEY; fi\nfi");
    let fish = format!("if not set -q HAC_API_KEY; or test -z \"$HAC_API_KEY\"\n  set -l hac_key (/usr/bin/security find-generic-password -s '{SERVICE}' -a '{ACCOUNT}' -w 2>/dev/null)\n  if test -n \"$hac_key\"\n    set -gx HAC_API_KEY \"$hac_key\"\n  end\nend");
    let helper = format!("#!/bin/sh\n# Reads the current key on every login; contains no credential.\nhac_key=$(/usr/bin/security find-generic-password -s '{SERVICE}' -a '{ACCOUNT}' -w 2>/dev/null) || exit 1\n[ -n \"$hac_key\" ] || exit 1\n/usr/bin/launchctl setenv HAC_API_KEY \"$hac_key\" >/dev/null 2>&1\n");
    let plist = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict>\n<key>Label</key><string>com.hac.codexmanager.environment</string>\n<key>ProgramArguments</key><array><string>/bin/sh</string><string>{}</string></array>\n<key>RunAtLoad</key><true/>\n<key>LimitLoadToSessionType</key><string>Aqua</string>\n</dict></plist>\n", xml(&helper_path(home).to_string_lossy()));
    let mut changes = vec![
        Change::text(helper_path(home), |_| Ok(helper))?,
        Change::text(agent_path(home), |_| Ok(plist))?,
    ];
    // Respect zsh's custom startup directory when inherited by HAC.
    let zsh_dir = std::env::var_os("ZDOTDIR")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home.to_path_buf());
    changes.push(Change::text(zsh_dir.join(".zshenv"), |s| {
        managed(s, &shell)
    })?);
    changes.push(Change::text(home.join(".bashrc"), |s| managed(s, &shell))?);
    // Bash reads only the first existing login file; do not shadow .bash_login/.profile.
    let login = [".bash_profile", ".bash_login", ".profile"]
        .iter()
        .map(|n| home.join(n))
        .find(|p| p.exists())
        .unwrap_or_else(|| home.join(".bash_profile"));
    changes.push(Change::text(login, |s| managed(s, &shell))?);
    let fish_dir = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home.join(".config"));
    changes.push(Change::text(fish_dir.join("fish/conf.d/hac.fish"), |s| {
        managed(s, &fish)
    })?);
    Ok(changes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn managed_blocks_are_idempotent_and_keep_user_content() {
        let a = managed("# my shell\nalias ll='ls -l'", "first").unwrap();
        let b = managed(&a, "second").unwrap();
        assert!(b.contains("alias ll='ls -l'"));
        assert!(!b.contains("first"));
        assert_eq!(managed(&b, "second").unwrap(), b);
        assert!(managed(START, "x").is_err());
    }
    #[test]
    fn launchagent_escapes_paths_and_does_not_embed_a_key() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("A & B's home");
        let files = changes(&home).unwrap();
        let plist = String::from_utf8(files[1].after.clone()).unwrap();
        assert!(plist.contains("A &amp; B&apos;s home"));
        assert!(plist.contains("<key>RunAtLoad</key><true/>"));
        assert!(files
            .iter()
            .all(|f| !String::from_utf8_lossy(&f.after).contains("sk-secret")));
    }
}
