import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Button } from "@proxyshard/shardx-ui-kit";
import { DownloadIcon } from "../../shared/icons";
import { toast } from "../../shared/model/toast";

export function DownloadMcp() {
  const [mcpBusy, setMcpBusy] = useState(false);

  const downloadMcp = async () => {
    const dir = await open({ directory: true, title: "Where to download the MCP server" });
    if (typeof dir !== "string") return;
    setMcpBusy(true);
    try {
      const p = await invoke<string>("mcp_download", { dir });
      toast.ok(`MCP downloaded to ${p}`);
    } catch (e) {
      toast.err("MCP download failed: " + String(e));
    } finally {
      setMcpBusy(false);
    }
  };

  return (
    <Button
      variant="primary"
      mode="lighter"
      size="xsmall"
      className="w-full"
      isLoading={mcpBusy}
      leftIcon={<DownloadIcon className="size-4" />}
      onClick={downloadMcp}
    >
      {mcpBusy ? "Downloading…" : "Download MCP"}
    </Button>
  );
}
