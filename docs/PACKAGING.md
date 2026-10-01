# 安装包构建与验收

本文记录本仓库的打包方法。所有命令均在仓库根目录执行。按目标平台分别构建安装包；Universal 表示 macOS 的 Intel 与 Apple Silicon 双架构。

## 1. macOS：Universal 应用与 DMG

### 1.1 适用范围与执行约束

产物包含 `x86_64` 和 `arm64`，输出 `.app` 应用和 `.dmg` 安装镜像。此流程已用于构建 1.0.2，包含紧凑窗口、连续打开 APK 修复及安装成功后的 3 秒关闭倒计时；这些功能的实机验收仍待完成。

遵守 [AGENTS.md](../AGENTS.md) 的编译限制：普通代码修改涉及 5 个及以下代码文件时，不以验证为由运行构建；用户明确要求打包时，按其请求执行本流程。仅编辑本打包文档不需要重新构建。不要为了打包查询提交历史。

### 1.2 构建环境与资源

- 在 macOS 上准备 Rust、Node.js、pnpm 和可用的 Xcode Command Line Tools。
- 安装前端依赖和两个 Rust 编译目标；环境已就绪时不必重复安装。

```bash
pnpm install --frozen-lockfile
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

这些资源由 `src-tauri/tauri.macos.conf.json` 打进 macOS 应用，Windows 安装包不会包含这份 `adb`。确认以下文件存在，且内置 `adb` 有执行权限：

```text
src-tauri/resources/platform-tools/adb
src-tauri/resources/platform-tools/NOTICE.txt
src-tauri/resources/platform-tools/source.properties
src-tauri/icons/icon.icns
src-tauri/Info.plist
```

```bash
test -x src-tauri/resources/platform-tools/adb
lipo -archs src-tauri/resources/platform-tools/adb
```

`lipo` 输出应同时包含 `x86_64`、`arm64`，顺序不限。应用始终调用打包资源中的 `adb`，不得回退到 `PATH` 或系统 Android SDK。更新内置 adb 时，必须从同一份 platform-tools 同步更新 `adb`、`NOTICE.txt` 和 `source.properties`，并保留执行权限。

### 1.3 同步版本号

仅在用户要求升级版本时更新版本号。按已确认的版本号同步以下位置，不要修改依赖包的版本：

| 文件 | 对应字段 |
| --- | --- |
| `package.json` | 顶层 `version` |
| `src-tauri/tauri.conf.json` | 顶层 `version` |
| `src-tauri/Cargo.toml` | `[package]` 下的 `version` |
| `src-tauri/Cargo.lock` | `name = "autoapkinstaller"` 对应条目的 `version` |

例如本次补丁升级为 `1.0.1 → 1.0.2`。`pnpm-lock.yaml` 中出现的相同版本字符串属于依赖时，不随应用版本修改。

### 1.4 构建命令

本机测试用临时签名打包：

```bash
APPLE_SIGNING_IDENTITY=- pnpm tauri build --target universal-apple-darwin --bundles app,dmg --ci
```

- `universal-apple-darwin`：分别编译两个架构并合并为 Universal 应用。
- `--bundles app,dmg`：明确生成两种产物；当前配置的默认目标只有 `app`。
- `APPLE_SIGNING_IDENTITY=-`：使用临时签名，不等于 Apple 开发者签名或公证。
- Tauri 会先运行配置中的 `pnpm build`，无需另行重复前端构建。

等待整个命令成功退出，并确认最后输出两份 bundle。仅看到某个架构编译完成或 `.app` 生成，不代表 DMG 已打包成功。正式分发的开发者签名与公证流程尚未在本仓库配置，应在交付说明中注明当前签名状态。

### 1.5 产物位置与校验

```text
src-tauri/target/universal-apple-darwin/release/bundle/
  macos/AutoApkInstaller.app
  dmg/AutoApkInstaller_<版本号>_universal.dmg
```

以下命令从 `package.json` 读取本次版本，避免误检同目录保留的旧 DMG：

```bash
apk_installer_version=$(node -p 'JSON.parse(require("fs").readFileSync("package.json", "utf8")).version')
apk_installer_app="src-tauri/target/universal-apple-darwin/release/bundle/macos/AutoApkInstaller.app"
apk_installer_dmg="src-tauri/target/universal-apple-darwin/release/bundle/dmg/AutoApkInstaller_${apk_installer_version}_universal.dmg"

