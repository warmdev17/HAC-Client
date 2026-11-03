use crate::files::Change;
use std::path::{Path, PathBuf};

const START: &str = "# >>> HAC managed environment >>>";
const END: &str = "# <<< HAC managed environment <<<";

pub(super) fn managed(input: &str, block: &str) -> Result<String, String> {
    let starts: Vec<_> = input.match_indices(START).collect();
    let ends: Vec<_> = input.match_indices(END).collect();
    match (starts.as_slice(), ends.as_slice()) {
        ([], []) => Ok(format!("{}{}{}\n{}\n{}\n", input, if input.is_empty() || input.ends_with('\n') { "" } else { "\n" }, START, block, END)),
        ([(start, _)], [(end, _)]) if start < end => Ok(format!("{}{}\n{}\n{}{}", &input[..*start], START, block, END, &input[*end + END.len()..])),
        _ => Err("Đoạn môi trường HAC trong shell config không hợp lệ. Sửa các dấu đánh dấu HAC rồi thử lại.".into()),
    }
}

pub(super) fn changes(
    home: &Path,
    config: &Path,
    zsh: &Path,
    shell: &str,
    fish: &str,
) -> Result<Vec<Change>, String> {
    let mut changes = vec![];
    changes.push(Change::text(zsh.join(".zshenv"), |s| managed(s, shell))?);
    changes.push(Change::text(home.join(".bashrc"), |s| managed(s, shell))?);
    // Bash reads only the first existing login file; do not shadow .bash_login/.profile.
    let login = [".bash_profile", ".bash_login", ".profile"]
        .iter()
        .map(|n| home.join(n))
        .find(|p| p.exists())
        .unwrap_or_else(|| home.join(".bash_profile"));
    changes.push(Change::text(login, |s| managed(s, shell))?);
    changes.push(Change::text(config.join("fish/conf.d/hac.fish"), |s| {
        managed(s, fish)
    })?);
    Ok(changes)
}

pub(super) fn directories(home: &Path) -> (PathBuf, PathBuf) {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home.join(".config"));
    let zsh = std::env::var_os("ZDOTDIR")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home.to_path_buf());
    (config, zsh)
}
