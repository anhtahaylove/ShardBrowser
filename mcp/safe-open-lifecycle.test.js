import assert from "node:assert/strict";
import test from "node:test";

import {
  acquireSafeOpenProfile,
  finalizeSafeOpenLifecycle,
  navigateActivePage,
  runSafeOpenLifecycle,
} from "./safe-open-lifecycle.js";

test("safe_open_url reuses an already-running profile without taking ownership", async () => {
  let starts = 0;
  const session = await acquireSafeOpenProfile({
    profileId: "profile-1",
    headless: false,
    listRunning: async () => [{ profile_id: "profile-1", cdp: { http_url: "http://127.0.0.1:1" } }],
    startProfile: async () => {
      starts += 1;
      return { cdp: { http_url: "http://127.0.0.1:2" } };
    },
  });

  assert.equal(starts, 0);
  assert.deepEqual(session, {
    wasRunning: true,
    startedByHelper: false,
    ownedPid: null,
    cdp: { http_url: "http://127.0.0.1:1" },
  });
});

test("safe_open_url records ownership only when its start call succeeds", async () => {
  let starts = 0;
  const session = await acquireSafeOpenProfile({
    profileId: "profile-1",
    headless: true,
    listRunning: async () => [],
    startProfile: async ({ headless }) => {
      starts += 1;
      assert.equal(headless, true);
      return { pid: 111, cdp: { http_url: "http://127.0.0.1:2" } };
    },
  });

  assert.equal(starts, 1);
  assert.deepEqual(session, {
    wasRunning: false,
    startedByHelper: true,
    ownedPid: 111,
    cdp: { http_url: "http://127.0.0.1:2" },
  });
});

test("safe_open_url refuses to relaunch a running profile without CDP", async () => {
  await assert.rejects(
    acquireSafeOpenProfile({
      profileId: "profile-1",
      headless: false,
      listRunning: async () => [{ profile_id: "profile-1", cdp: null }],
      startProfile: async () => assert.fail("must not relaunch an existing process"),
    }),
    /running without CDP/,
  );
});

test("safe_open_url cleans up a profile it started when CDP never becomes ready", async () => {
  let cleanups = 0;
  await assert.rejects(
    acquireSafeOpenProfile({
      profileId: "profile-1",
      headless: true,
      listRunning: async () => [],
      startProfile: async () => ({ pid: 111, cdp: null }),
      cleanupStartedProfile: async (ownedPid) => {
        assert.equal(ownedPid, 111);
        cleanups += 1;
      },
    }),
    /did not start with CDP/,
  );
  assert.equal(cleanups, 1);
});

test("safe_open_url foregrounds and registers the navigated page for follow-up tools", async () => {
  const events = [];
  const response = { status: 200 };
  const page = {
    goto: async (url, options) => {
      events.push(["goto", url, options]);
      return response;
    },
    bringToFront: async () => events.push(["bringToFront"]),
  };
  const activePages = new Map();

  const actual = await navigateActivePage({
    page,
    profileId: "profile-1",
    targetUrl: "https://example.com/",
    activePages,
  });

  assert.equal(actual, response);
  assert.equal(activePages.get("profile-1"), page);
  assert.deepEqual(events, [
    ["goto", "https://example.com/", { waitUntil: "domcontentloaded", timeout: 60000 }],
    ["bringToFront"],
  ]);
});

test("safe_open_url reports a foreground failure instead of claiming the tab is active", async () => {
  const page = {
    goto: async () => ({ status: 200 }),
    bringToFront: async () => {
      throw new Error("foreground failed");
    },
  };
  const activePages = new Map();

  await assert.rejects(
    navigateActivePage({ page, profileId: "profile-1", targetUrl: "https://example.com/", activePages }),
    /foreground failed/,
  );
  assert.equal(activePages.has("profile-1"), false);
});

test("safe_open_url restores and verifies a profile it started by default", async () => {
  let stops = 0;
  const lifecycle = await finalizeSafeOpenLifecycle({
    wasRunning: false,
    startedByHelper: true,
    ownedPid: 111,
    keepRunning: false,
    completed: true,
    stopStartedProfile: async (ownedPid) => {
      assert.equal(ownedPid, 111);
      stops += 1;
    },
    getRunningProfile: async () => null,
  });

  assert.equal(stops, 1);
  assert.deepEqual(lifecycle, {
    was_running: false,
    started_by_helper: true,
    keep_running_requested: false,
    running_after: false,
    stopped_by_helper: true,
    restoration_confirmed: true,
  });
});

