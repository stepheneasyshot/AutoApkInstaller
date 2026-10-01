mod adb;

use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::Mutex,
};
use tauri::{Emitter, Manager};

struct PendingApks(Mutex<VecDeque<String>>);

fn path_from_open_arg(arg: &str, cwd: Option<&Path>) -> Option<PathBuf> {
    let arg = arg.trim().trim_matches('"');
    if arg.is_empty() || arg.starts_with('-') {
        return None;
    }
    let path = if arg.starts_with("file:") {
        tauri::Url::parse(arg)
            .ok()
            .and_then(|url| url.to_file_path().ok())?
    } else {
        PathBuf::from(arg)
    };
    let path = if path.is_absolute() {
        path
    } else {
        cwd?.join(path)
    };
    let is_apk = path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("apk"));
    is_apk.then_some(path)
}

fn apk_paths_from_args(args: &[String], cwd: Option<&Path>) -> Vec<String> {
    args.iter()
        .skip(1)
        .filter_map(|arg| path_from_open_arg(arg, cwd))
        .map(|path| path.to_string_lossy().into_owned())
        .collect()
}

fn deliver_opened_apks(app: &tauri::AppHandle, paths: &[String]) {
    if paths.is_empty() {
        return;
    }
    app.state::<PendingApks>()
        .0
        .lock()
        .unwrap()
        .extend(paths.iter().cloned());
    let _ = app.emit("apk-opened", ());
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

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
    let mut builder = tauri::Builder::default();

    // 必须最先注册。Windows 再次打开 APK 会启动第二个进程，插件把参数交给当前实例后退出。
    #[cfg(windows)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            let cwd = PathBuf::from(cwd);
            deliver_opened_apks(app, &apk_paths_from_args(&args, Some(&cwd)));
        }));
    }

    builder
        .manage(PendingApks(Mutex::new(VecDeque::new())))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            #[cfg(windows)]
            {
                let args: Vec<String> = std::env::args().collect();
                let cwd = std::env::current_dir().ok();
                deliver_opened_apks(app.handle(), &apk_paths_from_args(&args, cwd.as_deref()));
            }
            #[cfg(not(windows))]
            let _ = app;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            take_pending_apks,
            list_devices,
            install_apk
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(handle_run_event);
}

fn handle_run_event(app: &tauri::AppHandle, event: tauri::RunEvent) {
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
        deliver_opened_apks(app, &paths);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (app, event);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_flags_and_non_apk_arguments() {
        let args = vec![
            "app".to_string(),
            "--help".to_string(),
            "readme.txt".to_string(),
        ];
        assert!(apk_paths_from_args(&args, Some(Path::new("."))).is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn reads_windows_apk_arguments() {
        let args = vec![
            "AutoApkInstaller.exe".to_string(),
            r"D:\apps\demo.apk".to_string(),
            r"file:///C:/tmp/a.APK".to_string(),
            r"queued\beta.apk".to_string(),
        ];
        let paths = apk_paths_from_args(&args, Some(Path::new(r"D:\downloads")));
        assert_eq!(
            paths,
            vec![
                r"D:\apps\demo.apk".to_string(),
                r"C:\tmp\a.APK".to_string(),
                r"D:\downloads\queued\beta.apk".to_string(),
            ]
        );
    }
}
