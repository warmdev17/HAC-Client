# Maintenance actions implementation plan

Goal: reset only two config files with backups, stop ChatGPT/Codex desktop apps on explicit confirmation, and use high reasoning by default.
Approved scope: user conversation 2026-11-05. Builder executes inline (ponytail/TDD); PROJECT_STATE.md is the sole progress tracker.

1. Backend reset (uninstall.rs/files.rs/lib.rs): snapshot config.toml, hac.config.toml and existing journal; back up all before deleting either config. Update only their journal entries, keep other entries and credentials. Roll back on failure without overwriting concurrent edits. Return backup paths via SetupResult. Test arbitrary/invalid TOML, missing files, symlinks, setup-reset-setup-remove, failed transaction.
2. Process shutdown (processes.rs/main.rs): use cross-platform process handles, current-user filter and desktop executable paths; exclude CLI, HAC, browser and other users. Revalidate process identity before kill; wait bounded time and report partial failures. No PID/path supplied by webview. Tests use own disposable child only; matching fixtures include CLI and desktop helpers.
3. Defaults and UI: render/matches use high at root and profile. Keep legacy medium uninstall compatibility. Add reset and stop confirmation dialogs; disable during any operation and browser preview; report results/errors and backups. Test cancel, duplicate clicks, recovery from errors, secret clearing and unsupported states.
4. Verify: npm test/build, cargo test --no-default-features --locked, cargo fmt --check, clippy --all-targets --locked. Review diff and update README/state. No live user process termination or real config mutation in tests; Windows/macOS runtime remains a manual check.
