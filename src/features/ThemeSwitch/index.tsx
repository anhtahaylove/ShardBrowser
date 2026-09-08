import { SegmentControl, useTheme } from "@proxyshard/shardx-ui-kit";
import { SunIcon, MoonIcon } from "../../shared/icons";

/// Light/dark switch — UI-kit SegmentControl bound to the kit ThemeProvider.
export function ThemeSwitch() {
  const { resolvedTheme, setTheme } = useTheme();
  return (
    <SegmentControl
      size="small"
      className="mb-2 w-full *:flex-1"
      value={resolvedTheme}
      items={[
        { value: "light", label: "Light", icon: <SunIcon className="size-4" /> },
        { value: "dark", label: "Dark", icon: <MoonIcon className="size-4" /> },
      ]}
      onChange={(v) => setTheme(v as "light" | "dark")}
    />
  );
}
