import type { ReactNode } from "react";
import Badge from "../../../shared/ui/Badge";
import type { FingerprintEntry } from "../model/types";

export function FingerprintCard({ entry, actions }: { entry: FingerprintEntry; actions?: ReactNode }) {
  return (
    <div
      className="relative flex flex-col gap-1.5 rounded-10 border-l-[3px] bg-bg-white-0 px-3.5 py-3 shadow-[var(--shadow-xs)] ring-1 ring-inset ring-stroke-soft-200 transition-colors hover:bg-bg-weak-50 hover:ring-stroke-sub-300"
      style={{ borderLeftColor: entry.tag_color ?? "var(--color-primary-base)" }}
    >
      <div className="flex items-baseline justify-between gap-2.5">
        <span className="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-label-xs leading-[1.25] text-text-strong-950">{entry.label}</span>
        {entry.chrome && <Badge color="gray" variant="filled" size="small" className="flex-none">Chrome {entry.chrome}</Badge>}
      </div>
      <div className="mono overflow-hidden text-ellipsis whitespace-nowrap text-[11px] text-text-soft-400" title={entry.gpu}>{entry.gpu || "—"}</div>
      {actions && (
        <div className="mt-1 flex items-center gap-1.5 border-t border-stroke-soft-200 pt-2">
          {actions}
        </div>
      )}
    </div>
  );
}
