import { create } from "zustand";
import { open, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import { toast } from "../../../shared/lib/toast";
import { confirmModal } from "../../../shared/lib/confirm";
import { clip } from "../../../shared/lib/clipboard";
import { readTextFile } from "../../../shared/lib/utils";
import { proxyList, type ProxyEntry } from "../../proxy";
import { fingerprintList, type FingerprintEntry } from "../../fingerprint";
import type { ProfileMeta, ProfileForm } from "../model/types";
import {
  profileList, profileGet, profileSave, profileDelete, profileClone,
  profileSetPin, profileSetFolder, profileBindProxy, profileImport,
  profileCreateFromTemplate, processList, processKill, launch,
  folderDelete, cookiesExportToFile, cookiesImport,
} from "../model/api";
import { defaultForm, fromStored, toStored } from "../model/form";

const FOLDERS_KEY = "shardx-folders";

const loadFolderRegistry = (): string[] => {
  try { return JSON.parse(localStorage.getItem(FOLDERS_KEY) || "[]"); }
  catch { return []; }
};

export type QuickEditTarget = { kind: "proxy" | "notes"; profile: ProfileMeta };
export type FolderModalTarget = { profileId: string | null };

export type ProfileStore = {
  status: "idle" | "loading" | "ready" | "error";
  error: string | null;

  profiles: ProfileMeta[];
  proxies: ProxyEntry[];
  fingerprints: FingerprintEntry[];

  /// Value = epoch ms at which the engine was first observed running. Used both
  /// as a truthy flag (any number = running) and as the anchor for the ticking
  /// uptime display in the Status column.
  running: Record<string, number>;
  /// Profiles whose `launch()` call is in-flight (pre-flight probes can be slow).
  startBusy: Set<string>;
  selected: Set<string>;

  // UI state lives in the store so feature buttons stay prop-free.
  search: string;
  folder: string;
  expanded: string | null;
  draft: ProfileForm | null;
  /// Empty folders persist here until a profile lands in them.
  folderRegistry: string[];
  folderModal: FolderModalTarget | null;
  /// Folder name currently highlighted as a drag-and-drop target ("__all__"
  /// for the All tab). Cleared in dragleave/drop.
  dropTarget: string | null;
  templatePickerOpen: boolean;
  quickEdit: QuickEditTarget | null;

  init: () => Promise<void>;
  reload: () => Promise<void>;
  startProcessPolling: () => () => void;

  setSearch: (q: string) => void;
  setFolder: (f: string) => void;
  setDraft: (draft: ProfileForm | null) => void;
  setDropTarget: (target: string | null) => void;
  setTemplatePickerOpen: (open: boolean) => void;
  setQuickEdit: (target: QuickEditTarget | null) => void;
  setFolderModal: (target: FolderModalTarget | null) => void;

  rememberFolder: (f: string) => void;
  forgetFolder: (f: string) => void;

  selectProfiles: (isChecked: boolean, profiles: ProfileMeta[]) => void;
  toggleSelect: (id: string) => void;
  clearSelected: () => void;

  expand: (id: string) => Promise<void>;
  newProfile: () => void;
  cancelEdit: () => void;
  saveDraft: () => Promise<void>;

  startStop: (p: ProfileMeta) => Promise<void>;
  remove: (id: string) => Promise<void>;
  cloneProfile: (id: string) => Promise<void>;
  togglePin: (p: ProfileMeta) => Promise<void>;
  exportCookies: (p: ProfileMeta) => Promise<void>;
  importCookies: (p: ProfileMeta) => Promise<void>;

  setProfileFolder: (id: string, f: string) => Promise<void>;
  deleteFolder: (f: string) => Promise<void>;
  createFromTemplate: (tplId: string) => Promise<void>;

  bulkLaunch: () => Promise<void>;
  bulkStop: () => Promise<void>;
  bulkDelete: () => Promise<void>;
  bulkExport: () => Promise<void>;
  bulkImport: () => Promise<void>;
};

export const useProfile = create<ProfileStore>((set, get) => ({
  status: "idle",
  error: null,

  profiles: new Array<ProfileMeta>(),
  proxies: new Array<ProxyEntry>(),
  fingerprints: new Array<FingerprintEntry>(),

  running: {},
  startBusy: new Set<string>(),
  selected: new Set<string>(),

  search: "",
  folder: "all",
  expanded: null,
  draft: null,
  folderRegistry: loadFolderRegistry(),
  folderModal: null,
  dropTarget: null,
  templatePickerOpen: false,
  quickEdit: null,

  init: async () => {
    if (get().status === "loading" || get().status === "ready") return;
    set({ status: "loading" });
    try {
      const [profiles, proxies, fingerprints] = await Promise.all([
        profileList(), proxyList(), fingerprintList(),
      ]);
      set({ profiles, proxies, fingerprints, status: "ready" });
    } catch (e) {
      set({ status: "error", error: (e as Error).message });
      toast.err(String(e));
    }
  },

  reload: async () => {
    try {
      const [profiles, proxies] = await Promise.all([profileList(), proxyList()]);
      set({ profiles, proxies });
    } catch (e) { toast.err(String(e)); }
  },

  // 2s poll for real child status; not optimistic UI state. Uptime is anchored
  // to the moment the engine actually started (now - uptime_ms), preserved
  // across polls so the displayed clock doesn't jitter. When a profile
  // transitions running → not-running, the backend has just bumped its persisted
  // total_runtime_ms — re-fetch so the Time column reflects the new total.
  startProcessPolling: () => {
    let cancelled = false;
    const tick = async () => {
      try {
        const list = await processList();
        if (cancelled) return;
        const now = Date.now();
        const prev = get().running;
        const next: Record<string, number> = {};
        for (const r of list) {
          next[r.profile_id] = prev[r.profile_id] ?? (now - r.uptime_ms);
        }
        const justExited = Object.keys(prev).some((id) => !(id in next));
        set({ running: next });
        if (justExited) get().reload();
      } catch {}
    };
    tick();
    const handle = setInterval(tick, 2000);
    return () => { cancelled = true; clearInterval(handle); };
  },

  setSearch: (search) => set({ search }),
  setFolder: (folder) => set({ folder }),
  setDraft: (draft) => set({ draft }),
  setDropTarget: (dropTarget) => set({ dropTarget }),
  setTemplatePickerOpen: (templatePickerOpen) => set({ templatePickerOpen }),
  setQuickEdit: (quickEdit) => set({ quickEdit }),
  setFolderModal: (folderModal) => set({ folderModal }),

  rememberFolder: (f) => {
    const next = get().folderRegistry.includes(f)
      ? get().folderRegistry
      : [...get().folderRegistry, f];
    localStorage.setItem(FOLDERS_KEY, JSON.stringify(next));
    set({ folderRegistry: next });
  },
  forgetFolder: (f) => {
    const next = get().folderRegistry.filter((x) => x !== f);
    localStorage.setItem(FOLDERS_KEY, JSON.stringify(next));
    set({ folderRegistry: next });
  },

  selectProfiles: (isChecked, profiles) => {
    const next = new Set(get().selected);
    if (isChecked) {
      for (const p of profiles) next.add(p.id);
    } else {
      for (const p of profiles) next.delete(p.id);
    }
    set({ selected: next });
  },
  toggleSelect: (id) => {
    const next = new Set(get().selected);
    if (next.has(id)) next.delete(id); else next.add(id);
    set({ selected: next });
  },
  clearSelected: () => set({ selected: new Set<string>() }),

  expand: async (id) => {
    if (get().expanded === id) { set({ expanded: null, draft: null }); return; }
    const stored = await profileGet(id);
    set({ draft: fromStored(stored), expanded: id });
  },
  newProfile: () => set({ draft: defaultForm(), expanded: "__new__" }),
  cancelEdit: () => set({ expanded: null, draft: null }),

  saveDraft: async () => {
    const { draft, fingerprints, folder } = get();
    if (!draft) return;
    try {
      const fp = fingerprints.find((g) => g.id === draft.gpu_preset_id) ?? null;
      const saved = await profileSave(toStored(draft, fp));
      await profileBindProxy(saved.id, draft.proxy_id);
      // A profile created while a folder tab is active should land in that
      // folder (otherwise it pops into "All" and the user has to drag it back).
      // `!draft.id` scopes this to creations only — edits keep their folder.
      if (!draft.id && folder && folder !== "all") {
        try { await profileSetFolder(saved.id, folder); }
        catch (e) { console.warn("auto-assign folder failed:", e); }
      }
      set({ expanded: null, draft: null });
      get().reload();
      toast.ok(draft.id ? "Profile saved" : `Created "${saved.name}"`);
    } catch (e) { toast.err(String(e)); }
  },

  // Block the Start button until launch() returns. The launch includes
  // pre-flight steps that can take real time (UDP probe, geo, Widevine
  // pre-warm); surfacing the busy state is what the user reads as "did it work?".
  startStop: async (p) => {
    if (get().running[p.id]) {
      try { await processKill(p.id); }
      catch (e) { toast.err(String(e)); }
      return;
    }
    if (get().startBusy.has(p.id)) return;
    set({ startBusy: new Set([...get().startBusy, p.id]) });
    try {
      await launch(p.id);
      // Don't optimistically flip `running`; the 2s poll picks up the new child.
    } catch (e) {
      toast.err(String(e));
    } finally {
      const n = new Set(get().startBusy);
      n.delete(p.id);
      set({ startBusy: n });
    }
  },

  remove: async (id) => {
    if ((await confirmModal({ title: "Delete profile", message: "Delete this profile? Its user-data dir is wiped too.", danger: true })) !== true) return;
    await profileDelete(id);
    get().reload();
  },

  cloneProfile: async (id) => {
    try { await profileClone(id); get().reload(); }
    catch (e) { toast.err(String(e)); }
  },

  togglePin: async (p) => {
    try { await profileSetPin(p.id, !p.pinned); get().reload(); }
    catch (e) { toast.err(String(e)); }
  },

  exportCookies: async (p) => {
    try {
      const path = await saveDialog({
        defaultPath: `${(p.name || p.id).replace(/[^\w.-]+/g, "_")}-cookies.json`,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (typeof path !== "string") return; // cancelled
      const n = await cookiesExportToFile(p.id, path);
      toast.ok(`Exported ${n} cookie${n === 1 ? "" : "s"}`);
      // Open the containing folder so the user sees exactly where it went.
      const dir = path.replace(/[/\\][^/\\]*$/, "");
      try { await openPath(dir); } catch {}
    } catch (e) { toast.err(String(e)); }
  },

  importCookies: async (p) => {
    if (get().running[p.id]) { toast.err("Stop the profile before importing cookies"); return; }
    try {
      const path = await open({
        multiple: false, directory: false, title: "Select cookies JSON",
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (typeof path !== "string") return;
      const text = await readTextFile(path);
      const cookies = JSON.parse(text);
      if (!Array.isArray(cookies)) { toast.err("Expected a JSON array of cookies"); return; }
      const n = await cookiesImport(p.id, cookies);
      toast.ok(`Imported ${n} cookie${n === 1 ? "" : "s"}`);
    } catch (e) { toast.err(String(e)); }
  },

  setProfileFolder: async (id, f) => {
    // Dropping a profile onto the folder it already lives in is a no-op — tell
    // the user instead of silently doing nothing.
    const p = get().profiles.find((x) => x.id === id);
    if (p && p.folder === f) {
      const who = p.name || id.slice(0, 8);
      toast.info(f ? `"${who}" is already in "${f}"` : `"${who}" isn't in any folder`);
      return;
    }
    try {
      await profileSetFolder(id, f);
      if (f) get().rememberFolder(f);
      get().reload();
    } catch (e) { toast.err(String(e)); }
  },

  deleteFolder: async (f) => {
    const count = get().profiles.filter((p) => p.folder === f).length;
    // Three outcomes: delete profiles, unfile, cancel.
    const choice = await confirmModal({
      title: `Delete folder "${f}"`,
      message:
        count > 0
          ? `This folder has ${count} profile${count === 1 ? "" : "s"}. ` +
            `Delete them too, or keep them (they move to "All")?`
          : `Delete the empty folder "${f}"?`,
      buttons:
        count > 0
          ? [
              { label: "Cancel", value: "cancel" },
              { label: "Keep profiles", value: "keep" },
              { label: "Delete profiles", value: "delete", danger: true },
            ]
          : [
              { label: "Cancel", value: "cancel" },
              { label: "Delete", value: "keep", danger: true },
            ],
    });
    if (choice == null || choice === "cancel") return;
    const alsoDelete = choice === "delete";
    try {
      const n = await folderDelete(f, alsoDelete);
      // The folder lives in two places: profile tags (cleared by folder_delete)
      // and the localStorage registry of empty folders. Drop it from the
      // registry too, otherwise the tab lingers after every profile is gone.
      get().forgetFolder(f);
      if (get().folder === f) set({ folder: "all" });
      get().reload();
      toast.ok(
        alsoDelete
          ? `Deleted folder "${f}" + ${n} profile${n === 1 ? "" : "s"}`
          : `Removed folder "${f}" (${n} profile${n === 1 ? "" : "s"} kept)`,
      );
    } catch (e) { toast.err(String(e)); }
  },

  createFromTemplate: async (tplId) => {
    try {
      const meta = await profileCreateFromTemplate(tplId);
      set({ templatePickerOpen: false });
      get().reload();
      toast.ok(`Profile "${meta.name}" created`);
      // Auto-open the new profile in the editor.
      const stored = await profileGet(meta.id);
      set({ draft: fromStored(stored), expanded: meta.id });
    } catch (e) { toast.err(String(e)); }
  },

  bulkLaunch: async () => {
    const { selected, running } = get();
    for (const id of selected) {
      if (running[id]) continue;
      try { await launch(id); } catch {}
    }
    get().clearSelected();
  },

  bulkStop: async () => {
    for (const id of get().selected) {
      try { await processKill(id); } catch {}
    }
    get().clearSelected();
  },

  bulkDelete: async () => {
    const ids = [...get().selected];
    if (ids.length === 0) return;
    if ((await confirmModal({ title: "Delete profiles", message: `Delete ${ids.length} profile${ids.length === 1 ? "" : "s"}? This wipes their user-data dirs too.`, danger: true })) !== true) return;
    for (const id of ids) {
      try { await profileDelete(id); } catch (e) { toast.err(String(e)); }
    }
    get().clearSelected();
    get().reload();
    toast.ok(`Deleted ${ids.length}`);
  },

  // Dump selected profile FingerprintConfigs as a JSON array to clipboard.
  bulkExport: async () => {
    const ids = [...get().selected];
    if (ids.length === 0) return;
    try {
      const payloads = await Promise.all(ids.map((id) => profileGet(id)));
      await clip.write(JSON.stringify(payloads, null, 2));
      toast.ok(`Copied ${payloads.length} to clipboard`);
    } catch (e) { toast.err(String(e)); }
  },

  // Paste profile JSON from clipboard → fresh profiles.
  bulkImport: async () => {
    try {
      const text = await clip.read();
      if (!text.trim()) { toast.err("Clipboard is empty"); return; }
      const data = JSON.parse(text);
      const arr = Array.isArray(data) ? data : [data];
      const n = await profileImport(arr);
      get().reload();
      toast.ok(`Imported ${n} profile${n === 1 ? "" : "s"}`);
    } catch (e) { toast.err("Import failed: " + String(e)); }
  },
}));
