import { Select } from "@proxyshard/shardx-ui-kit";
import type { CSOption } from "../types";

/// Generic-typed dropdown — UI-kit Select with non-string value support.
export function CSSelect<T extends string | number>({
  value, options, onChange, placeholder,title,
}: {
  value: T;
  options: CSOption<T>[];
  onChange: (v: T) => void;
  placeholder?: string;
  title?: string;
}) {
  const byKey = new Map(options.map((o) => [String(o.value), o.value]));
  return (
    <Select
      size="small"
      label={title}
      value={String(value)}
      placeholder={placeholder}
      options={options.map((o) => ({ value: String(o.value), label: o.label }))}
      onChange={(v) => {
        const next = byKey.get(v);
        if (next !== undefined) onChange(next);
      }}
    />
  );
}
