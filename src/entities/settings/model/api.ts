import { invoke } from "@tauri-apps/api/core";
import type { Settings, ApiInfo, DataRootInfo, StartupStatus, McpStatus, CodexMcpStatus } from "./types";

export const settingsGet = () => invoke<Settings>("settings_get");
export const settingsSave = (value: Settings) => invoke("settings_save", { value });
export const apiInfo = () => invoke<ApiInfo>("api_info");
export const apiRegenerateToken = () => invoke<ApiInfo>("api_regenerate_token");
export const mcpDownload = (dir: string) => invoke<string>("mcp_download", { dir });

export const startupStatus = () => invoke<StartupStatus>("startup_status");
export const mcpStatus = () => invoke<McpStatus>("mcp_status");
export const codexMcpStatus = () => invoke<CodexMcpStatus>("codex_mcp_status");

export const dataRootGet = () => invoke<DataRootInfo>("data_root_get");
/** Moves the data; progress arrives as `data-migration` events. */
export const dataRootMigrate = (path: string) => invoke<number>("data_root_migrate", { path });
