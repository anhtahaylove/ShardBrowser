import { Checkbox } from "@proxyshard/shardx-ui-kit";
import Badge from "../../shared/ui/Badge";
import type { ContextItem } from "../../shared/types";
import {
  useProxy,
  type ProxyEntry,
} from "../../entities/proxy";
import { ProxyRowActions, RenameProxyCell } from "../../features/manage-proxies";

import { ProxyTypeBadge, ProxyCountryCell, ProxyTestResult } from "../../entities/proxy";

export function ProxyRow({ proxy, profileCount, onMenu }: {
  proxy: ProxyEntry;
  profileCount: number;
  onMenu: (e: React.MouseEvent, items: ContextItem[]) => void;
}) {
  const snap = useProxy((s) => s.snapshots[proxy.id]);
  const busy = useProxy((s) => !!s.proxyTesting[proxy.id]);
  const isSel = useProxy((s) => s.proxySel.has(proxy.id));
  const selectProxy = useProxy((s) => s.selectProxy);
  const testProxy = useProxy((s) => s.testProxy);
  const removeProxy = useProxy((s) => s.removeProxy);
  const setEditing = useProxy((s) => s.setEditing);
  const setInfoFor = useProxy((s) => s.setInfoFor);

  return (
    <div
      className="relative border-t border-stroke-soft-200 first:border-t-0"
      onContextMenu={(e) =>
        onMenu(e, [
          { label: "Test (TCP/UDP/geo)", onClick: () => testProxy(proxy) },
          { label: "View details", onClick: () => setInfoFor({ proxy, anchor: { x: e.clientX, y: e.clientY } }) },
          { label: "Edit", onClick: () => setEditing(proxy) },
          { sep: true, label: "", onClick: () => { } },
          { label: "Delete", onClick: () => removeProxy(proxy.id), danger: true },
        ])
      }
    >
      <div className="p-cols transition-colors hover:bg-bg-weak-50">
        <div>
          <Checkbox
            checked={isSel}
            onChange={() => selectProxy(!isSel, [proxy])}
          />
        </div>
        <RenameProxyCell proxy={proxy} />
        <div><ProxyTypeBadge kind={proxy.kind} /></div>
        <div className="min-w-0 overflow-hidden">
          <span
            className="mono small inline-block max-w-full cursor-pointer overflow-hidden text-ellipsis whitespace-nowrap align-middle text-text-sub-600 transition-colors hover:text-primary-base"
            onClick={() => setEditing(proxy)}
            title="Edit proxy"
          >
            {proxy.host}:{proxy.port}
          </span>
        </div>
        <div><ProxyCountryCell snap={snap} fallback={proxy.country} /></div>
        <div>
          <Badge color="gray" variant='filled' size="small" title={`${profileCount} profile(s) bound to this proxy`}>
            {profileCount}
          </Badge>
        </div>
        <div className="min-w-0">
          <ProxyTestResult snap={snap} kind={proxy.kind} busy={busy} />
        </div>
        <ProxyRowActions proxy={proxy} />
      </div>
    </div>
  );
}
