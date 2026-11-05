use serde::Serialize;
use std::{
    collections::HashSet,
    path::PathBuf,
    time::{Duration, Instant},
};
use sysinfo::{
    Pid, Process, ProcessRefreshKind, ProcessStatus, ProcessesToUpdate, System, Uid, UpdateKind,
};

#[derive(Serialize)]
pub struct StopResult {
    pub stopped: usize,
    pub failed: usize,
}

#[derive(Clone)]
struct Candidate {
    pid: Pid,
    parent: Option<Pid>,
    start: u64,
    exe: PathBuf,
    same_user: bool,
    root: Option<PathBuf>,
}

fn desktop_root(os: &str, exe: &str, resources: bool) -> Option<String> {
    let path = exe.replace('\\', "/");
    let (parent, name) = path.rsplit_once('/')?;
    match os {
        "macos" => {
            if !path.contains("/Contents/MacOS/") {
                return None;
            }
            let marker = if path.contains("/Codex.app/") {
                "/Codex.app/"
            } else if path.contains("/ChatGPT.app/") {
                "/ChatGPT.app/"
            } else {
                return None;
            };
            let prefix = path.split_once(marker)?.0;
            Some(format!("{prefix}{}", marker.trim_end_matches('/')))
        }
        "windows" => {
            let name = name.to_ascii_lowercase();
            let app = match name.as_str() {
                "chatgpt.exe" => "chatgpt",
                "codex.exe" => "codex",
                _ => return None,
            };
            let lower = parent.to_ascii_lowercase();
            let package = lower.split('/').any(|part| {
                part.starts_with(&format!("openai.{app}_"))
                    || part.starts_with(&format!("openai.{app}-desktop_"))
            });
            // Bundled CLI binaries live below resources; only the desktop entry executable qualifies.
            let leaf = lower.rsplit('/').next()?;
            ((package && leaf == "app") || (resources && leaf == app)).then(|| parent.to_string())
        }
        "linux" => (resources && matches!(name, "codex" | "Codex" | "chatgpt" | "ChatGPT"))
            .then(|| parent.to_string()),
        _ => None,
    }
}

fn refresh_kind() -> ProcessRefreshKind {
    ProcessRefreshKind::nothing()
        .with_exe(UpdateKind::Always)
        .with_user(UpdateKind::Always)
}

fn inventory() -> Result<(System, Uid), String> {
    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, refresh_kind());
    let owner = system
        .process(Pid::from_u32(std::process::id()))
        .and_then(Process::user_id)
        .cloned()
        .ok_or("Không xác định được tài khoản của tiến trình HAC; chưa dừng ứng dụng")?;
    Ok((system, owner))
}

fn snapshot(process: &Process, owner: &Uid) -> Candidate {
    let exe = process
        .exe()
        .unwrap_or_else(|| std::path::Path::new(""))
        .to_path_buf();
    let resources = exe
        .parent()
        .is_some_and(|p| p.join("resources/app.asar").is_file());
    let root = exe
        .to_str()
        .and_then(|path| desktop_root(std::env::consts::OS, path, resources))
        .map(PathBuf::from);
    Candidate {
        pid: process.pid(),
        parent: process.parent(),
        start: process.start_time(),
        exe,
        same_user: process.user_id() == Some(owner),
        root,
    }
}

fn select(rows: &[Candidate], current: Pid) -> Vec<Candidate> {
    let mut protected = HashSet::new();
    let mut ancestor = Some(current);
    while let Some(pid) = ancestor {
        if !protected.insert(pid) {
            break;
        }
        ancestor = rows.iter().find(|r| r.pid == pid).and_then(|r| r.parent);
    }
    let mut chosen: Vec<(Candidate, PathBuf)> = rows
        .iter()
        .filter(|r| r.same_user && !protected.contains(&r.pid))
        .filter_map(|r| r.root.clone().map(|root| (r.clone(), root)))
        .collect();
    loop {
        let children: Vec<_> = rows
            .iter()
            .filter(|r| {
                r.same_user
                    && !protected.contains(&r.pid)
                    && !chosen.iter().any(|(c, _)| c.pid == r.pid)
            })
            .filter_map(|r| {
                chosen
                    .iter()
                    .find(|(parent, root)| Some(parent.pid) == r.parent && r.exe.starts_with(root))
                    .map(|(_, root)| (r.clone(), root.clone()))
            })
            .collect();
        if children.is_empty() {
            break;
        }
        chosen.extend(children);
    }
    chosen.into_iter().map(|(r, _)| r).collect()
}

fn same_process(process: &Process, expected: &Candidate, owner: &Uid) -> bool {
    process.start_time() == expected.start
        && process.exe() == Some(expected.exe.as_path())
        && process.user_id() == Some(owner)
}

fn exited(system: &System, expected: &Candidate) -> bool {
    system.process(expected.pid).is_none_or(|p| {
        p.start_time() != expected.start
            || matches!(p.status(), ProcessStatus::Zombie | ProcessStatus::Dead)
    })
}

