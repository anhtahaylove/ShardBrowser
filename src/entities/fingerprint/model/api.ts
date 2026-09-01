import { invoke } from "@tauri-apps/api/core";
import type { FingerprintEntry } from "./types";

export const fingerprintList = () => invoke<FingerprintEntry[]>("fingerprint_list");
export const fingerprintDelete = (id: string) => invoke("fingerprint_delete", { id });
export const fingerprintImport = (jsonText: string, idHint: string | null) => invoke<FingerprintEntry>("fingerprint_import", { jsonText, idHint });
export const fingerprintDir = () => invoke<string>("fingerprint_dir");
