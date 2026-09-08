import { Switch } from "@proxyshard/shardx-ui-kit";
import type { Settings, StartupStatus } from "../../../entities/settings";

/// Sign-in startup, and whether the OS actually registered it.
///
/// The switch is a request; `startup_status` is the truth. Showing both
/// stops a failed registration from looking like a working one.
export function StartupCard({
  settings,
  onChange,
  status,
  error,
}: {
  settings: Settings;
  onChange: (next: Settings) => void;
  status: StartupStatus | null;
  error: string | null;
}) {
  const launch = settings.launch_at_login ?? false;
  return (
    <div className="flex flex-col gap-3">
      <p className="m-0 text-paragraph-sm text-text-sub-600">
        Start ShardX Launcher when you sign in so its embedded Automation API is ready
        without opening the window. The MCP server remains a lightweight stdio process
        started on demand by Codex or another MCP client; it does not need a
        separate always-on daemon.
      </p>

      <Switch
        label="Start ShardX Launcher when I sign in"
        checked={launch}
        onChange={(checked) => onChange({ ...settings, launch_at_login: checked })}
      />
      <Switch
        label="Start in the system tray"
        checked={settings.start_minimized ?? true}
        disabled={!launch}
        onChange={(checked) => onChange({ ...settings, start_minimized: checked })}
      />

      <div
        role={error ? "alert" : "status"}
        className="flex flex-col gap-0.5 rounded-lg bg-bg-weak-50 px-3 py-2"
      >
        <strong className="text-label-xs text-text-strong-950">
          {status?.registered
            ? "Startup entry registered"
            : error
              ? "Startup status unavailable"
              : "Startup entry not registered"}
        </strong>
        <span className="text-paragraph-xs text-text-sub-600">
          {error ??
            (status?.registered
              ? "Launcher and Automation API will start at desktop sign-in."
              : "Enable this option and save settings to register it for the current user.")}
        </span>
      </div>
    </div>
  );
}
