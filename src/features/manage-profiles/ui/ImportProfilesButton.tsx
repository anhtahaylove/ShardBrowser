import { Button } from "@proxyshard/shardx-ui-kit";
import { DownloadIcon } from "../../../shared/icons";
import { useProfile } from "../../../entities/profile";

export function ImportProfilesButton() {
  const bulkImport = useProfile((s) => s.bulkImport);
  return (
    <Button
      variant="neutral"
      mode="stroke"
      size="small"
      leftIcon={<DownloadIcon className="size-4" />}
      onClick={bulkImport}
      title="Create profiles from exported JSON in the clipboard"
    >
      Import
    </Button>
  );
}
