import { flushPromises, type VueWrapper } from '@vue/test-utils';
import { vi } from 'vitest';
import { pickNasFolder, requestNetworkDisks } from '@/services/nasShell';

export const networkShare = {
  name: 'office',
  mount: '/mnt/office',
  source: '//nas/office',
  readOnly: false,
};

// The launcher's pickers, mocked by the spec: each folder picked is a new one, one share is mounted.
export function answerPickers() {
  let picked = 0;
  vi.mocked(pickNasFolder).mockImplementation(async () => {
    picked += 1;
    return { files: [{ name: `Folder ${picked}`, path: `/Folder ${picked}` }], cancelled: false };
  });
  vi.mocked(requestNetworkDisks).mockResolvedValue({ disks: [networkShare] });
}

// Names the rule and chooses every folder the first step of a rule asks for.
export async function chooseFolders(wrapper: VueWrapper, name = 'Rule') {
  await flushPromises();
  const field = wrapper.find('[data-testid="rule_name"]');
  if (field.exists()) {
    await field.setValue(name);
  }
  for (const button of wrapper.findAll('.folder_card button')) {
    await button.trigger('click');
    await flushPromises();
  }
  const share = wrapper.find('input[name="network-disk"]');
  if (share.exists()) {
    await share.setValue(true);
  }
}
