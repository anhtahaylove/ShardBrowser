import { Button } from "@proxyshard/shardx-ui-kit";
import { FolderIcon } from "../../../shared/icons";
import { useFingerprint } from "../../../entities/fingerprint";

export function LibraryFolderButton() {
  const openLibraryFolder = useFingerprint((s) => s.openLibraryFolder);
  return (
    <Button
      variant="neutral"
      mode="stroke"
      size="small"
      leftIcon={<FolderIcon className="size-4" />}
      onClick={openLibraryFolder}
      title="Reveal the on-disk library folder; drop JSONs here to add them"
    >
      Library folder
    </Button>
  );
}
