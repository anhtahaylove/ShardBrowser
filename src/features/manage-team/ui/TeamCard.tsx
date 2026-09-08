import { useEffect, useState } from "react";
import { Button, Input } from "@proxyshard/shardx-ui-kit";
import { CopyField } from "../../../shared/ui/CopyField";
import { toast } from "../../../shared/model/toast";
import {
  teamStatus, teamSetConnection, teamTestConnection,
  teamEnrollDevice, teamCollectCustody, useTeam,
  type TeamStatus,
} from "../../../entities/team";

/** A tenant id is a UUID the operator picks once; typing one by hand is how
 *  two devices end up in different fleets over a mistyped character. */
function newTenantId(): string {
  try {
    if (typeof crypto?.randomUUID === "function") return crypto.randomUUID();
  } catch { /* not a secure context */ }
  const b = new Uint8Array(16);
  crypto.getRandomValues(b);
  b[6] = (b[6] & 0x0f) | 0x40;
  b[8] = (b[8] & 0x3f) | 0x80;
  const h = [...b].map((x) => x.toString(16).padStart(2, "0")).join("");
  return `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}`;
}

/** A default label that says which machine this is without asking. */
function suggestLabel(): string {
  const ua = navigator.userAgent;
  const os = /Windows/i.test(ua) ? "Windows"
    : /Macintosh|Mac OS X/i.test(ua) ? "macOS"
    : /Linux|X11|CrOS/i.test(ua) ? "Linux" : "device";
  return `${os} ${new Date().toISOString().slice(0, 10)}`;
}

/**
 * Team server connection, device enrolment and key custody.
 *
 * The three states this card has to make obvious, because they look alike and
 * behave nothing alike: not connected, connected but not enrolled, and
 * enrolled but unable to receive custody (a device from before the HPKE seed
 * was kept — the server has its public key, the private half is gone).
 */
export function TeamCard() {
  const [st, setSt] = useState<TeamStatus | null>(null);
  const [url, setUrl] = useState("");
  const [token, setToken] = useState("");
  const [tenant, setTenant] = useState("");
  const [label, setLabel] = useState("");
  const [busy, setBusy] = useState<string | null>(null);

  const shareStatus = useTeam((s) => s.refresh);

  const refresh = () => teamStatus().then((s) => {
    setSt(s);
    setUrl(s.server_url);
    setTenant(s.tenant_id);
  }).catch(() => {});
  useEffect(() => { refresh(); }, []);

  const run = async (what: string, fn: () => Promise<void>) => {
    setBusy(what);
    try { await fn(); }
    catch (e) { toast.err(String(e)); }
    finally { setBusy(null); }
  };

  const save = () => run("save", async () => {
    // The token is only sent when it was typed: the field is left blank on
    // load, and sending that blank would clear a working token.
    const next = await teamSetConnection(url, token, tenant);
    setSt(next);
    setToken("");
    void shareStatus();
    toast.ok("Connection saved");
  });

  const test = () => run("test", async () => {
    const id = await teamTestConnection();
    toast.ok(`Server identity: ${id}`);
  });

  const enroll = () => run("enroll", async () => {
    const next = await teamEnrollDevice(label.trim() || suggestLabel());
    setSt(next);
    void shareStatus();
    toast.ok("Device enrolled");
  });

  const collect = () => run("collect", async () => {
    const r = await teamCollectCustody();
    if (r.grants === 0) {
      toast.info("No grants waiting — a custodian device has to issue one");
    } else if (r.failed > 0) {
      toast.err(`${r.opened} of ${r.grants} opened; ${r.failed} could not be opened`);
    } else {
      const gen = r.newest_generation == null ? "" : ` (generation ${r.newest_generation})`;
      toast.ok(`Custody in place: ${r.opened} grant(s) opened${gen}`);
    }
  });

  const connected = !!st && !!st.server_url && st.has_token;

  return (
    <div className="flex flex-col gap-2">
      <p className="m-0 text-paragraph-xs text-text-soft-400">
        Enrol this device with a team server to sync encrypted profiles. The
        server routes ciphertext only — it never sees a key.
      </p>

      <Input
        label="Server URL"
        inputSize="small"
        placeholder="https://team.example.com"
        value={url}
        onChange={(e) => setUrl(e.target.value)}
      />
      <Input
        label="API token"
        inputSize="small"
        type="password"
        placeholder={st?.has_token ? "•••••••• (saved — type to replace)" : "paste the token"}
        value={token}
        onChange={(e) => setToken(e.target.value)}
      />

      <div className="flex items-end gap-2">
        <div className="grow">
          <Input
            label="Tenant ID"
            inputSize="small"
            className="mono"
            placeholder="00000000-0000-0000-0000-000000000000"
            value={tenant}
            onChange={(e) => setTenant(e.target.value)}
          />
        </div>
        <Button
          size="small"
          mode="stroke"
          disabled={!!busy}
          onClick={() => { setTenant(newTenantId()); toast.info("Generated — save to apply"); }}
        >
          Generate
        </Button>
      </div>
      <p className="m-0 text-paragraph-xs text-text-soft-400">
        Generate one for a new fleet; paste the existing one to join a fleet
        that already has devices. Changing the server or tenant clears this
        device's keys, because keys enrolled against one fleet mean nothing to
        another.
      </p>

      <div className="flex gap-2">
        <Button size="small" disabled={!!busy} onClick={save}>
          {busy === "save" ? "Saving…" : "Save connection"}
        </Button>
        <Button size="small" mode="stroke" disabled={!!busy || !connected} onClick={test}>
          {busy === "test" ? "Testing…" : "Test connection"}
        </Button>
      </div>

      {st && (
        <div className="mt-1 flex flex-col gap-2 border-t border-stroke-soft-200 pt-2">
          {!st.is_enrolled ? (
            <>
              <Input
                label="Device label"
                inputSize="small"
                placeholder={suggestLabel()}
                value={label}
                onChange={(e) => setLabel(e.target.value)}
              />
              <div>
                <Button size="small" disabled={!!busy || !connected} onClick={enroll}>
                  {busy === "enroll" ? "Enrolling…" : "Enrol this device"}
                </Button>
              </div>
              {!connected && (
                <p className="m-0 text-paragraph-xs text-text-soft-400">
                  Save a server URL and token first.
                </p>
              )}
            </>
          ) : (
            <>
              <div>
                <span className="text-paragraph-xs text-text-soft-400">Device ID</span>
                <CopyField value={st.device_id} />
              </div>

              {st.can_receive_custody ? (
                <>
                  <div>
                    <Button size="small" disabled={!!busy} onClick={collect}>
                      {busy === "collect" ? "Collecting…" : "Collect key custody"}
                    </Button>
                  </div>
                  <p className="m-0 text-paragraph-xs text-text-soft-400">
                    Picks up the root key grants a custodian has issued to this
                    device and checks they open. The key is never shown, and
                    never leaves this machine.
                  </p>
                </>
              ) : (
                <p className="m-0 text-paragraph-xs text-state-error-base">
                  This device was enrolled before its key material was kept, so
                  grants sealed to it can never be opened. Re-enrol it to take
                  custody.
                </p>
              )}

              {!st.can_sync && (
                <p className="m-0 text-paragraph-xs text-state-error-base">
                  Enrolled before profile sync existed — re-enrol to sync.
                </p>
              )}
            </>
          )}
        </div>
      )}
    </div>
  );
}
