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

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_status, setup])
        .run(tauri::generate_context!())
        .expect("Không khởi động được HAC");
}
