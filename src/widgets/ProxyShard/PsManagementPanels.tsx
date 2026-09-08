import { usePsConnected } from "../../entities/proxyshard";
import { PsResidentialCard, PsOrdersCard, PsBuyCard } from "../../features/proxyshard";

export function PsManagementPanels() {
  const connected = usePsConnected();

  if (!connected) {
    return (
      <div className="rounded-lg bg-bg-white-0 p-[18px] ring-1 ring-inset ring-stroke-soft-200">
        <p className="m-0 text-paragraph-xs text-text-soft-400">
          Add a valid API key above to view traffic, manage orders, and buy proxies.
        </p>
      </div>
    );
  }

  return (
    <>
      <PsResidentialCard />
      <PsOrdersCard />
      <PsBuyCard />
    </>
  );
}
