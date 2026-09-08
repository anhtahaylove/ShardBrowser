import { openUrl } from "@tauri-apps/plugin-opener";
import { Button } from "@proxyshard/shardx-ui-kit";
import { ShardMini } from "../../../shared/icons";
import { DASHBOARD_URL } from "../../../shared/lib/utils";

export function OpenDashboardButton() {
  return (
    <Button
      variant="primary"
      mode="lighter"
      size="small"
      leftIcon={<ShardMini />}
      onClick={() => { openUrl(DASHBOARD_URL).catch(() => {}); }}
      title="Open the ProxyShard dashboard in your browser"
    >
      Open dashboard
    </Button>
  );
}
