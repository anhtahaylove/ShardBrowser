import { Button } from "@proxyshard/shardx-ui-kit";
import { DownloadIcon } from "../../../shared/icons";
import { useProxy } from "../../../entities/proxy";

export function ImportProxyButton() {
  const bulkImportClipboard = useProxy((s) => s.bulkImportClipboard);
  return (
    <Button
      variant="neutral"
      mode="stroke"
      size="small"
      leftIcon={<DownloadIcon className="size-4" />}
      onClick={bulkImportClipboard}
      title="Import proxies from the clipboard"
    >
      Import
    </Button>
  );
}