lipo -archs "$apk_installer_app/Contents/MacOS/autoapkinstaller"
lipo -archs "$apk_installer_app/Contents/Resources/platform-tools/adb"
/usr/libexec/PlistBuddy -c 'Print CFBundleShortVersionString' "$apk_installer_app/Contents/Info.plist"
hdiutil verify "$apk_installer_dmg"
```

验收校验结果：

- 应用主程序和打包后的 adb 均含 `x86_64`、`arm64`。
- `CFBundleShortVersionString` 与本次目标版本一致。
- `hdiutil verify` 成功退出，并报告镜像校验有效。

交付时提供本次 DMG 的可点击路径，说明版本、双架构校验和签名状态；需要时也提供 `.app` 路径。产物校验不替代下面的实机功能验收。

### 1.6 打包应用的实机验收

将 DMG 内的 `.app` 安装到应用目录，退出旧版本后运行新版本。在 Finder 的 APK“显示简介 → 打开方式”中选择本应用，必要时点击“全部更改”。对新打包的 `.app` 检查：

1. 冷启动双击 APK，以及应用运行中再次打开 APK，均能显示正确文件。
2. 零台设备时提示连接或授权；一台可用设备时自动安装；多台设备时要求选择。未授权、离线设备不能成为安装目标。
3. 安装 A 后打开 B，界面切换到 B，且实际安装 B；安装过程中收到新文件时暂存，不覆盖当前任务。
4. APK 路径含空格、中文及较长文件名时仍能正确安装。
5. 安装失败时保留错误结果；限定重试参数仍可使用。
6. 最后一个 APK 成功安装后显示 3、2、1 秒倒计时，再关闭窗口；新任务、选择文件、调整参数或“保持窗口”能取消关闭。有等待任务或安装失败时不自动关闭。
7. 自动关闭后再次双击 APK 能重新打开应用。
8. 默认 480×360 和最小 400×300 尺寸下，路径、状态、设备信息、倒计时及滚动操作可用。

没有完成的项目明确记为“待实机验收”，不要根据构建成功宣称功能已验证。

## 2. Windows：x64 安装包

### 2.1 适用范围与执行约束

产物是 64 位 Windows 的 NSIS `setup.exe`。代码已按平台选择 `adb.exe`，冷启动时读取 APK 启动参数，应用已在运行时由单实例插件把新的 APK 交给当前窗口。本章的安装包构建，以及安装后在资源管理器中双击打开，都还没有验收。

遵守 [AGENTS.md](../AGENTS.md) 的编译限制：普通代码修改涉及 5 个及以下代码文件时，不以验证为由运行构建；用户明确要求打包时，按其请求执行本流程。仅编辑本打包文档不需要重新构建。不要为了打包查询提交历史。

在 Windows 本机或 Windows CI 上构建。不要在 macOS 上交叉编译这个安装包。

### 2.2 构建环境与资源

- Windows x64。
- Rust stable，默认主机为 `x86_64-pc-windows-msvc`。
- Visual Studio 2022 Build Tools，包含“使用 C++ 的桌面开发”以及 Windows SDK。
- WebView2 Runtime。Windows 11 通常已经带有。
- Node.js 与 pnpm。

环境已就绪时不必重复安装。前端依赖：

```bash
pnpm install --frozen-lockfile
```

确认 Windows 资源和图标存在：

```text
src-tauri/resources/platform-tools/windows/adb.exe
src-tauri/resources/platform-tools/windows/AdbWinApi.dll
src-tauri/resources/platform-tools/windows/AdbWinUsbApi.dll
src-tauri/resources/platform-tools/windows/NOTICE.txt
src-tauri/resources/platform-tools/windows/source.properties
src-tauri/icons/icon.ico
src-tauri/tauri.windows.conf.json
```

`src-tauri/tauri.windows.conf.json` 把上述 `adb.exe`、两份 DLL、`NOTICE.txt` 和 `source.properties` 映射到安装包内的同一 `platform-tools` 目录，并把打包目标设为 NSIS。应用只运行这份 `adb.exe`。Windows 加载 `adb.exe` 时会从它所在目录查找 `AdbWinApi.dll` 和 `AdbWinUsbApi.dll`，不能只复制可执行文件。不得回退到 `PATH` 或本机 Android SDK。

当前副本是 Android SDK Platform-Tools `37.0.1`。更新时从同一份 `platform-tools` 覆盖 `windows/` 中的这五个文件，不要带入 `fastboot.exe` 或其他命令，然后重新打包。

### 2.3 同步版本号

与 [1.3 同步版本号](#13-同步版本号) 使用同一组文件。仅在用户要求升级版本时修改，不要修改依赖包的版本。

### 2.4 构建命令

在仓库根目录执行：

```bash
pnpm tauri build --bundles nsis
```

Tauri 会先运行配置中的 `pnpm build`，无需另行重复前端构建。首次打包时 Tauri CLI 会下载 NSIS。等待命令成功退出，并确认输出了 `setup.exe`。只完成 Rust 编译不代表安装包已经生成。

### 2.5 产物位置与校验

```text
src-tauri/target/release/bundle/nsis/AutoApkInstaller_<版本号>_x64-setup.exe
```

用 `package.json` 的版本核对文件名，避免把同目录里的旧安装包当成本次产物。安装这份 `setup.exe` 后，确认安装目录的 `platform-tools` 中同时有 `adb.exe`、`AdbWinApi.dll` 和 `AdbWinUsbApi.dll`。产物存在不代表下面的实机功能已经验收。

### 2.6 打包应用的实机验收

安装 `setup.exe` 并退出正在运行的旧进程。对新安装的应用检查：

1. 冷启动双击 APK，以及应用运行中再次双击 APK，都只保留一个窗口，并显示正确文件。运行中再次打开不得再弹出第二个窗口。
2. 零台设备时提示连接或授权；一台可用设备时自动安装；多台设备时要求选择。未授权、离线设备不能成为安装目标。
3. 安装 A 后打开 B，界面切换到 B，且实际安装 B；安装过程中收到新文件时暂存，不覆盖当前任务。
4. APK 路径含空格、中文及较长文件名时仍能正确安装。
5. 安装失败时保留错误结果；限定重试参数仍可使用。
6. 最后一个 APK 成功安装后显示 3、2、1 秒倒计时，再关闭窗口；新任务、选择文件、调整参数或“保持窗口”能取消关闭。有等待任务或安装失败时不自动关闭。
7. 自动关闭后再次双击 APK 能重新打开应用。
8. 默认 480×360 和最小 400×300 尺寸下，路径、状态、设备信息、倒计时及滚动操作可用。

`pnpm tauri dev` 不会向系统注册 `.apk` 关联，不能代替以上安装后的验收。没有完成的项目记为“待实机验收”。
