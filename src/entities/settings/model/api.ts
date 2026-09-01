import { invoke } from "@tauri-apps/api/core";
import type { Settings, ApiInfo } from "./types";

export const settingsGet = () => invoke<Settings>("settings_get");
export const settingsSave = (value: Settings) => invoke("settings_save", { value });
export const apiInfo = () => invoke<ApiInfo>("api_info");
export const apiRegenerateToken = () => invoke<ApiInfo>("api_regenerate_token");
export const mcpDownload = (dir: string) => invoke<string>("mcp_download", { dir });
