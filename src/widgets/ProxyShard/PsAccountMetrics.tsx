import { Metric } from "../../shared/ui/Metric";
import { fmtCents } from "../../shared/lib/utils";
import { usePsAccount, usePsConnected } from "../../entities/proxyshard";

export function PsAccountMetrics() {
  const me = usePsAccount((s) => s.me);
  const connected = usePsConnected();

  return (
    <div className="grid grid-cols-4 gap-[10px] mb-4">
      <Metric label="Account" value={connected ? "Connected" : "—"} accent={connected} pulse={connected} />
      <Metric label="Balance" value={me ? fmtCents(me.wallet_balance) : "—"} />
      <Metric label="Active orders" value={me ? String(me.active_orders) : "—"} />
    </div>
  );
}
