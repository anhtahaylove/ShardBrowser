import { useState } from "react";
import { DialogModal } from "@proxyshard/shardx-ui-kit";
import { EditIcon } from "../../../shared/icons";
import { Field } from "../../../shared/ui/Field";
import { toast } from "../../../shared/model/toast";
import type { PsOrder } from "../../../entities/proxyshard";
import { psSetTag } from "../../../entities/proxyshard";

export function PsTagModal({ order, onClose, onDone }: { order: PsOrder; onClose: () => void; onDone: () => void }) {
  const [tag, setTag] = useState(order.tag && order.tag !== "none" ? order.tag : "");
  const [busy, setBusy] = useState(false);
  const submit = async () => {
    setBusy(true);
    try {
      await psSetTag(order.order_id, tag.trim() || "none");
      toast.ok(`Tag updated for #${order.order_id}`);
      onDone();
    } catch (e) { toast.err(String(e)); }
    finally { setBusy(false); }
  };
  return (
    <DialogModal
      open
      onClose={onClose}
      icon={<EditIcon className="size-5" />}
      title="Edit tag"
      subtitle={`${order.product_name} · order #${order.order_id}`}
      confirmLabel={busy ? "Saving…" : "Save"}
      onConfirm={submit}
      isLoading={busy}
      cancelLabel="Cancel"
      onCancel={onClose}
    >
      <div className="py-4">
        <Field label="Tag" value={tag} onChange={setTag} placeholder="leave empty to clear" />
      </div>
    </DialogModal>
  );
}
