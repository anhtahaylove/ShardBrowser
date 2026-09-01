import { Button } from "@proxyshard/shardx-ui-kit";
import { RefreshIcon, UploadIcon, DeleteIcon } from "../../../shared/icons";
import { useProxy } from "../../../entities/proxy";

export function BulkActionsBar() {
  const count = useProxy((s) => s.proxySel.size);
  const bulkTest = useProxy((s) => s.bulkTest);
  const bulkExport = useProxy((s) => s.bulkExport);
  const bulkDelete = useProxy((s) => s.bulkDelete);
  const clearSelected = useProxy((s) => s.clearSelected);

  if (count === 0) return null;

  return (
    <div className="flex items-center gap-2 rounded-8 bg-primary-alpha-10 py-1 pl-3 pr-1 text-label-xs text-primary-base ring-1 ring-inset ring-primary-alpha-24">
      <span>{count} selected</span>
      <Button variant="neutral" mode="stroke" size="2xsmall" leftIcon={<RefreshIcon className="size-3.5" />} onClick={bulkTest}>Test</Button>
      <Button variant="neutral" mode="stroke" size="2xsmall" leftIcon={<UploadIcon className="size-3.5" />} onClick={bulkExport}>Export</Button>
      <Button variant="error" mode="stroke" size="2xsmall" leftIcon={<DeleteIcon className="size-3.5" />} onClick={bulkDelete}>Delete</Button>
      <Button variant="neutral" mode="ghost" size="2xsmall" onClick={clearSelected}>Clear</Button>
    </div>
  );
}
