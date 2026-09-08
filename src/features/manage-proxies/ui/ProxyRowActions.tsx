import { Button } from "@proxyshard/shardx-ui-kit";
import {
  InfoIcon,
  RefreshIcon,
  EditIcon,
  DeleteIcon,
} from "../../../shared/icons";
import { useProxy, type ProxyEntry } from "../../../entities/proxy";

export function ProxyRowActions({ proxy }: { proxy: ProxyEntry }) {
  const busy = useProxy((s) => !!s.proxyTesting[proxy.id]);
  const testProxy = useProxy((s) => s.testProxy);
  const removeProxy = useProxy((s) => s.removeProxy);
  const setEditing = useProxy((s) => s.setEditing);
  const setInfoFor = useProxy((s) => s.setInfoFor);
  const isInfoOpen = useProxy((s) => s.infoFor?.proxy.id === proxy.id);

  return (
    <div className="flex justify-end gap-1">
      <Button
        variant="neutral"
        mode="stroke"
        size="xsmall"
        onMouseDown={(e) => e.stopPropagation()}
        onClick={(e) =>
          isInfoOpen
            ? setInfoFor(null)
            : setInfoFor({ proxy, anchor: { x: e.clientX, y: e.clientY } })
        }
        title="Details + history"
        leftIcon={<InfoIcon />}
      >
      </Button>
      <Button variant="neutral" mode="stroke" size="xsmall"  onlyIcon onClick={() => testProxy(proxy)} disabled={busy} title="Test TCP + UDP + geo"
        leftIcon={<RefreshIcon />}
      >
      </Button>
      <Button variant="neutral" mode="stroke" size="xsmall"  onlyIcon onClick={() => setEditing(proxy)} title="Edit"
        leftIcon={<EditIcon />}
      >
      </Button>
      <Button variant="error" mode='filled' size="xsmall"  onlyIcon onClick={() => removeProxy(proxy.id)} title="Delete"
        leftIcon={<DeleteIcon />}
      >
      </Button>
    </div>
  );
}
