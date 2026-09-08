import { useState } from "react";
import { DialogModal, Textarea } from "@proxyshard/shardx-ui-kit";
import { Field } from "../../../shared/ui/Field";
import { toast } from "../../../shared/model/toast";
import { fingerprintImport } from "../../../entities/fingerprint";

export function FingerprintImporter({ onClose }: { onClose: () => void }) {
  const [text, setText] = useState("");
  const [name, setName] = useState("");
  const save = async () => {
    try {
      const e = await fingerprintImport(text, name || null);
      toast.ok(`Imported "${e.label}"`);
      onClose();
    } catch (e) { toast.err(String(e)); }
  };
  return (
    <DialogModal
      open
      onClose={onClose}
      title="Paste FingerprintConfig JSON"
      maxWidthClassName="max-w-[880px]"
      confirmLabel="Import"
      onConfirm={save}
      cancelLabel="Cancel"
      onCancel={onClose}
    >
      <div className="flex flex-col gap-3 py-4">
        <Field label="Name (optional, becomes the file id)" value={name} onChange={setName} placeholder="e.g. mac-m4-pro-real" />
        <Textarea
          label="Paste the full JSON"
          rows={14}
          className="mono w-[500px]"
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder='{ "name": "...", "navigator": { ... }, "webgl": { ... }, ... }'
        />
      </div>
    </DialogModal>
  );
}
