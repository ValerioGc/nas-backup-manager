import { expect, it, vi } from 'vitest';
import { useNetworkDisks } from '@/composables/useNetworkDisks';
import { requestNetworkDisks } from '@/services/nasShell';

vi.mock('@/services/nasShell', () => ({ requestNetworkDisks: vi.fn() }));

const share = { name: 'office', mount: '/mnt/office', source: null, readOnly: false };

it('keeps only the answer to the latest request', async () => {
  let answerFirst!: (value: { disks: (typeof share)[]; error?: string }) => void;
  vi.mocked(requestNetworkDisks)
    .mockReturnValueOnce(new Promise((resolve) => (answerFirst = resolve)))
    .mockResolvedValueOnce({ disks: [share] });
  const networkDisks = useNetworkDisks();

  const first = networkDisks.load();
  expect(networkDisks.loading.value).toBe(true);
  await networkDisks.load();
  answerFirst({ disks: [], error: 'Launcher unavailable' });
  await first;

  expect(networkDisks.disks.value).toEqual([share]);
  expect(networkDisks.failed.value).toBe(false);
  expect(networkDisks.loaded.value).toBe(true);
  expect(networkDisks.loading.value).toBe(false);
});
