import { createNasShell } from '@valeriogc/nas-base-app';
import type { NetworkDisk, PickedFile, PowerSchedule } from '@valeriogc/nas-base-app';

/**
 * The bridge of the library to the launcher, with the answers the application reads: what was
 * asked, or why it was not, never a refusal to handle.
 */
const shell = createNasShell();

// The longest timer of the browser: a person takes the time they need to choose a folder.
const NO_TIMEOUT = 2147483647;

function reason(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function openNasApp(app: string, location?: string): void {
  shell.openApp(app, location);
}

/** Opens the launcher's picker on folders, at `directory` when one was already chosen. */
export async function pickNasFolder(
  directory?: string,
): Promise<{ files: PickedFile[]; cancelled: boolean; error?: string }> {
  try {
    return await shell.pickFolder(directory, NO_TIMEOUT);
  } catch (error) {
    return { files: [], cancelled: false, error: reason(error) };
  }
}

/** The network shares the launcher reads from their provider, which it starts when stopped. */
export async function requestNetworkDisks(): Promise<{ disks: NetworkDisk[]; error?: string }> {
  try {
    return { disks: await shell.requestNetworkDisks() };
  } catch (error) {
    return { disks: [], error: reason(error) };
  }
}

/** When the NAS switches itself off and on, as the launcher's power settings plan it. */
export async function requestPowerSchedule(): Promise<{
  schedule: PowerSchedule | null;
  error?: string;
}> {
  try {
    return { schedule: await shell.requestPowerSchedule() };
  } catch (error) {
    return { schedule: null, error: reason(error) };
  }
}