fn stop_candidates(system: &mut System, owner: &Uid, targets: &[Candidate]) -> StopResult {
    let mut pending = Vec::new();
    let mut result = StopResult {
        stopped: 0,
        failed: 0,
    };
    for target in targets {
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[target.pid]),
            true,
            refresh_kind(),
        );
        let Some(process) = system.process(target.pid) else {
            continue;
        };
        if !same_process(process, target, owner) {
            result.failed += 1;
            continue;
        }
        if exited(system, target) {
            continue;
        }
        if process.kill() {
            pending.push(target.clone());
        } else {
            result.failed += 1;
        }
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    while !pending.is_empty() {
        let pids: Vec<_> = pending.iter().map(|p| p.pid).collect();
        system.refresh_processes_specifics(ProcessesToUpdate::Some(&pids), true, refresh_kind());
        pending.retain(|target| {
            if exited(system, target) {
                result.stopped += 1;
                false
            } else {
                true
            }
        });
        if Instant::now() >= deadline {
            break;
        }
        if !pending.is_empty() {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    result.failed += pending.len();
    result
}

pub fn stop_desktop_apps() -> Result<StopResult, String> {
    let (mut system, owner) = inventory()?;
    let rows: Vec<_> = system
        .processes()
        .values()
        .map(|p| snapshot(p, &owner))
        .collect();
    let targets = select(&rows, Pid::from_u32(std::process::id()));
    Ok(stop_candidates(&mut system, &owner, &targets))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_desktop_installations_without_matching_cli_or_lookalikes() {
        let cases = [
            ("macos", "/Applications/Codex.app/Contents/Frameworks/Codex Helper (Renderer).app/Contents/MacOS/Codex Helper (Renderer)", false, true),
            ("macos", "/Applications/Codex.app/Contents/Resources/codex", false, false),
            ("windows", r"C:\Program Files\WindowsApps\OpenAI.ChatGPT-Desktop_1.0_x64__abc\app\ChatGPT.exe", false, true),

            (
                "macos",
                "/Applications/Codex.app/Contents/MacOS/Codex",
                false,
                true,
            ),
            (
                "macos",
                "/Users/test/Apps/ChatGPT.app/Contents/MacOS/ChatGPT",
                false,
                true,
            ),
            ("macos", "/usr/local/bin/codex", false, false),
            (
                "macos",
                "/Applications/FakeCodex.app/Contents/MacOS/Codex",
                false,
                false,
            ),
            (
                "windows",
                r"C:\Program Files\WindowsApps\OpenAI.Codex_1.0_x64__abc\app\Codex.exe",
                false,
                true,
            ),
            (
                "windows",
                r"C:\Users\test\AppData\Local\Programs\ChatGPT\ChatGPT.exe",
                true,
                true,
            ),
            (
                "windows",
                r"C:\Users\test\node_modules\@openai\codex\vendor\codex.exe",
                false,
                false,
            ),
            ("windows", r"C:\other\Codex.exe", false, false),
            (
                "windows",
                r"C:\Programs\Codex\resources\codex.exe",
                false,
                false,
            ),
            ("linux", "/opt/codex/codex", true, true),
            ("linux", "/opt/chatgpt/chatgpt", true, true),
            ("linux", "/usr/local/bin/codex", false, false),
            ("linux", "/opt/other/not-codex", true, false),
        ];
        for (os, path, resources, wanted) in cases {
            assert_eq!(
                desktop_root(os, path, resources).is_some(),
                wanted,
                "{os}: {path}"
            );
        }
    }
    #[test]
    fn includes_only_same_user_bundled_descendants_and_protects_hac_ancestors() {
        fn row(
            pid: u32,
            parent: Option<u32>,
            exe: &str,
            same_user: bool,
            root: Option<&str>,
        ) -> Candidate {
            Candidate {
                pid: Pid::from_u32(pid),
                parent: parent.map(Pid::from_u32),
                exe: exe.into(),
                start: 1,
                same_user,
                root: root.map(Into::into),
            }
        }
        let rows = vec![
            row(1, None, "/opt/codex/codex", true, Some("/opt/codex")),
            row(2, Some(1), "/opt/codex/resources/codex", true, None),
            row(3, Some(1), "/usr/bin/bash", true, None),
            row(4, None, "/usr/bin/codex", true, None),
            row(5, None, "/opt/chatgpt/chatgpt", false, Some("/opt/chatgpt")),
            row(6, Some(1), "/opt/codex/helper", false, None),
            row(7, Some(2), "/opt/codex/helpers/renderer", true, None),
        ];
        let mut ids: Vec<_> = select(&rows, Pid::from_u32(99))
            .iter()
            .map(|p| p.pid.as_u32())
            .collect();
        ids.sort();
        assert_eq!(ids, vec![1, 2, 7]);
        assert!(select(&rows, Pid::from_u32(2))
            .iter()
            .all(|p| p.pid.as_u32() != 1 && p.pid.as_u32() != 2));
    }

    #[cfg(unix)]
    #[test]
    fn stops_only_its_disposable_test_child_and_rejects_changed_identity() {
        struct Child(std::process::Child);
        impl Drop for Child {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let mut child = Child(
            std::process::Command::new("/bin/sleep")
                .arg("30")
                .spawn()
                .unwrap(),
        );
        let pid = Pid::from_u32(child.0.id());
        let (mut system, owner) = inventory().unwrap();
        let selected = snapshot(system.process(pid).unwrap(), &owner);
        let mut wrong = selected.clone();
        wrong.start += 1;
        assert_eq!(stop_candidates(&mut system, &owner, &[wrong]).failed, 1);
        assert!(child.0.try_wait().unwrap().is_none());
        let result = stop_candidates(&mut system, &owner, &[selected]);
        assert_eq!(result.stopped, 1);
        assert_eq!(result.failed, 0);
        assert!(!child.0.wait().unwrap().success());
    }
}