test("safe_open_url keeps an owned profile running only after a successful keep_running request", async () => {
  let stops = 0;
  const lifecycle = await finalizeSafeOpenLifecycle({
    wasRunning: false,
    startedByHelper: true,
    ownedPid: 111,
    keepRunning: true,
    completed: true,
    stopStartedProfile: async () => {
      stops += 1;
    },
    getRunningProfile: async () => ({ profile_id: "profile-1", pid: 111 }),
  });

  assert.equal(stops, 0);
  assert.deepEqual(lifecycle, {
    was_running: false,
    started_by_helper: true,
    keep_running_requested: true,
    running_after: true,
    stopped_by_helper: false,
    restoration_confirmed: null,
  });
});

test("safe_open_url never stops a profile it did not start", async () => {
  let stops = 0;
  const lifecycle = await finalizeSafeOpenLifecycle({
    wasRunning: true,
    startedByHelper: false,
    ownedPid: null,
    keepRunning: false,
    completed: true,
    stopStartedProfile: async () => {
      stops += 1;
    },
    getRunningProfile: async () => ({ profile_id: "profile-1", pid: 222 }),
  });

  assert.equal(stops, 0);
  assert.equal(lifecycle.running_after, true);
  assert.equal(lifecycle.stopped_by_helper, false);
  assert.equal(lifecycle.restoration_confirmed, null);
});

test("safe_open_url cleans up an owned profile when navigation fails even with keep_running", async () => {
  let stops = 0;
  let readbacks = 0;
  const lifecycle = await finalizeSafeOpenLifecycle({
    wasRunning: false,
    startedByHelper: true,
    ownedPid: 111,
    keepRunning: true,
    completed: false,
    stopStartedProfile: async () => {
      stops += 1;
    },
    getRunningProfile: async () => {
      readbacks += 1;
      return null;
    },
  });

  assert.equal(stops, 1);
  assert.equal(readbacks, 0);
  assert.equal(lifecycle, null);
});

test("safe_open_url fails closed when restoration readback still reports the profile running", async () => {
  await assert.rejects(
    finalizeSafeOpenLifecycle({
      wasRunning: false,
      startedByHelper: true,
      ownedPid: 111,
      keepRunning: false,
      completed: true,
      stopStartedProfile: async () => {},
      getRunningProfile: async () => ({ profile_id: "profile-1", pid: 111 }),
    }),
    /still running after restoration/,
  );
});

test("safe_open_url fails closed instead of stopping a replacement PID", async () => {
  const stopRequests = [];
  await assert.rejects(
    finalizeSafeOpenLifecycle({
      wasRunning: false,
      startedByHelper: true,
      ownedPid: 111,
      keepRunning: false,
      completed: true,
      stopStartedProfile: async (ownedPid) => {
        stopRequests.push(ownedPid);
        throw new Error("profile process changed: expected pid 111, running pid 222");
      },
      getRunningProfile: async () => ({ profile_id: "profile-1", pid: 222 }),
    }),
    /expected pid 111, running pid 222/,
  );

  assert.deepEqual(stopRequests, [111]);
});

test("safe_open_url never retries a foreground failure through stale-process recovery", async () => {
  let staleCleanups = 0;
  const stoppedPids = [];
  const page = {
    goto: async () => ({ status: 200 }),
    bringToFront: async () => {
      throw new Error("foreground failed");
    },
  };

  await assert.rejects(
    runSafeOpenLifecycle({
      acquire: async () => ({
        wasRunning: false,
        startedByHelper: true,
        ownedPid: 111,
        cdp: { http_url: "http://127.0.0.1:1" },
      }),
      open: async () =>
        navigateActivePage({
          page,
          profileId: "profile-1",
          targetUrl: "https://example.com/",
          activePages: new Map(),
        }),
      cleanupStaleProfileProcesses: async () => {
        staleCleanups += 1;
        return { stale: [{ pid: 111 }], killed_pids: [111], errors: [] };
      },
      stopStartedProfile: async (ownedPid) => stoppedPids.push(ownedPid),
      getRunningProfile: async () => null,
      delay: async () => {},
      keepRunning: false,
    }),
    /foreground failed/,
  );

  assert.equal(staleCleanups, 0);
  assert.deepEqual(stoppedPids, [111]);
});

test("safe_open_url preserves both the operation error and restoration error", async () => {
  await assert.rejects(
    runSafeOpenLifecycle({
      acquire: async () => ({
        wasRunning: false,
        startedByHelper: true,
        ownedPid: 111,
        cdp: { http_url: "http://127.0.0.1:1" },
      }),
      open: async () => {
        throw new Error("operation failed");
      },
      cleanupStaleProfileProcesses: async () => ({ stale: [], killed_pids: [], errors: [] }),
      stopStartedProfile: async () => {
        throw new Error("restoration failed");
      },
      getRunningProfile: async () => ({ profile_id: "profile-1", pid: 111 }),
      delay: async () => {},
      keepRunning: false,
    }),
    (error) => {
      assert.equal(error instanceof AggregateError, true);
      assert.match(error.message, /operation failed/);
      assert.deepEqual(error.errors.map((item) => item.message), ["operation failed", "restoration failed"]);
      return true;
    },
  );
});
