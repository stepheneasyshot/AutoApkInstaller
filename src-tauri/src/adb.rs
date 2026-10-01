use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use tauri::{path::BaseDirectory, Manager};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub serial: String,
    pub state: String,
    pub description: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: &'static str,
    pub message: String,
}

impl AppError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn task_failed(message: String) -> Self {
        Self::new("task_failed", format!("后台任务未完成：{message}"))
    }
}

#[derive(Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallOptions {
    pub allow_downgrade: bool,
    pub grant_permissions: bool,
    pub allow_test_apk: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallReport {
    pub success: bool,
    pub detail: String,
}

#[cfg(windows)]
const ADB_RESOURCE: &str = "platform-tools/adb.exe";
#[cfg(not(windows))]
const ADB_RESOURCE: &str = "platform-tools/adb";

fn without_verbatim_prefix(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path
    }
}

fn adb_path(app: &tauri::AppHandle) -> Result<PathBuf, AppError> {
    let path = app
        .path()
        .resolve(ADB_RESOURCE, BaseDirectory::Resource)
        .map_err(|error| AppError::new("adb_missing", format!("无法定位应用内置 adb：{error}")))?;
    if !path.is_file() {
        return Err(AppError::new(
            "adb_missing",
            "应用内置 adb 缺失，请重新安装完整的应用。",
        ));
    }
    Ok(path)
}

fn adb_command(app: &tauri::AppHandle) -> Result<Command, AppError> {
    let mut command = Command::new(adb_path(app)?);
    // adb.exe 是控制台程序。从窗口应用启动时要隐藏控制台，状态只显示在主窗口。
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    Ok(command)
}

fn output_text(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    format!("{}\n{}", stdout.trim(), stderr.trim())
        .trim()
        .to_string()
}

fn parse_devices(stdout: &str) -> Vec<Device> {
    stdout
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty()
                || line.starts_with("List of devices attached")
                || line.starts_with('*')
            {
                return None;
            }
            let mut fields = line.split_whitespace();
            let serial = fields.next()?;
            let state = fields.next()?;
            let description = fields.collect::<Vec<_>>().join(" ");
            Some(Device {
                serial: serial.to_string(),
                state: state.to_string(),
                description,
            })
        })
        .collect()
}

pub fn list_devices(app: &tauri::AppHandle) -> Result<Vec<Device>, AppError> {
    let output = adb_command(app)?
        .args(["devices", "-l"])
        .output()
        .map_err(|error| AppError::new("adb_failed", format!("无法运行 adb：{error}")))?;
    if !output.status.success() {
        return Err(AppError::new(
            "adb_failed",
            format!("读取设备列表失败：{}", output_text(&output)),
        ));
    }

    Ok(parse_devices(&String::from_utf8_lossy(&output.stdout)))
}

fn checked_apk(path: &str) -> Result<PathBuf, AppError> {
    let requested = Path::new(path);
    let is_apk = requested
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("apk"));
    if !is_apk {
        return Err(AppError::new("invalid_apk", "请选择单个 .apk 文件。"));
    }
    let canonical = fs::canonicalize(requested)
        .map_err(|_| AppError::new("invalid_apk", "APK 文件不存在或无法读取。"))?;
    if !canonical.is_file() {
        return Err(AppError::new("invalid_apk", "所选路径不是常规 APK 文件。"));
    }
    Ok(without_verbatim_prefix(canonical))
}

pub fn install_apk(
    app: &tauri::AppHandle,
    path: &str,
    serial: &str,
    options: InstallOptions,
) -> Result<InstallReport, AppError> {
    let apk = checked_apk(path)?;
    let devices = list_devices(app)?;
    if !devices
        .iter()
        .any(|device| device.serial == serial && device.state == "device")
    {
        return Err(AppError::new(
            "device_unavailable",
            "所选设备已断开连接或未授权，请刷新设备列表。",
        ));
    }

    let mut command = adb_command(app)?;
    command.arg("-s").arg(serial).arg("install").arg("-r");
    if options.allow_downgrade {
        command.arg("-d");
    }
    if options.grant_permissions {
        command.arg("-g");
    }
    if options.allow_test_apk {
        command.arg("-t");
    }
    let output = command
        .arg(&apk)
        .output()
        .map_err(|error| AppError::new("adb_failed", format!("无法运行安装命令：{error}")))?;
    let raw = output_text(&output);
    let detail = raw
        .replace(&apk.to_string_lossy().to_string(), "APK")
        .replace(path, "APK");
    let success = output.status.success() && detail.lines().any(|line| line.trim() == "Success");
    Ok(InstallReport {
        success,
        detail: if detail.is_empty() {
            if success {
                "安装成功。".to_string()
            } else {
                "adb 未返回安装结果。".to_string()
            }
        } else {
            detail.chars().take(1200).collect()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_online_and_unavailable_devices() {
        let devices = parse_devices("List of devices attached\nemulator-5554 device product:sdk model:Pixel_8\nabc unauthorized\n");
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].serial, "emulator-5554");
        assert_eq!(devices[0].state, "device");
        assert_eq!(devices[0].description, "product:sdk model:Pixel_8");
        assert_eq!(devices[1].state, "unauthorized");
    }

    #[test]
    fn strips_windows_verbatim_prefix() {
        assert_eq!(
            without_verbatim_prefix(PathBuf::from(r"\\?\C:\apps\demo.apk")),
            PathBuf::from(r"C:\apps\demo.apk")
        );
        assert_eq!(
            without_verbatim_prefix(PathBuf::from(r"\\?\UNC\server\share\demo.apk")),
            PathBuf::from(r"\\server\share\demo.apk")
        );
    }
}
