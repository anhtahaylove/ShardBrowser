import { Button } from "@proxyshard/shardx-ui-kit";
import { AddIcon } from "../../../shared/icons";
import { useProxy } from "../../../entities/proxy";

export function NewProxyButton({ className }: { className?: string }) {
  const setBulkOpen = useProxy((s) => s.setBulkOpen);
  return (
    <Button
      variant="primary"
      mode="filled"
      size="small"
      className={className}
      leftIcon={<AddIcon className="size-4" />}
      onClick={() => setBulkOpen(true)}
    >
      New proxy
    </Button>
  );
}
