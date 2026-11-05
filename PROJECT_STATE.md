# HAC project state

Updated: 2026-11-05 (Asia/Ho_Chi_Minh). Branch: main; base commit: 0fd1e68 (v0.1.1).

## Objective / approved decisions
- Builder owns implementation: add confirmed shutdown of ChatGPT and Codex Desktop, excluding standalone CLI.
- Add reset of config.toml and hac.config.toml, backing up first; preserve auth, conversations, credentials and environment.
- Set default reasoning to high, matching HAC profile. Official docs: since Codex 0.134.0, default profile selector is unsupported; use base config directly.

## Completed (release v0.1.2 pending commit)
- Backend reset + Tauri command and UI confirmation; raw-byte private backups, ownership journal with nullable after-state, rollback without overwriting concurrent edits. Reset backups are not removed by uninstall.
- Desktop shutdown + UI confirmation: sysinfo inventory, current-user/executable/start-time checks, protect HAC ancestors/independent CLI, bounded exit verification and partial-failure counts. OS layouts covered by fixtures; a disposable sleep child exercised real termination.
- Base/profile high defaults, legacy medium uninstall compatibility. README explains profile behavior and reset/stop limitations.
- Plan: [maintenance actions](docs/superpowers/plans/2026-11-05-maintenance-actions.md).

## Verification at 0fd1e68 + release v0.1.2 changes
- `npm test`: 12 passed. `npm run build`: passed.
- `HAC_BASE_URL=https://provider.example/v1 cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --locked`: 33 passed, 2 optional OS integration tests ignored.
- `HAC_BASE_URL=https://provider.example/v1 cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings`: passed.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` and `git diff --check`: passed.
- `HAC_BASE_URL=https://provider.example/v1 cargo build --manifest-path src-tauri/Cargo.toml --locked`: native Linux debug build passed; placeholder endpoint, not for distribution.
- Reviewer agent disconnected; builder self-reviewed, adding regression coverage for missing configs with a stale journal, old medium configs, macOS helpers, Windows ChatGPT package naming.
- Browser visual check blocked: IAB unavailable, Brave tab creation tool failed. Windows/macOS live termination and desktop layout not tested; no real user app/config/keyring mutated.

## Compatibility and next steps
- User authorized release on 2026-11-05. Version bumped to 0.1.2; commit/push/tag and four-platform CI release pending.
- Existing separate Python setup script remains unchanged (medium; cannot handle reset journal after=null). Use new app after reset; old app may reject reset journals. Normal old journals remain readable by new app.
- GitHub verified v0.1.1 published successfully; remote main matches 0fd1e68 before release.
- Preserve unrelated existing README standalone-script text, scripts/setup-linux.py, scripts/test_setup_linux.py, .ua/.
- Test app installation layouts and UI on Windows/macOS before release; nonstandard installations may require manual closing.
