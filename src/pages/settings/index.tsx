import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Button, Input, Select, Switch } from "@proxyshard/shardx-ui-kit";
import { DownloadIcon } from "../../shared/icons";
import { Topbar } from "../../shared/ui/Topbar";
import { CopyField } from "../../shared/ui/CopyField";
import { toast } from "../../shared/model/toast";
import { withUtm } from "../../shared/lib/utils";
import type { Settings, ApiInfo } from "../../entities/settings";
import { settingsGet, settingsSave, apiInfo, apiRegenerateToken, mcpDownload } from "../../entities/settings";

function SettingsCard({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="mb-3.5 rounded-lg bg-bg-white-0 p-[18px] shadow-[var(--shadow-xs)] ring-1 ring-inset ring-stroke-soft-200">
      <h3 className="m-0 mb-1.5 text-label-sm text-text-strong-950">{title}</h3>
      {children}
    </div>
  );
}

export function SettingsPage() {
  const [s, setS] = useState<Settings>({
    browser_path: null,
    theme: "dark",
    geo_checker: "ip-api.com",
    screen_resolution_mode: "fingerprint",
    api_enabled: true,
    api_port: 40325,
  });
  const [api, setApi] = useState<ApiInfo | null>(null);
  const refreshApi = () => apiInfo().then(setApi).catch(() => {});
  useEffect(() => { settingsGet().then(setS); refreshApi(); }, []);
  const regenToken = async () => {
    try { setApi(await apiRegenerateToken()); toast.ok("Token regenerated"); }
    catch (e) { toast.err(String(e)); }
  };

  const [mcpBusy, setMcpBusy] = useState(false);
  // Download MCP server source; user manages install + client setup.
  const downloadMcp = async () => {
    const dir = await open({ directory: true, title: "Where to download the MCP server" });
    if (typeof dir !== "string") return;
    setMcpBusy(true);
    try {
      const path = await mcpDownload(dir);
      toast.ok(`MCP downloaded to ${path}`);
    } catch (e) { toast.err("MCP download failed: " + String(e)); }
    finally { setMcpBusy(false); }
  };
  const save = async () => {
    try { await settingsSave(s); toast.ok("Settings saved"); }
    catch (e) { toast.err(String(e)); }
  };
  return (
    <section className="flex flex-col">
      <Topbar crumbs={["System", "Settings"]} search="" onSearch={() => {}} />
      <div className="mb-3.5 flex items-end justify-between gap-4">
        <h1 className="m-0 text-title-h5 text-text-strong-950">Settings</h1>
      </div>

      <SettingsCard title="Proxy geo checker">
        <p className="m-0 mb-2 text-paragraph-xs text-text-soft-400">
          Which free public IP-geo service to hit when you press the proxy <strong>Test</strong> button. All three are no-key, rate-limited.
        </p>
        <Select
          label="Provider"
          size="small"
          value={s.geo_checker ?? "ip-api.com"}
          onChange={(v) => setS({ ...s, geo_checker: v })}
          options={[
            { value: "ip-api.com", label: "ip-api.com (45 req/min, HTTP)" },
            { value: "ipapi.co", label: "ipapi.co (1k/day, HTTPS)" },
            { value: "ipwho.is", label: "ipwho.is (10k/month, HTTPS)" },
          ]}
        />
      </SettingsCard>

      <SettingsCard title="Screen resolution">
        <p className="m-0 mb-2 text-paragraph-xs text-text-soft-400">
          <strong>From fingerprint</strong> reports the screen carried in the bound profile (recommended for anti-detect coherence).
          <strong> Real</strong> lets ShardX expose the host monitor's actual size.
        </p>
        <Select
          label="Mode"
          size="small"
          value={s.screen_resolution_mode ?? "fingerprint"}
          onChange={(v) => setS({ ...s, screen_resolution_mode: v })}
          options={[
            { value: "fingerprint", label: "From fingerprint" },
            { value: "real", label: "Real (host monitor)" },
          ]}
        />
      </SettingsCard>

      <SettingsCard title="Automation API">
        <p className="m-0 mb-2 text-paragraph-xs text-text-soft-400">
          Local HTTP API (axum) for scripting — create/launch/close profiles
          and get a CDP WebSocket URL. Binds <strong>127.0.0.1</strong> only,
          JWT Bearer auth. Changes to enable/port apply after restarting the app.{" "}
          <a
            href="#"
            className="text-primary-base hover:underline"
            onClick={(e) => {
              e.preventDefault();
              openUrl(withUtm("https://docs.proxyshard.com/eng/shardx-launcher-api/binding-and-lifecycle?fallback=true")).catch(() => {});
            }}
          >
            Full API reference →
          </a>
        </p>
        <div className="flex flex-col gap-3">
          <Switch
            label="Enable API server"
            checked={s.api_enabled ?? true}
            onChange={(checked) => setS({ ...s, api_enabled: checked })}
          />
          <Input
            label="Port"
            inputSize="small"
            type="number"
            value={s.api_port ?? 40325}
            onChange={(e) => setS({ ...s, api_port: Number(e.target.value) || 40325 })}
          />
          {api && (
            <>
              <label className="flex flex-col gap-1.5">
                <span className="text-label-xs text-text-sub-600">Base URL</span>
                <CopyField value={api.base_url} />
              </label>
              <label className="flex flex-col gap-1.5">
                <span className="text-label-xs text-text-sub-600">Bearer token</span>
                <CopyField value={api.token} secret />
              </label>
              <div className="mt-1 flex items-center gap-2.5">
                <Button variant="neutral" mode="stroke" size="small" onClick={regenToken}>
                  Regenerate token
                </Button>
                <span className="text-paragraph-xs text-text-soft-400">Invalidates the current token immediately.</span>
              </div>
              <p className="m-0 text-paragraph-xs text-text-soft-400">
                Send it as <code>Authorization: Bearer &lt;token&gt;</code>.
              </p>
            </>
          )}
        </div>
      </SettingsCard>

      <SettingsCard title="MCP server">
        <p className="m-0 mb-2 text-paragraph-xs text-text-soft-400">
          Download the <strong>MCP</strong> server source (lets an AI client drive
          profiles and a CDP browser) into a folder you choose. The app does not run
          it — install its deps and register it with your MCP client per the included
          README. Requires Node.js.
        </p>
        <Button
          variant="neutral"
          mode="stroke"
          size="small"
          leftIcon={<DownloadIcon className="size-4" />}
          onClick={downloadMcp}
          disabled={mcpBusy}
          isLoading={mcpBusy}
        >
          {mcpBusy ? "Downloading…" : "Download MCP server"}
        </Button>
      </SettingsCard>

      <div className="mt-3.5">
        <Button
          variant="primary"
          mode="filled"
          size="small"
      //    leftIcon={<ShardMini />}
          onClick={async () => { await save(); refreshApi(); }}
        >
          Save settings
        </Button>
      </div>
    </section>
  );
}
