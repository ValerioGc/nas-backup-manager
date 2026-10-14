import { beforeEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import NetworkDiskPicker from '@/components/cloud/NetworkDiskPicker.vue';
import { messages, setLocale } from '@/i18n';
import { requestNetworkDisks } from '@/services/nasShell';
import { networkShare } from '../../helpers/folders';

vi.mock('@/services/nasShell', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/services/nasShell')>()),
  requestNetworkDisks: vi.fn(),
}));

beforeEach(() => {
  setLocale('en');
  vi.mocked(requestNetworkDisks).mockReset();
});

it('points to the disk manager when no network disk is configured', async () => {
  vi.mocked(requestNetworkDisks).mockResolvedValue({ disks: [] });
  const wrapper = mount(NetworkDiskPicker, { props: { modelValue: null } });
  await flushPromises();

  expect(wrapper.find('input').exists()).toBe(false);
  expect(wrapper.get('[data-testid="network_disk_notice"]').text()).toContain(
    messages.en.networkDisks.noneTitle,
  );
});

it('says when the network disks cannot be read and tries again', async () => {
  vi.mocked(requestNetworkDisks)
    .mockResolvedValueOnce({ disks: [], error: 'Launcher unavailable' })
    .mockResolvedValueOnce({ disks: [networkShare] });
  const wrapper = mount(NetworkDiskPicker, { props: { modelValue: '/mnt/office' } });
  await flushPromises();

  expect(wrapper.get('[role="alert"]').text()).toContain(messages.en.networkDisks.loadError);
  expect(wrapper.find('[data-testid="network_disk_notice"]').exists()).toBe(false);
  await wrapper.get('[role="alert"] button').trigger('click');
  await flushPromises();

  expect(requestNetworkDisks).toHaveBeenCalledTimes(2);
  expect(wrapper.get('input[name="network-disk"]').element).toHaveProperty('checked', true);
});
