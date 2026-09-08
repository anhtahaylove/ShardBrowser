import { useEffect } from "react";
import { Topbar } from "../../shared/ui/Topbar";
import { useFingerprint } from "../../entities/fingerprint";
import { FingerprintImporter } from "../../features/manage-fingerprints";
import { FingerprintLibrary } from "../../widgets/FingerprintLibrary/FingerprintLibrary";
import { FingerprintToolbar } from "../../widgets/FingerprintLibrary/FingerprintToolbar";

export function FingerprintsPage() {
  const init = useFingerprint((s) => s.init);
  const reload = useFingerprint((s) => s.reload);
  const importerOpen = useFingerprint((s) => s.importerOpen);
  const setImporterOpen = useFingerprint((s) => s.setImporterOpen);

  useEffect(() => { init(); }, [init]);

  return (
    <section className="flex flex-col">
      <Topbar crumbs={["Library", "Fingerprints"]} search="" onSearch={() => {}} />
      <div className="mb-3.5 flex items-end justify-between gap-4">
        <h1 className="m-0 text-title-h5 text-text-strong-950">Fingerprint Library</h1>
        <FingerprintToolbar />
      </div>
      <p className="m-0 mb-3.5 text-paragraph-xs text-text-soft-400">
        These FingerprintConfig snapshots populate the <strong>GPU</strong> select in the profile editor.
        Import your own from any working ShardX profile JSON to expand the list.
      </p>
      <FingerprintLibrary />
      {importerOpen && (
        <FingerprintImporter onClose={() => { setImporterOpen(false); reload(); }} />
      )}
    </section>
  );
}
