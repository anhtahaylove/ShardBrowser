function aggregatePreservingPrimary(primaryError, secondaryError, context) {
  const primaryCause =
    primaryError instanceof AggregateError && primaryError.cause instanceof Error
      ? primaryError.cause
      : primaryError;
  const primaryErrors = primaryError instanceof AggregateError ? primaryError.errors : [primaryError];
  const secondaryErrors =
    secondaryError instanceof AggregateError ? secondaryError.errors : [secondaryError];
  return new AggregateError(
    [...primaryErrors, ...secondaryErrors],
    `${primaryCause.message}; ${context}: ${secondaryError.message}`,
    { cause: primaryCause },
  );
}

export const LAUNCH_INSTANCE_OWNERSHIP_CAPABILITY = "launch-instance-ownership-v1";

export function requireLaunchInstanceOwnership(health) {
  if (
    !Array.isArray(health?.capabilities) ||
    !health.capabilities.includes(LAUNCH_INSTANCE_OWNERSHIP_CAPABILITY)
  ) {
    throw new Error(
      "ShardX Launcher does not support exact launch-instance ownership; update Launcher before starting a profile",
    );
  }
}

export function redactLaunchInstanceToken(started) {
  if (!started || typeof started !== "object" || Array.isArray(started)) return started;
  const { launch_instance_token: _launchInstanceToken, ...publicResult } = started;
  return publicResult;
}

export async function acquireSafeOpenProfile({
  profileId,
  headless,
  listRunning,
  getLauncherHealth,
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
      ownedLaunchInstanceToken: null,
      cdp: entry.cdp,
    };
  }

  if (typeof getLauncherHealth !== "function") {
    throw new Error(
      "ShardX Launcher health capability check is required before starting a profile",
    );
  }
  requireLaunchInstanceOwnership(await getLauncherHealth());

  const started = await startProfile({ headless: Boolean(headless) });
  const ownedPid = Number.isInteger(started?.pid) && started.pid > 0 ? started.pid : null;
  const ownedLaunchInstanceToken =
    typeof started?.launch_instance_token === "string" && started.launch_instance_token.trim()
      ? started.launch_instance_token
      : null;
  if (ownedPid === null) {
    throw new Error(`profile ${profileId} did not start with a valid PID`);
  }
  if (ownedLaunchInstanceToken === null) {
    throw new Error(`profile ${profileId} did not start with a launch-instance token`);
  }
  if (!started?.cdp?.http_url) {
    const startError = new Error(`profile ${profileId} did not start with CDP`);
    if (cleanupStartedProfile) {
      try {
        await cleanupStartedProfile(ownedPid, ownedLaunchInstanceToken);
      } catch (cleanupError) {
        throw aggregatePreservingPrimary(startError, cleanupError, "startup cleanup failed");
      }
    }
    throw startError;
  }
  return {
    wasRunning: false,
    startedByHelper: true,
    ownedPid,
    ownedLaunchInstanceToken,
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
  ownedLaunchInstanceToken,
  keepRunning,
  completed,
  stopStartedProfile,
  getRunningProfile,
}) {
  const restoreRequested = Boolean(startedByHelper) && (!keepRunning || !completed);
  if (restoreRequested) {
    await stopStartedProfile(ownedPid, ownedLaunchInstanceToken);
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
  stopStartedProfile,
  getRunningProfile,
  keepRunning,
}) {
  const session = await acquire();
  const wasRunning = session.wasRunning;
  let result;
  let operationError = null;

  try {
    result = await open(session);
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
      ownedLaunchInstanceToken: session.ownedLaunchInstanceToken,
      keepRunning,
      completed: result !== undefined,
      stopStartedProfile,
      getRunningProfile,
    });
  } catch (error) {
    restorationError = error;
  }

  if (operationError && restorationError) {
    throw aggregatePreservingPrimary(operationError, restorationError, "restoration failed");
  }
  if (operationError) throw operationError;
  if (restorationError) throw restorationError;

  return { result, selfHealed: null, lifecycle };
}
