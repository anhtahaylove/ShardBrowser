import { useState } from "react";
import { Button, Input, Modal } from "@proxyshard/shardx-ui-kit";
import { usePassphraseStore } from "../../shared/model/passphrase";

/** Global passphrase prompt. Kept deliberately plain: no strength meter, no
 *  "remember this" — there is nowhere safe to remember it. */
export function PassphraseHost() {
  const req = usePassphraseStore((s) => s.req);
  const [a, setA] = useState("");
  const [b, setB] = useState("");

  if (!req) return null;

  const mismatch = req.confirm && b.length > 0 && a !== b;
  const ready = a.length > 0 && (!req.confirm || a === b);

  const done = (value: string | null) => {
    setA("");
    setB("");
    req.resolve(value);
  };

  return (
    <Modal
      open
      onClose={() => done(null)}
      title={req.title}
      maxWidthClassName="max-w-md"
      footer={
        <div className="flex justify-end gap-2">
          <Button size="small" mode="stroke" onClick={() => done(null)}>
            Cancel
          </Button>
          <Button size="small" disabled={!ready} onClick={() => ready && done(a)}>
            Continue
          </Button>
        </div>
      }
    >
      <form
        className="flex flex-col gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          if (ready) done(a);
        }}
      >
        <p className="m-0 text-paragraph-sm text-text-sub-600">{req.message}</p>
        <Input
          autoFocus
          label="Passphrase"
          inputSize="small"
          type="password"
          value={a}
          onChange={(e) => setA(e.target.value)}
        />
        {req.confirm && (
          <Input
            label="Repeat passphrase"
            inputSize="small"
            type="password"
            value={b}
            onChange={(e) => setB(e.target.value)}
          />
        )}
        {mismatch && (
          <p className="m-0 text-paragraph-xs text-state-error-base">
            The two entries don't match.
          </p>
        )}
        {/* Lets Enter submit without a visible duplicate button. */}
        <button type="submit" className="hidden" aria-hidden tabIndex={-1} />
      </form>
    </Modal>
  );
}
