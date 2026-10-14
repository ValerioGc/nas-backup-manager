import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import {
  openNasApp,
  pickNasFolder,
  requestNetworkDisks,
  requestPowerSchedule,
} from '@/services/nasShell';

let originalParent: PropertyDescriptor | undefined;
let parent: { postMessage: ReturnType<typeof vi.fn> };

function arrive(data: unknown) {
  const event = new MessageEvent('message', { data, origin: window.location.origin });
  Object.defineProperty(event, 'source', { value: parent });
  window.dispatchEvent(event);
}

/** Answers the last request of the application with `answer`. */
function answer(type: string, fields: Record<string, unknown>) {
  const request = parent.postMessage.mock.calls.at(-1)![0];
  expect(request.type).toBe(type);
  arrive({ nas: 1, type: `${type}-result`, requestId: request.requestId, ...fields });
}

beforeEach(() => {
  originalParent = Object.getOwnPropertyDescriptor(window, 'parent');
  parent = { postMessage: vi.fn() };
  Object.defineProperty(window, 'parent', { configurable: true, value: parent });
});

afterEach(() => {
  if (originalParent) {
    Object.defineProperty(window, 'parent', originalParent);
  }
  vi.useRealTimers();
});

it('opens another application through the launcher', () => {
  openNasApp('nas-disk-manager', '?section=network');
  expect(parent.postMessage).toHaveBeenCalledWith(
    { nas: 1, type: 'open', app: 'nas-disk-manager', location: '?section=network' },
    window.location.origin,
  );
});

it('hands back the folder chosen in the picker, or why there is none', async () => {
  const chosen = pickNasFolder('/srv/storage/disk');
  answer('pick-files', { files: [{ name: 'disk', path: '/srv/storage/disk' }], cancelled: false });
  expect(await chosen).toEqual({
    files: [{ name: 'disk', path: '/srv/storage/disk' }],
    cancelled: false,
  });
  expect(parent.postMessage.mock.calls.at(-1)![0]).toMatchObject({
    folder: true,
    directory: '/srv/storage/disk',
  });

  const refused = pickNasFolder();
  answer('pick-files', { files: [], cancelled: false, error: 'No provider' });
  expect(await refused).toEqual({ files: [], cancelled: false, error: 'No provider' });

  expect(await pickNasFolder('relative')).toMatchObject({ files: [], error: expect.any(String) });
});

it('reads the network disks and the power schedule, with an error instead of a refusal', async () => {
  const disks = requestNetworkDisks();
  const disk = { name: 'Share', mount: '/mnt/share', source: '//pc/share', readOnly: false };
  answer('network-disks', { success: true, data: [disk] });
  expect(await disks).toEqual({ disks: [disk] });

  const missing = requestNetworkDisks();
  answer('network-disks', { success: false, error: 'No provider' });
  expect(await missing).toEqual({ disks: [], error: 'No provider' });

  const schedule = {
    enabled: true,
    rules: [{ day: 'everyday', time: '23:00', action: 'shutdown' }],
  };
  const power = requestPowerSchedule();
  answer('power-schedule', { success: true, data: schedule });
  expect(await power).toEqual({ schedule });

  const invalid = requestPowerSchedule();
  answer('power-schedule', { success: true, data: { enabled: true, rules: [{ day: 'someday' }] } });
  expect(await invalid).toMatchObject({ schedule: null, error: expect.any(String) });
});

it('gives up on a launcher that never answers', async () => {
  vi.useFakeTimers();
  const power = requestPowerSchedule();
  await vi.advanceTimersByTimeAsync(10000);
  expect(await power).toMatchObject({ schedule: null, error: expect.any(String) });
});
