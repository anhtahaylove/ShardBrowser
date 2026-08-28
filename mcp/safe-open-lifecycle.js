export async function acquireSafeOpenProfile({
  profileId,
  headless,
  listRunning,
  startProfile,
  cleanupStartedProfile,
}) {
  const running = await listRunning();
  const entry = running.find((item) => item.profile_id === profileId);
  if (entry) {
    if (!entry.cdp?.http_url) {
      throw new Error(`profile ${profileId} is running without CDP`);
    }
    return {
      wasRunning: true,
      startedByHelper: false,
      ownedPid: null,
      cdp: entry.cdp,
    };
  }

  const started = await startProfile({ headless: Boolean(headless) });
  const ownedPid = Number.isInteger(started?.pid) && started.pid > 0 ? started.pid : null;
  if (!started?.cdp?.http_url) {
    if (ownedPid !== null) await cleanupStartedProfile?.(ownedPid);
    throw new Error(`profile ${profileId} did not start with CDP`);
  }
  if (ownedPid === null) {
    throw new Error(`profile ${profileId} did not start with a valid PID`);
  }
  return {
    wasRunning: false,
    startedByHelper: true,
    ownedPid,
    cdp: started.cdp,
  };
}

export class SafeOpenForegroundError extends Error {
  constructor(cause) {
    super(cause?.message || "failed to foreground the active page", { cause });
    this.name = "SafeOpenForegroundError";
  }
}

export async function navigateActivePage({ page, profileId, targetUrl, activePages }) {
  const response = await page.goto(targetUrl, {
    waitUntil: "domcontentloaded",
    timeout: 60000,
  });
  try {
    await page.bringToFront();
  } catch (error) {
    throw new SafeOpenForegroundError(error);
  }
  activePages.set(profileId, page);
  return response;
}

export async function finalizeSafeOpenLifecycle({
  wasRunning,
  startedByHelper,
  ownedPid,
  keepRunning,
  completed,
  stopStartedProfile,
  getRunningProfile,
}) {
  const restoreRequested = Boolean(startedByHelper) && (!keepRunning || !completed);
  if (restoreRequested) {
    await stopStartedProfile(ownedPid);
  }
  if (!completed) return null;

  const runningProfile = await getRunningProfile();
  const runningAfter = Boolean(runningProfile);
  const ownedProcessStillRunning = restoreRequested && runningProfile?.pid === ownedPid;
  if (ownedProcessStillRunning) {
    throw new Error(`owned profile process ${ownedPid} is still running after restoration`);
  }

  return {
    was_running: Boolean(wasRunning),
    started_by_helper: Boolean(startedByHelper),
    keep_running_requested: Boolean(keepRunning),
    running_after: runningAfter,
    stopped_by_helper: restoreRequested && !ownedProcessStillRunning,
    restoration_confirmed: restoreRequested ? !ownedProcessStillRunning : null,
  };
}

export async function runSafeOpenLifecycle({
  acquire,
  open,
  cleanupStaleProfileProcesses,
  stopStartedProfile,
  getRunningProfile,
  delay,
  keepRunning,
}) {
  let session = await acquire();
  const wasRunning = session.wasRunning;
  let result;
  let selfHealed = null;
  let operationError = null;

  try {
    try {
      result = await open(session);
    } catch (error) {
      if (error instanceof SafeOpenForegroundError) throw error;

      const cleanup = await cleanupStaleProfileProcesses();
      if (!cleanup.killed_pids.length) throw error;
      if (session.startedByHelper) {
        await stopStartedProfile(session.ownedPid);
        session = { ...session, startedByHelper: false, ownedPid: null };
      }
      await delay();
      session = await acquire();
      result = await open(session);
      selfHealed = {
        stale_count: cleanup.stale.length,
        killed_count: cleanup.killed_pids.length,
        errors_count: cleanup.errors.length,
      };
    }
  } catch (error) {
    operationError = error;
  }

  let lifecycle = null;
  let restorationError = null;
  try {
    lifecycle = await finalizeSafeOpenLifecycle({
      wasRunning,
      startedByHelper: session.startedByHelper,
      ownedPid: session.ownedPid,
      keepRunning,
      completed: result !== undefined,
      stopStartedProfile,
      getRunningProfile,
    });
  } catch (error) {
    restorationError = error;
  }

  if (operationError && restorationError) {
    throw new AggregateError(
      [operationError, restorationError],
      `${operationError.message}; restoration failed: ${restorationError.message}`,
      { cause: operationError },
    );
  }
  if (operationError) throw operationError;
  if (restorationError) throw restorationError;

  return { result, selfHealed, lifecycle };
}
