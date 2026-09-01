import { useMemo } from "react";
import type { ProxyEntry } from "../../proxy";
import { useProfile } from "./useProfile";

/// Folder tabs derived from profile assignments + the persisted registry of
/// empty folders. Sorted; "all" is rendered separately as the first tab.
export function useFolders() {
  const profiles = useProfile((s) => s.profiles);
  const folderRegistry = useProfile((s) => s.folderRegistry);
  return useMemo(() => {
    const set = new Set<string>(folderRegistry);
    for (const p of profiles) if (p.folder) set.add(p.folder);
    return [...set].sort((a, b) => a.localeCompare(b));
  }, [profiles, folderRegistry]);
}

/// Profiles filtered by the active folder tab and the search query.
export function useVisibleProfiles() {
  const profiles = useProfile((s) => s.profiles);
  const search = useProfile((s) => s.search);
  const folder = useProfile((s) => s.folder);
  return useMemo(
    () =>
      profiles.filter(
        (p) =>
          (folder === "all" || p.folder === folder) &&
          p.name.toLowerCase().includes(search.toLowerCase()),
      ),
    [profiles, search, folder],
  );
}

/// proxy_id → ProxyEntry lookup for the Proxy column.
export function useProxyMap() {
  const proxies = useProfile((s) => s.proxies);
  return useMemo(
    () => Object.fromEntries(proxies.map((p) => [p.id, p])) as Record<string, ProxyEntry>,
    [proxies],
  );
}

/// Count of currently-running engines (for the Running metric).
export function useRunningCount() {
  const running = useProfile((s) => s.running);
  return useMemo(() => Object.values(running).filter(Boolean).length, [running]);
}
