import { Metric } from "../../shared/ui/Metric";
import { useProfile, useRunningCount } from "../../entities/profile";

export function BrowsersMetrics() {
  const profileCount = useProfile((s) => s.profiles.length);
  const proxyCount = useProfile((s) => s.proxies.length);
  const fingerprintCount = useProfile((s) => s.fingerprints.length);
  const runningCount = useRunningCount();

  return (
    <div className="grid grid-cols-4 gap-[10px] mb-4">
      <Metric label="Profiles" value={String(profileCount)} accent />
      <Metric label="Running" value={String(runningCount)} pulse={runningCount > 0} />
      <Metric label="Proxies" value={String(proxyCount)} />
      <Metric label="Fingerprints" value={String(fingerprintCount)} />
    </div>
  );
}
