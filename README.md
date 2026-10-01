<p align="center">
  <img src="icons/app_icon.png" alt="AutoApkInstaller 图标" width="128" height="128">
</p>

<h1 align="center">AutoApkInstaller</h1>

一个面向 macOS 和 Windows 的轻量 APK 安装工具。目标是将应用设为 `.apk` 文件的打开方式，双击 APK 后调用 `adb` 安装到 Android 设备，并用简洁界面处理未连接设备、多设备选择、安装结果和重试参数。

> 当前状态：macOS 核心流程已实现，文件关联仍需以打包后的 `.app` 在 Finder 中设置默认打开方式并实机验收。Windows 已适配内置 `adb.exe` 及 DLL、冷启动参数、运行中再次打开时的单实例转发，以及 NSIS 安装包配置；Windows 安装包构建和资源管理器双击打开尚未验收。

## 第一期体验

1. 用户在 Finder 或资源管理器中双击 APK，系统将文件交给已打包的 AutoApkInstaller。也可先在应用内选择 APK，方便开发和首次使用。
2. Rust 后端检查文件并运行 `adb devices -l`，仅把状态为 `device` 的设备视为可安装目标。
3. 只有一台可用设备时，立即用默认参数安装；没有可用设备时展示连接与授权提示、刷新按钮；有多台时让用户选择目标设备。
4. 安装完成后展示成功或失败结果。结果页允许调整安装参数并对同一 APK、同一设备重试。
5. 最后一个 APK 安装成功、没有等待任务时，页脚显示 3 秒关闭倒计时，到期关闭窗口。收到新 APK、选择文件、调整重试参数或点击“保持窗口”会取消倒计时；下次安装成功后重新计时。安装失败或报错时保留窗口供排查与重试。

默认安装采用 `adb -s <serial> install -r <apk>`：覆盖已有安装并保留应用数据。第一期只提供单个 `.apk` 安装；可选重试参数限定为 `-d`（允许降级）、`-g`（授予清单权限）和 `-t`（允许测试 APK）。这些选项由界面中的明确开关控制，不提供任意 adb 参数输入。

## 范围

| 第一期包含 | 暂不包含 |
| --- | --- |
| macOS 与 Windows x64 的 `.apk` 打开 | Linux 支持 |
| 单 APK、单目标设备安装 | `.apks`/`.xapk`/拆分 APK 批量安装 |
| 无设备提示、多设备选择、安装结果与重试 | 无线配对、设备管理、安装队列 |
| 随应用打包当前平台的 `adb` | 运行时下载或更新 platform-tools |

第一期不保存设备偏好或安装历史。一次只处理一个安装任务；检查设备、等待设备选择或安装期间打开的 APK 会暂存，当前任务结束后可点击“处理下一个 APK”。已在当前任务或等待列表中的同一 APK 会忽略，不会重复加入。安装成功、失败或设备检查报错后再次打开另一个 APK，会立即切换到等待列表中的下一个文件并重新检查设备；一台可用设备时自动安装。结果页仍可重试当前 APK，新的文件不会覆盖正在安装的任务。

## 技术分工

- **Rust / Tauri**：在 macOS 上接收打开文件事件；在 Windows 上读取启动参数，并用单实例插件把再次打开的 APK 交给已运行窗口。随后校验 APK 路径，定位应用资源中的 `adb`，解析设备状态，执行安装，并向前端返回结构化结果。
- **React / TypeScript**：展示待机、缺少 adb、无设备、多设备、安装中、成功和失败等状态；收集目标设备及重试选项。前端不直接执行 shell 命令。
- **Tauri 桥接**：前端通过命令读取待处理文件、刷新设备并请求安装；运行中再次打开 APK 时通过事件通知前端。文件处理和安装使用同一条后端逻辑。

当前按职责拆成少量文件，没有引入状态管理框架、数据库或通用任务系统：

```text
src/
  App.tsx                 # 页面状态与交互编排
  api.ts                  # Tauri 命令/事件的类型化封装
src-tauri/src/
  lib.rs                  # Tauri 初始化、打开文件事件、命令注册
  adb.rs                  # 内置 adb 定位、设备解析、APK 校验与安装命令
src-tauri/resources/platform-tools/
  adb                     # macOS universal 可执行文件
  NOTICE.txt              # macOS 副本附带的第三方声明
  source.properties       # macOS 副本的平台工具版本
  windows/
    adb.exe               # Windows adb
    AdbWinApi.dll         # adb.exe 需要的 DLL，必须放在同一目录
    AdbWinUsbApi.dll
    NOTICE.txt
    source.properties
src-tauri/tauri.conf.json         # 共用的 .apk 文件关联
src-tauri/tauri.macos.conf.json   # macOS 资源
src-tauri/tauri.windows.conf.json # Windows 资源、图标与 NSIS 安装包
```

当前命令接口为 `list_devices()`、`install_apk({ path, serial, options })`、`take_pending_apks()`。错误以明确的类别和可展示的说明返回，避免让界面解析 adb 的整段输出。adb 命令在后台任务中执行，避免阻塞窗口。

## 已实现与验收路线

1. **后端安装链路**：已实现内置 adb 定位、`devices -l` 解析、APK 路径校验和带 `-s` 的安装；区分内置 adb 缺失、设备未授权或离线、安装失败。
2. **最小界面**：已替换模板欢迎页；接入手动选择 APK、设备刷新/选择、安装状态与结果页；在结果页添加重试选项。
3. **双击打开**：已在 Tauri bundle 中声明 `.apk` 文件关联。macOS 处理冷启动和运行中的打开文件事件；Windows 从启动参数读取冷启动文件，运行中再次打开时由单实例插件转发。两边复用同一安装流程。
4. **打包应用验收**：检查 macOS 文件关联及默认打开方式，分别用零台、一台、多台设备验证，并检查路径含空格、安装失败和再次打开 APK 的情况。

