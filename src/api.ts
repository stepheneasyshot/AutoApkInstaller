import { invoke } from "@tauri-apps/api/core";

export interface Device {
  serial: string;
  state: string;
  description: string;
}

export interface InstallOptions {
  allowDowngrade: boolean;
  grantPermissions: boolean;
  allowTestApk: boolean;
}

export interface InstallReport {
  success: boolean;
  detail: string;
}

export interface AppError {
  code: string;
  message: string;
}

export const defaultOptions: InstallOptions = {
  allowDowngrade: false,
  grantPermissions: false,
  allowTestApk: false,
};

export function errorFrom(reason: unknown): AppError {
  if (reason && typeof reason === "object" && "message" in reason) {
    const value = reason as { code?: unknown; message: unknown };
    return {
      code: typeof value.code === "string" ? value.code : "unknown",
      message: String(value.message),
    };
  }
  return { code: "unknown", message: String(reason) };
}

export const takePendingApks = () => invoke<string[]>("take_pending_apks");
export const listDevices = () => invoke<Device[]>("list_devices");
export const installApk = (path: string, serial: string, options: InstallOptions) =>
  invoke<InstallReport>("install_apk", { path, serial, options });
