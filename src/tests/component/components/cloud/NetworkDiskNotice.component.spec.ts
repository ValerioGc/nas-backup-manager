import { expect, it, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import NetworkDiskNotice from '@/components/cloud/NetworkDiskNotice.vue';
import { messages, setLocale } from '@/i18n';
import { openNasApp } from '@/services/nasShell';

vi.mock('@/services/nasShell', () => ({ openNasApp: vi.fn() }));

it('explains what a cloud backup needs and opens the network disks of the disk manager', async () => {
  setLocale('en');
  const wrapper = mount(NetworkDiskNotice);

  expect(wrapper.text()).toContain(messages.en.networkDisks.noneTitle);
  expect(wrapper.text()).toContain(messages.en.networkDisks.noneDescription);
  await wrapper.get('button').trigger('click');

  expect(openNasApp).toHaveBeenCalledWith('nas-disk-manager', '?kind=network');
});
