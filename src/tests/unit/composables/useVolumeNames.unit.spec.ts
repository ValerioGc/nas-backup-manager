import { afterEach, expect, it, vi } from 'vitest';
import { forgetVolumeNames, useVolumeNames } from '@/composables/useVolumeNames';

afterEach(() => {
  vi.unstubAllGlobals();
  forgetVolumeNames();
});

it('reads the names of the disks once for the whole page', async () => {
  const request = vi
    .fn()
    .mockResolvedValue(new Response(JSON.stringify([{ id: 'f00d', name: 'Dati' }])));
  vi.stubGlobal('fetch', request);
  const first = useVolumeNames();
  await first.loaded;
  const second = useVolumeNames();
  await second.loaded;
  expect(request).toHaveBeenCalledTimes(1);
  expect(second.names.value).toEqual([{ id: 'f00d', name: 'Dati' }]);
});
