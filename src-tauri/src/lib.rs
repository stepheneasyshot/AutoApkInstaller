mod adb;

use std::{collections::VecDeque, sync::Mutex};
use tauri::{Emitter, Manager};

struct PendingApks(Mutex<VecDeque<String>>);

#[tauri::command]
fn take_pending_apks(state: tauri::State<'_, PendingApks>) -> Vec<String> {
    state.0.lock().unwrap().drain(..).collect()
}

#[tauri::command]
async fn list_devices(app: tauri::AppHandle) -> Result<Vec<adb::Device>, adb::AppError> {
    tauri::async_runtime::spawn_blocking(move || adb::list_devices(&app))
        .await
        .map_err(|error| adb::AppError::task_failed(error.to_string()))?
}

#[tauri::command]
async fn install_apk(
    app: tauri::AppHandle,
    path: String,
    serial: String,
    options: adb::InstallOptions,
) -> Result<adb::InstallReport, adb::AppError> {
    tauri::async_runtime::spawn_blocking(move || adb::install_apk(&app, &path, &serial, options))
        .await
        .map_err(|error| adb::AppError::task_failed(error.to_string()))?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(PendingApks(Mutex::new(VecDeque::new())))
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            take_pending_apks,
            list_devices,
            install_apk
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Opened { urls } = event {
                let paths: Vec<String> = urls
                    .into_iter()
                    .filter_map(|url| url.to_file_path().ok())
                    .filter(|path| {
                        path.extension()
                            .and_then(|ext| ext.to_str())
                            .is_some_and(|ext| ext.eq_ignore_ascii_case("apk"))
                    })
                    .map(|path| path.to_string_lossy().into_owned())
                    .collect();
                if !paths.is_empty() {
                    app.state::<PendingApks>().0.lock().unwrap().extend(paths);
                    let _ = app.emit("apk-opened", ());
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        });
}
