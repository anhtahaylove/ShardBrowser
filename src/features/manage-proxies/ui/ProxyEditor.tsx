import { useEffect, useMemo, useState } from "react";
import { DialogModal, Select } from "@proxyshard/shardx-ui-kit";
import { Field } from "../../../shared/ui/Field";
import { NumField } from "../../../shared/ui/NumField";
import { CSSelect } from "../../../shared/ui/CSSelect";
import { toast } from "../../../shared/model/toast";
import { PS_SIGNATURES } from "../../../entities/proxyshard";
import type { ProxyEntry } from "../../../entities/proxy";
import { proxySave } from "../../../entities/proxy";
import { psActive, psSignatureSet } from "../../../entities/proxyshard";

export function ProxyEditor({ initial, onClose }: { initial: ProxyEntry; onClose: () => void }) {
  const [p, setP] = useState<ProxyEntry>(initial);
  // DC/ISP proxies imported from a ProxyShard order carry the order id in
  // notes — enables editing their p0f OS signature here.
  const orderId = useMemo(() => {
    const m = initial.notes.match(/ProxyShard order (\d+)/);
    return m ? Number(m[1]) : null;
  }, [initial.notes]);
  const [sig, setSig] = useState("");
  const [curSig, setCurSig] = useState("");
  // Pull the IP's currently-set p0f signature from the order's active list.
  useEffect(() => {
    if (!orderId) return;
    psActive(orderId)
      .then((r) => {
        const found = (r.data ?? []).find((d: any) => d.ip === initial.host);
        const s = found?.signature ?? "";
        setSig(s);
        setCurSig(s);
      })
      .catch(() => {});
  }, [orderId]);
  const save = async () => {
    try {
      await proxySave(p);
      // Apply the p0f signature only when it changed to a non-empty value.
      if (orderId && sig && sig !== curSig) {
        try {
          await psSignatureSet(orderId, [{ ip: p.host, signature: sig }]);
          toast.ok(`p0f set to ${sig}`);
        } catch (e) { toast.err("p0f: " + String(e)); }
      }
      toast.ok(initial.id ? "Proxy saved" : "Proxy added");
      onClose();
    } catch (e) { toast.err(String(e)); }
  };
  return (
    <DialogModal
      open
      onClose={onClose}
      title={initial.id ? "Edit proxy" : "New proxy"}
      confirmLabel="Save"
      onConfirm={save}
      cancelLabel="Cancel"
      onCancel={onClose}
    >
      <div className="flex flex-col gap-3 py-4">
        <Field label="Name" value={p.name} onChange={(v: string) => setP({ ...p, name: v })} />
        <div className="grid grid-cols-2 gap-3">
          <Select
            label="Type"
            size="small"
            value={p.kind}
            onChange={(v) => setP({ ...p, kind: v as ProxyEntry["kind"] })}
            options={[
              { value: "socks5", label: "SOCKS5" },
              { value: "http", label: "HTTP" },
              { value: "https", label: "HTTPS" },
            ]}
          />
          <Field label="Country" value={p.country} onChange={(v: string) => setP({ ...p, country: v })} />
        </div>
        <div className="grid grid-cols-2 gap-3">
          <Field label="Host" value={p.host} onChange={(v: string) => setP({ ...p, host: v })} />
          <NumField label="Port" value={p.port} onChange={(v) => setP({ ...p, port: v as any })} />
        </div>
        <div className="grid grid-cols-2 gap-3">
          <Field label="Username" value={p.username} onChange={(v: string) => setP({ ...p, username: v })} />
          <Field label="Password" value={p.password} onChange={(v: string) => setP({ ...p, password: v })} type="password" />
        </div>
        {orderId && (
          <div className="grid grid-cols-2 gap-3">
            <label className="flex flex-col gap-1">
              <span className="text-label-xs text-text-sub-600">
                p0f signature · order #{orderId}
                <span className="text-text-soft-400"> · current: {curSig || "none"}</span>
              </span>
              <CSSelect value={sig} onChange={setSig} options={PS_SIGNATURES} placeholder="Don't change" />
            </label>
            <div />
          </div>
        )}
      </div>
    </DialogModal>
  );
}
