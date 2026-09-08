import { Button } from "@proxyshard/shardx-ui-kit";
import { FolderIcon } from "../../../shared/icons";
import { useFingerprint } from "../../../entities/fingerprint";

export function ImportFromFileButton() {
  const importJsonFile = useFingerprint((s) => s.importJsonFile);
  return (
    <Button variant="neutral" mode="stroke" size="small" leftIcon={<FolderIcon className="size-4" />} onClick={importJsonFile}>
      Import from file
    </Button>
  );
}
