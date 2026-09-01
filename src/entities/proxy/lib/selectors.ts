import { useMemo } from "react";
import { useProxy } from "./useProxy";

/// Search-filtered proxies, matching name/host/port/country/notes/user + geo snapshot.
export function useFilteredProxies() {
    const proxies = useProxy((s) => s.proxies);
    const snapshots = useProxy((s) => s.snapshots);
    const search = useProxy((s) => s.search);
    return useMemo(() => {
        const q = search.trim().toLowerCase();
        if (!q) return proxies;
        return proxies.filter((p) => {
            const ip = (snapshots[p.id]?.ip ?? "").toLowerCase();
            const city = (snapshots[p.id]?.city ?? "").toLowerCase();
            const isp = (snapshots[p.id]?.isp ?? "").toLowerCase();
            return (
                p.name.toLowerCase().includes(q) ||
                p.host.toLowerCase().includes(q) ||
                String(p.port).includes(q) ||
                p.country.toLowerCase().includes(q) ||
                p.notes.toLowerCase().includes(q) ||
                p.username.toLowerCase().includes(q) ||
                ip.includes(q) ||
                city.includes(q) ||
                isp.includes(q)
            );
        });
    }, [proxies, snapshots, search]);
}

/// proxy_id → bound-profile count (O(n) tally; n is small).
export function useProfileCountByProxy() {
    const profiles = useProxy((s) => s.profiles);
    return useMemo(() => {
        const out: Record<string, number> = {};
        for (const p of profiles) {
            if (p.proxy_id) out[p.proxy_id] = (out[p.proxy_id] ?? 0) + 1;
        }
        return out;
    }, [profiles]);
}
