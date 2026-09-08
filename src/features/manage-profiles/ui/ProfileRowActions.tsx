import { Button } from "@proxyshard/shardx-ui-kit";
import {
  PinIconApp,
  EditIcon,
  CopyIcon,
  DeleteIcon,
  MoreIcon,
  PlayIcon,
  StopIcon,
} from "../../../shared/icons";
import { useProfile, type ProfileMeta } from "../../../entities/profile";

export function ProfileRowActions({ profile, onMore }: {
  profile: ProfileMeta;
  onMore: (e: React.MouseEvent) => void;
}) {
  const p = profile;
  const isRunning = useProfile((s) => !!s.running[p.id]);
  const isStarting = useProfile((s) => s.startBusy.has(p.id));
  const startStop = useProfile((s) => s.startStop);
  const togglePin = useProfile((s) => s.togglePin);
  const cloneProfile = useProfile((s) => s.cloneProfile);
  const remove = useProfile((s) => s.remove);
  const expand = useProfile((s) => s.expand);

  return (
    <div className="flex justify-end gap-1">
      <Button
        variant={isRunning ? "error" : "primary"}
        mode="lighter"
        size="xsmall"
        fullRadius
        className="min-w-[84px]"
        leftIcon={
          isRunning
            ? <StopIcon className="size-3.5" />
            : <span className={isStarting ? "spin-icon inline-grid place-items-center" : "inline-grid place-items-center"}><PlayIcon className="size-3.5" /></span>
        }
        onClick={() => startStop(p)}
        disabled={!isRunning && isStarting}
        title={!isRunning && isStarting ? "Starting (UDP probe + geo + spawn)…" : undefined}
      >
        {isRunning ? "Stop" : isStarting ? "Starting…" : "Start"}
      </Button>
      <Button
        variant={p.pinned ? "primary" : "neutral"}
        mode={p.pinned ? "lighter" : "stroke"}
        size="xsmall"
        onlyIcon
        onClick={() => togglePin(p)}
        title={p.pinned ? "Unpin" : "Pin to top"}
        leftIcon={<PinIconApp className="size-4" />}
      >
      
      </Button>
      <Button variant="neutral" mode="stroke" size="xsmall" onlyIcon onClick={() => expand(p.id)} title="Edit"
        leftIcon={<EditIcon className="size-4" />}
      >
      </Button>
      <Button variant="neutral" mode="stroke" size="xsmall" onlyIcon onClick={() => cloneProfile(p.id)} title="Clone"
        leftIcon={<CopyIcon className="size-4" />}
      >
      </Button>
      <Button variant="error" mode='filled' size="xsmall" onlyIcon onClick={() => remove(p.id)} title="Delete"
        leftIcon={<DeleteIcon className="size-4" />}
      >
      </Button>
      <Button
        variant="neutral"
        mode="stroke"
        size="xsmall"
        onlyIcon
        onClick={onMore}
        title="More actions"
        leftIcon={<MoreIcon className="size-4" />}
      >
      </Button>
    </div>
  );
}
