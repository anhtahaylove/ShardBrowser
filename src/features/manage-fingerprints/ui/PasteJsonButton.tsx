import { Button } from "@proxyshard/shardx-ui-kit";
import { AddIcon } from "../../../shared/icons";
import { useFingerprint } from "../../../entities/fingerprint";

export function PasteJsonButton() {
  const setImporterOpen = useFingerprint((s) => s.setImporterOpen);
  return (
    <Button variant="primary" mode="filled" size="small" leftIcon={<AddIcon className="size-4" />} onClick={() => setImporterOpen(true)}>
      Paste JSON
    </Button>
  );
}
