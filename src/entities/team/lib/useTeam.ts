import { create } from "zustand";
import { teamStatus } from "../model/api";
import type { TeamStatus } from "../model/types";

/**
 * Fleet status, fetched once and shared.
 *
 * Every profile row asks whether team actions apply, so this is cached rather
 * than invoked per row. `refresh` is called after enrolment changes it.
 */
type TeamState = {
  status: TeamStatus | null;
  loaded: boolean;
  refresh: () => Promise<void>;
};

export const useTeam = create<TeamState>((set) => ({
  status: null,
  loaded: false,
  refresh: async () => {
    try {
      set({ status: await teamStatus(), loaded: true });
    } catch {
      // No team configured is the normal case, not an error worth a toast.
      set({ status: null, loaded: true });
    }
  },
}));

/** True when this device is enrolled and able to sync profiles. */
export const canSyncProfiles = (s: TeamState) =>
  !!s.status?.is_enrolled && !!s.status?.can_sync;
