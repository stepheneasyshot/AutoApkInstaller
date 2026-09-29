import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import {
  defaultOptions, errorFrom, installApk, listDevices, takePendingApks,
  type AppError, type Device, type InstallOptions, type InstallReport,
} from "./api";
import "./App.css";

type Phase = "idle" | "checking" | "no_devices" | "choose" | "installing" | "result" | "error";

function fileName(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}

function App() {
  const [phase, setPhase] = useState<Phase>("idle");
  const [file, setFile] = useState<string | null>(null);
  const [devices, setDevices] = useState<Device[]>([]);
  const [selectedSerial, setSelectedSerial] = useState("");
  const [options, setOptions] = useState<InstallOptions>(defaultOptions);
  const [report, setReport] = useState<InstallReport | null>(null);
  const [problem, setProblem] = useState<AppError | null>(null);
  const [queued, setQueued] = useState(0);
  const queueRef = useRef<string[]>([]);
  const currentRef = useRef<string | null>(null);
  const installingRef = useRef(false);

  const install = useCallback(async (path: string, serial: string, installOptions: InstallOptions) => {
    if (installingRef.current) return;
    installingRef.current = true;
    setSelectedSerial(serial);
    setProblem(null);
    setPhase("installing");
    try {
      setReport(await installApk(path, serial, installOptions));
      setPhase("result");
    } catch (reason) {
      setProblem(errorFrom(reason));
      setPhase("error");
    } finally {
      installingRef.current = false;
    }
  }, []);

  const inspect = useCallback(async (path: string) => {
    currentRef.current = path;
    setFile(path);
    setReport(null);
    setProblem(null);
    setOptions(defaultOptions);
    setDevices([]);
    setSelectedSerial("");
    setPhase("checking");
    try {
      const found = await listDevices();
      setDevices(found);
      const online = found.filter((device) => device.state === "device");
      if (online.length === 0) {
        setPhase("no_devices");
      } else if (online.length === 1) {
        await install(path, online[0].serial, defaultOptions);
      } else {
        setPhase("choose");
      }
    } catch (reason) {
      setProblem(errorFrom(reason));
      setPhase("error");
    }
  }, [install]);

  const startNext = useCallback(() => {
    if (installingRef.current) return;
    const next = queueRef.current.shift();
    setQueued(queueRef.current.length);
    if (next) {
      void inspect(next);
    } else {
      currentRef.current = null;
      setFile(null);
      setPhase("idle");
    }
  }, [inspect]);

  const enqueue = useCallback((paths: string[]) => {
    queueRef.current.push(...paths);
    setQueued(queueRef.current.length);
    if (!currentRef.current) startNext();
  }, [startNext]);

  const drainOpenedFiles = useCallback(async () => {
    try {
      const paths = await takePendingApks();
      if (paths.length) enqueue(paths);
    } catch (reason) {
      setProblem(errorFrom(reason));
      setPhase("error");
    }
  }, [enqueue]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void (async () => {
      try {
        const stop = await listen("apk-opened", () => { void drainOpenedFiles(); });
        if (disposed) { stop(); return; }
        unlisten = stop;
        await drainOpenedFiles();
      } catch (reason) {
        setProblem(errorFrom(reason));
        setPhase("error");
      }
    })();
    return () => { disposed = true; unlisten?.(); };
  }, [drainOpenedFiles]);

  async function chooseFile() {
    try {
      const picked = await open({ multiple: false, directory: false, filters: [{ name: "Android APK", extensions: ["apk"] }] });
      if (typeof picked === "string") enqueue([picked]);
    } catch (reason) {
      setProblem(errorFrom(reason));
      setPhase("error");
    }
  }

  function refresh() {
    if (currentRef.current) void inspect(currentRef.current);
  }

  const available = devices.filter((device) => device.state === "device");
  const unavailable = devices.filter((device) => device.state !== "device");

  return (
    <main className="app-shell">
      <header className="topbar">
        <div className="brand"><span className="brand-mark">A</span><span>AutoApkInstaller</span></div>
        <button className="button button-quiet" onClick={() => void chooseFile()}>
          {file ? "添加 APK" : "选择 APK"}
        </button>
      </header>

      <section className={`content${file ? " has-file" : ""}`} data-phase={phase}>
        {!file && <div className="eyebrow">ANDROID · MACOS</div>}
        <h1>{file ? <>安装到<span>设备。</span></> : <>双击 APK，<br /><span>安装到设备。</span></>}</h1>
        {!file && <p className="intro">连接 Android 设备后，打开 APK 即可开始安装。</p>}

        {file && <div className="file-card">
          <div className="file-icon">APK</div>
          <div className="file-copy"><strong title={file}>{fileName(file)}</strong><span title={file}>{file}</span></div>
          <div className="file-tag">当前文件</div>
        </div>}

        <div className="panel" aria-live="polite">
          {phase === "idle" && <div className="empty-state">
            <div className="empty-icon">↓</div>
            <h2>准备安装</h2>
            <p>在 Finder 中用本应用打开一个 .apk 文件，或点击右上角选择文件。</p>
            <button className="button button-primary" onClick={() => void chooseFile()}>选择 APK 文件</button>
          </div>}

          {phase === "checking" && <div className="status-state">
            <div className="spinner" /><h2>正在检查设备</h2><p>正在读取 adb 设备列表…</p>
          </div>}

          {phase === "no_devices" && <div className="status-state">
            <div className="status-symbol amber">!</div>
            <h2>没有可用设备</h2>
            <p>请连接设备、开启 USB 调试，并在设备上允许此电脑调试，然后刷新。</p>
            {unavailable.length > 0 && <div className="device-notes">
              {unavailable.map((device) => <div key={device.serial}><strong>{device.serial}</strong><span>{device.state}</span></div>)}
            </div>}
            <button className="button button-primary" onClick={refresh}>刷新设备</button>
          </div>}

          {phase === "choose" && <div className="choose-state">
            <div className="panel-heading"><div><h2>选择安装设备</h2><p>检测到 {available.length} 台可用设备</p></div><button className="text-button" onClick={refresh}>刷新</button></div>
            <div className="device-list">
              {available.map((device) => <label className={`device-row ${selectedSerial === device.serial ? "selected" : ""}`} key={device.serial}>
                <input type="radio" name="device" value={device.serial} checked={selectedSerial === device.serial} onChange={() => setSelectedSerial(device.serial)} />
                <span className="device-dot" /><span className="device-text"><strong>{device.serial}</strong><small>{device.description || "Android 设备"}</small></span>
                <span className="online-label">已连接</span>
              </label>)}
            </div>
            <button className="button button-primary wide" disabled={!selectedSerial || !file} onClick={() => { if (file) void install(file, selectedSerial, defaultOptions); }}>安装到所选设备</button>
          </div>}

          {phase === "installing" && <div className="status-state">
            <div className="spinner" /><h2>正在安装</h2><p>目标设备：{selectedSerial}</p>
            <p className="muted">请保持设备连接，等待 adb 返回结果。</p>
          </div>}

          {phase === "result" && report && <div className="result-state">
            <div className={`status-symbol ${report.success ? "green" : "red"}`}>{report.success ? "✓" : "×"}</div>
            <h2>{report.success ? "安装成功" : "安装失败"}</h2>
            <p>目标设备：{selectedSerial}</p>
            <pre className="result-detail">{report.detail}</pre>
            <div className="options"><h3>调整参数后重试</h3>
              <label><input type="checkbox" checked={options.allowDowngrade} onChange={(event) => setOptions({ ...options, allowDowngrade: event.target.checked })} /><span>允许降级 <code>-d</code></span></label>
              <label><input type="checkbox" checked={options.grantPermissions} onChange={(event) => setOptions({ ...options, grantPermissions: event.target.checked })} /><span>授予清单权限 <code>-g</code></span></label>
              <label><input type="checkbox" checked={options.allowTestApk} onChange={(event) => setOptions({ ...options, allowTestApk: event.target.checked })} /><span>允许测试 APK <code>-t</code></span></label>
            </div>
            <div className="actions"><button className="button button-primary" onClick={() => { if (file) void install(file, selectedSerial, options); }}>重新安装</button><button className="button button-quiet" onClick={startNext}>{queued ? "处理下一个 APK" : "完成"}</button></div>
          </div>}

          {phase === "error" && <div className="status-state">
            <div className="status-symbol red">×</div><h2>{problem?.code === "adb_missing" ? "找不到 adb" : "无法继续安装"}</h2>
            <p>{problem?.message || "发生未知错误。"}</p>
            <div className="actions"><button className="button button-primary" onClick={refresh} disabled={!file}>重试</button><button className="button button-quiet" onClick={startNext}>{queued ? "处理下一个 APK" : "返回"}</button></div>
          </div>}
        </div>

        {queued > 0 && <p className="queue-note">还有 {queued} 个 APK 等待处理。当前任务结束后，点击“处理下一个 APK”。</p>}
      </section>
      <footer>通过 adb 安装 · 默认覆盖安装并保留应用数据</footer>
    </main>
  );
}

export default App;
