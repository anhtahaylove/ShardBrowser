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
      cdp: entry.cdp,
    };
  }

  const started = await startProfile({ headless: Boolean(headless) });
  if (!started?.cdp?.http_url) {
    await cleanupStartedProfile?.();
    throw new Error(`profile ${profileId} did not start with CDP`);
  }
  return {
    wasRunning: false,
    startedByHelper: true,
    cdp: started.cdp,
  };
}

export async function navigateActivePage({ page, profileId, targetUrl, activePages }) {
  const response = await page.goto(targetUrl, {
    waitUntil: "domcontentloaded",
    timeout: 60000,
  });
  await page.bringToFront();
  activePages.set(profileId, page);
  return response;
}

export async function finalizeSafeOpenLifecycle({
  wasRunning,
  startedByHelper,
  keepRunning,
  completed,
  stopStartedProfile,
  isRunning,
}) {
  const restoreRequested = Boolean(startedByHelper) && (!keepRunning || !completed);
  if (restoreRequested) {
    await stopStartedProfile();
  }
  if (!completed) return null;

  const runningAfter = Boolean(await isRunning());
  if (restoreRequested && runningAfter) {
    throw new Error("profile is still running after restoration");
  }

  return {
    was_running: Boolean(wasRunning),
    started_by_helper: Boolean(startedByHelper),
    keep_running_requested: Boolean(keepRunning),
    running_after: runningAfter,
    stopped_by_helper: restoreRequested && !runningAfter,
    restoration_confirmed: restoreRequested ? !runningAfter : null,
  };
}
