export function safeOpenLifecycle({ wasRunning, keepRunning }) {
  const wasRunningValue = Boolean(wasRunning);
  const keepRunningValue = Boolean(keepRunning);
  const stoppedByHelper = !wasRunningValue && !keepRunningValue;

  return {
    was_running: wasRunningValue,
    keep_running_requested: keepRunningValue,
    running_after: !stoppedByHelper,
    stopped_by_helper: stoppedByHelper,
  };
}
