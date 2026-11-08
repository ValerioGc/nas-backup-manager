import { createNasShell } from '@valeriogc/nas-base-app';
import type { PickedFile, PowerSchedule } from '@valeriogc/nas-base-app';
import { errorMessage } from '@/services/errors';

/** Launcher bridge. The wrappers below resolve with the result or the error and never reject. */
const shell = createNasShell();

// Largest browser timer delay, since choosing a folder can take as long as the user needs.
const NO_TIMEOUT = 2147483647;

export function openNasApp(app: string, location?: string): void {
  shell.openApp(app, location);
}

/** Opens the launcher folder picker, starting at `directory` when one was already chosen. */
export async function pickNasFolder(
  directory?: string,
): Promise<{ files: PickedFile[]; cancelled: boolean; error?: string }> {
  try {
    return await shell.pickFolder(directory, NO_TIMEOUT);
  } catch (error) {
    return { files: [], cancelled: false, error: errorMessage(error, String(error)) };
  }
}

/** Power schedule of the NAS from the launcher power settings. */
export async function requestPowerSchedule(): Promise<{
  schedule: PowerSchedule | null;
  error?: string;
}> {
  try {
    return { schedule: await shell.requestPowerSchedule() };
  } catch (error) {
    return { schedule: null, error: errorMessage(error, String(error)) };
  }
}
