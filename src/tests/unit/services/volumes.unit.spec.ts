import { afterEach, expect, it, vi } from 'vitest';
import { fetchVolumeNames, readableFolder } from '@/services/volumes';

afterEach(() => vi.unstubAllGlobals());

const names = [{ id: 'f00d', name: 'Dati' }];

it('shows a folder by the name of its disk', () => {
  expect(readableFolder('/f00d/Photos/2026', names)).toEqual({ disk: 'Dati', path: 'Photos/2026' });
  expect(readableFolder('/f00d', names)).toEqual({ disk: 'Dati', path: '' });
  expect(readableFolder('/unknown/Photos', names)).toEqual({ disk: null, path: 'unknown/Photos' });
  expect(readableFolder('/', [])).toEqual({ disk: null, path: '' });
});

it('reads the names of the disks, and none when the backend cannot tell', async () => {
  vi.stubGlobal(
    'fetch',
    vi
      .fn()
      .mockResolvedValue(
        new Response(JSON.stringify([...names, { id: 1 }, null]), { status: 200 }),
      ),
  );
  await expect(fetchVolumeNames()).resolves.toEqual(names);
  expect(fetch).toHaveBeenCalledWith('api/volumes');

  vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response('{}', { status: 200 })));
  await expect(fetchVolumeNames()).resolves.toEqual([]);
  vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response('', { status: 503 })));
  await expect(fetchVolumeNames()).resolves.toEqual([]);
  vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('offline')));
  await expect(fetchVolumeNames()).resolves.toEqual([]);
});
