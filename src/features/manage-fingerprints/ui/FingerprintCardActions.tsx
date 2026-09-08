import { Button } from "@proxyshard/shardx-ui-kit";
import Badge from "../../../shared/ui/Badge";
import { useFingerprint, type FingerprintEntry } from "../../../entities/fingerprint";

export function FingerprintCardActions({ entry }: { entry: FingerprintEntry }) {
  const useTemplate = useFingerprint((s) => s.useTemplate);
  const remove = useFingerprint((s) => s.remove);

  return (
    <>
      <Button variant="neutral" mode="stroke" size="2xsmall" onClick={() => useTemplate(entry.id)}>
        Use →
      </Button>
      {entry.builtin ? (
        <Badge color="gray" variant="lighter" size="small" className="ml-auto">built-in</Badge>
      ) : (
        <Button variant="error" mode="stroke" size="2xsmall" onClick={() => remove(entry.id)} title="Remove">
          ✕
        </Button>
      )}
    </>
  );
}
