import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Button, Input, Select, Switch, Textarea } from "@proxyshard/shardx-ui-kit";
import { DownloadIcon } from "../../shared/icons";
import { Topbar } from "../../shared/ui/Topbar";
import { CopyField } from "../../shared/ui/CopyField";
import { toast } from "../../shared/model/toast";
import { withUtm } from "../../shared/lib/utils";
import type { Settings, ApiInfo } from "../../entities/settings";
import { HELPER_KINDS } from "../../entities/settings";
import { settingsGet, settingsSave, apiInfo, apiRegenerateToken, mcpDownload } from "../../entities/settings";
import { DataRootCard } from "../../features/manage-profiles/ui/DataRootCard";
import { TeamCard } from "../../features/manage-team/ui/TeamCard";

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
    helper_enabled: true,
    helper_triggers: [],
    extra_args: "",
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

      <SettingsCard title="Startup">
        <p className="m-0 mb-2 text-paragraph-xs text-text-soft-400">
          Useful when profiles are launched by the automation API: ShardX is
          already running and waiting when the first request arrives.
        </p>
        <Switch
          label="Start ShardX when I sign in"
          checked={s.launch_at_login ?? false}
          onChange={(checked) => setS({ ...s, launch_at_login: checked })}
        />
        <div className="mt-2">
          <Switch
            label="Start hidden in the tray"
            checked={s.start_minimized ?? false}
            onChange={(checked) => setS({ ...s, start_minimized: checked })}
          />
        </div>
      </SettingsCard>

      <SettingsCard title="Team">
        <TeamCard />
      </SettingsCard>

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

      <SettingsCard title="Shard Helper">
        <p className="m-0 mb-2 text-paragraph-xs text-text-soft-400">
          Watches each page for fields a generated identity fits — names, email,
          phone, date of birth — and offers to fill them. It only ever
          <strong> offers</strong>: nothing is typed until you press the button
          on the panel that appears. Values come from the profile's own language,
          and go in through the same human typing the rest of the browser uses.
          <br />
          <strong>Never runs on a synchronised launch.</strong> In a group whatever
          you type in one window is mirrored into the others already, so a helper
          per window would find the same form ten times and offer ten prompts for
          one page.
        </p>
        <div className="flex flex-col gap-3">
          <Switch
            label="Enable Shard Helper"
            checked={s.helper_enabled ?? true}
            onChange={(checked) => setS({ ...s, helper_enabled: checked })}
          />
          {(s.helper_enabled ?? true) && (
            <div>
              <div className="mb-1.5 text-label-xs text-text-sub-600">
                React to
              </div>
              <p className="m-0 mb-2 text-paragraph-xs text-text-soft-400">
                Nothing selected means every kind. Narrow it if the panel appears
                on forms you do not care about — a login page with an email field
                is still a form.
              </p>
              <div className="flex flex-wrap gap-1.5">
                {HELPER_KINDS.map((k) => {
                  const picked = (s.helper_triggers ?? []).includes(k.value);
                  return (
                    <button
                      key={k.value}
                      type="button"
                      onClick={() => {
                        const cur = s.helper_triggers ?? [];
                        setS({
                          ...s,
                          helper_triggers: picked
                            ? cur.filter((x) => x !== k.value)
                            : [...cur, k.value],
                        });
                      }}
                      className={`rounded-6 px-2 py-1 text-paragraph-xs ring-1 ring-inset transition-colors ${
                        picked
                          ? "bg-primary-alpha-10 text-primary-base ring-primary-alpha-24"
                          : "text-text-sub-600 ring-stroke-soft-200 hover:bg-bg-weak-50"
                      }`}
                    >
                      {k.label}
                    </button>
                  );
                })}
              </div>
            </div>
          )}
        </div>
      </SettingsCard>

      <SettingsCard title="Profile camera">
        <p className="m-0 mb-2 text-paragraph-xs text-text-soft-400">
          The profile gets ShardX's camera instead of the machine's, and shows the
          picture or clip you pick from the control left of the browser's app menu.
          <strong> Leave this on.</strong> The profile's fingerprint already names a
          particular camera, so handing a page the host's real one contradicts the
          profile and identifies the machine behind every profile on it.
        </p>
        <Switch
          label="Substitute the camera"
          checked={s.camera_enabled ?? true}
          onChange={(checked) => setS({ ...s, camera_enabled: checked })}
        />
      </SettingsCard>

      <SettingsCard title="Profile data location">
        <DataRootCard />
      </SettingsCard>

      <SettingsCard title="Extra launch arguments">
        <p className="m-0 mb-2 text-paragraph-xs text-text-soft-400">
          Appended to every profile launch, one per line or space-separated.
          They go on <strong>last</strong>, so a switch repeated here is the one the
          engine sees — which is also how you get to undo one of the launcher's own.
          Quote a value with spaces.
          <br />
          Anything that changes what a page can measure belongs in the profile, not
          here: a switch applied to every profile at once makes them all alike, which
          is the opposite of what a profile is for.
        </p>
        <Textarea
          rows={3}
          className="mono"
          value={s.extra_args ?? ""}
          onChange={(e) => setS({ ...s, extra_args: e.target.value })}
          placeholder={"--disable-background-timer-throttling\n--window-size=1280,800"}
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
