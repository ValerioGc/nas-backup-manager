import { expect, it } from 'vitest';
import { mount } from '@vue/test-utils';
import ArchiveModal from '@/components/rules/steps/modals/ArchiveModal.vue';

it('configures an archive with metadata encryption and final verification', async () => {
  const wrapper = mount(ArchiveModal);

  expect(wrapper.find('#archive-name').exists()).toBe(true);
  expect(wrapper.findAll('input[type="checkbox"]')).toHaveLength(2);

  await wrapper.get('#archive-name').setValue('daily');
  await wrapper.get('#operation-password').setValue('secret');
  await wrapper.get('#operation-password-confirmation').setValue('secret');
  await wrapper.findAll('input[type="checkbox"]').at(0)!.setValue(true);
  await wrapper.findAll('input[type="checkbox"]').at(1)!.setValue(true);
  await wrapper.get('form').trigger('submit');

  expect(wrapper.emitted('save')?.[0]?.[0]).toEqual({
    archiveName: 'daily',
    password: 'secret',
    encryptFileInfo: true,
    verifyArchive: true,
  });
});