连续打开修复的实机验收待完成：在打包后的 `.app` 中安装 A，保持结果页并双击 B，确认文件路径切换到 B，且单设备时实际安装 B；另需验证失败后打开新文件、安装中打开新文件仍暂存，以及结果页重试。修复已包含在 1.0.2 Universal `.app` 和 DMG 中，构建通过。

自动关闭的实机验收待完成：确认最后一次成功安装后倒计时显示 3、2、1 并关闭窗口；倒计时内双击新 APK 应取消关闭并安装新文件，有等待任务或安装失败时不自动关闭，选择文件和“保持窗口”可保留窗口。关闭后再次双击 APK 需能重新打开应用。改动已包含在 1.0.2 Universal `.app` 和 DMG 中，构建通过。

完成标准：安装好的 `.app` 被设为 `.apk` 默认打开方式后，双击单个 APK 能按设备数量进入正确流程；单设备可直接安装；安装结果和错误可读；参数调整后可重试。

窗口默认尺寸为 480×360（逻辑像素），最小可缩至 400×300。界面优先展示 APK 文件名与路径、安装状态及设备简略信息；长路径悬停可查看全文，重试参数折叠展示，多设备或较长结果通过内容区域滚动查看。紧凑布局尚未在重新打包的 macOS `.app` 中实机验收，需检查默认及最小尺寸下的各状态、长路径和展开重试参数。

## 开发准备

- macOS 或 Windows x64、对应平台的 Rust 工具链、Node.js 与 pnpm。Windows 还需带 C++ 桌面开发组件的 Visual Studio Build Tools，以及系统自带的 WebView2。
- 已开启 USB 调试并授权本机的 Android 设备或模拟器。运行本应用不要求单独安装 adb，也不依赖终端的 `PATH`。

```bash
pnpm install
pnpm tauri dev
```

macOS Universal `.app` 与 DMG 见 [安装包构建与验收：第一章 macOS](docs/PACKAGING.md#1-macosuniversal-应用与-dmg)。当前流程使用临时签名，适合本机测试；正式分发仍需 Apple 开发者签名与公证。Windows x64 的 NSIS 安装包见 [第二章 Windows](docs/PACKAGING.md#2-windowsx64-安装包)。

`pnpm dev` 仅启动 Vite 页面；涉及 Rust 命令与文件打开事件时使用 `pnpm tauri dev`。程序只运行随应用打包的 `adb`，不会从 `PATH` 或本机 Android SDK 查找可执行文件。文件关联需以安装后的应用验收：macOS 在 Finder 的“显示简介 → 打开方式”中选择本应用并点击“全部更改”；Windows 由 NSIS 安装包注册，安装后可在资源管理器的“打开方式”中选择本应用。`pnpm tauri dev` 不会注册系统文件关联。

### 更新内置 adb

当前 macOS 与 Windows 副本都来自 Android SDK Platform-Tools `37.0.1`。macOS 文件是同时支持 Intel 和 Apple Silicon 的可执行文件，放在 `src-tauri/resources/platform-tools/`。Windows 文件放在该目录下的 `windows/`：`adb.exe`、`AdbWinApi.dll`、`AdbWinUsbApi.dll`、`NOTICE.txt` 和 `source.properties`。两份 DLL 由 `adb.exe` 加载，必须和它一起打包到同一目录。更新某一平台时，只替换该平台的这一组文件，保留 macOS `adb` 的可执行权限，然后重新打包对应安装包。仓库不包含 SDK 中与本工具无关的其他命令。

### 应用图标

图标原图是 `src-tauri/icons/app-icon-source.png`。macOS 打包使用 `src-tauri/icons/icon.icns`，Windows 打包使用 `src-tauri/icons/icon.ico`，配置中还保留桌面端 PNG 尺寸。`src-tauri/Info.plist` 另外把同一图标指定为 macOS 上的 APK 文件类型图标；仅设置应用图标不会改变 Finder 中的 APK 图标。更新图标或文件关联后需要重新打包并安装应用。Tauri 的 `icon` 命令默认同时生成移动端和各桌面图标。以后更新图标时先把输出写到临时目录，再只复制 `icon.icns`、`icon.ico` 和已有 PNG 到 `src-tauri/icons/`，避免重新引入 `android/`、`ios/` 等目录。

## 实现约束

- 所有 `adb` 调用使用参数数组和 `std::process::Command`（或其异步等价物），不拼接 shell 命令；APK 路径、设备序列号和选项分别作为参数传入。
- 后端只接受存在的常规 `.apk` 文件；前端不能传任意可执行文件路径或自由形式的 adb 参数。安装前再次确认所选设备仍可用。
- 日志和错误信息避免泄露不必要的本机绝对路径；保留足够的 adb 失败原因供用户排查。

## 参考资料

- [Tauri 2 配置：文件关联](https://v2.tauri.app/reference/config/#fileassociations)
- [Tauri 文件打开事件示例](https://tauri.app/learn/mobile-file-associations/#handling-opened-files)（示例同时说明 macOS 的 `RunEvent::Opened`）
- [Tauri macOS 应用打包](https://v2.tauri.app/distribute/macos-application-bundle/)
- [Tauri Windows 安装包](https://v2.tauri.app/distribute/windows-installer/)
- [Tauri 单实例插件](https://v2.tauri.app/plugin/single-instance/)
- [Android Debug Bridge 文档](https://developer.android.com/tools/adb)
