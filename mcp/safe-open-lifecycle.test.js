import assert from "node:assert/strict";
import test from "node:test";

import { safeOpenLifecycle } from "./safe-open-lifecycle.js";

test("safe_open_url restores a profile that it started by default", () => {
  assert.deepEqual(safeOpenLifecycle({ wasRunning: false, keepRunning: false }), {
    was_running: false,
    keep_running_requested: false,
    running_after: false,
    stopped_by_helper: true,
  });
});

test("safe_open_url can keep its active tab available for follow-up tools", () => {
  assert.deepEqual(safeOpenLifecycle({ wasRunning: false, keepRunning: true }), {
    was_running: false,
    keep_running_requested: true,
    running_after: true,
    stopped_by_helper: false,
  });
});

test("safe_open_url never stops a profile that was already running", () => {
  assert.deepEqual(safeOpenLifecycle({ wasRunning: true, keepRunning: false }), {
    was_running: true,
    keep_running_requested: false,
    running_after: true,
    stopped_by_helper: false,
  });
});
