import { flushPromises, type VueWrapper } from '@vue/test-utils';
import { vi } from 'vitest';
import { pickNasFolder } from '@/services/nasShell';

export function answerPickers() {
  let picked = 0;
  vi.mocked(pickNasFolder).mockImplementation(async () => {
    picked += 1;
    return { files: [{ name: `Folder ${picked}`, path: `/Folder ${picked}` }], cancelled: false };
  });
}

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
}
