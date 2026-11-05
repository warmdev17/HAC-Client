#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

#[tauri::command]
async fn get_status() -> Result<hac_core::SetupStatus, String> {
    hac_core::get_status().await
}

#[tauri::command]
async fn setup(request: hac_core::SetupRequest) -> Result<hac_core::SetupResult, String> {
    hac_core::setup(request).await
}

#[tauri::command]
async fn uninstall() -> Result<String, String> {
    hac_core::uninstall().await
}

#[tauri::command]
async fn reset_configs() -> Result<hac_core::SetupResult, String> {
    hac_core::reset_configs().await
}

#[tauri::command]
async fn stop_desktop_apps() -> Result<hac_core::processes::StopResult, String> {
    tauri::async_runtime::spawn_blocking(hac_core::processes::stop_desktop_apps)
        .await
        .map_err(|_| "Dừng ứng dụng bị gián đoạn; kiểm tra các cửa sổ đang mở")?
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_status,
            setup,
            uninstall,
            reset_configs,
            stop_desktop_apps
        ])
        .run(tauri::generate_context!())
        .expect("Không khởi động được HAC");
}
