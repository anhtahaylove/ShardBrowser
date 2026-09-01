import { TitleBar } from "../widgets/TitleBar/TitleBar";
import { Sidebar } from "../widgets/Sidebar/Sidebar";
import { FirstRunGate } from "../widgets/FirstRunGate/FirstRunGate";
import { ToastHost } from "../widgets/ToastHost/ToastHost";
import { ConfirmHost } from "../widgets/ConfirmHost/ConfirmHost";
import { StarModal } from "../widgets/StarModal/StarModal";
import { BrowsersPage } from "../pages/browsers";
import { ProxiesPage } from "../pages/proxies";
import { ProxyShardPage } from "../pages/proxyshard";
import { FingerprintsPage } from "../pages/fingerprints";
import { SettingsPage } from "../pages/settings";
import { useNav } from "../shared/model/navigation";

export function App() {
  const section = useNav((s) => s.section);

  return (
    <>
      <TitleBar />
      <FirstRunGate>
        <div
          className="grid overflow-hidden bg-bg-weak-50 [grid-template-columns:240px_1fr] [@media(min-width:1700px)]:[grid-template-columns:280px_1fr]"
          style={{ height: "100vh", paddingTop: "var(--titlebar-h)" }}
        >
          <Sidebar />
          <main className="overflow-y-auto px-7 py-6">
            {section === "browsers" && <BrowsersPage />}
            {section === "proxies" && <ProxiesPage />}
            {section === "proxyshard" && <ProxyShardPage />}
            {section === "fingerprints" && <FingerprintsPage />}
            {section === "settings" && <SettingsPage />}
          </main>
          <ToastHost />
          <ConfirmHost />
          <StarModal />
        </div>
      </FirstRunGate>
    </>
  );
}
