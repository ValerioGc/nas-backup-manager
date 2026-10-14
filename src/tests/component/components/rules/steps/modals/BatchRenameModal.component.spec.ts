import { expect, it } from 'vitest';
import { mount } from '@vue/test-utils';
import BatchRenameModal from '@/components/rules/steps/modals/BatchRenameModal.vue';

it('previews and saves the configured batch rename', async () => {
  const wrapper = mount(BatchRenameModal, { props: { initialValue: null } });

  await wrapper.get('#rename-filename').setValue('report.csv');
  await wrapper.get('#rename-prefix').setValue('2026');
  await wrapper.get('#rename-separator').setValue('_');
  await wrapper.get('#rename-position').setValue('suffix');

  expect(wrapper.find('.rename_preview code').text()).toBe('report_2026.csv');

  await wrapper.get('form').trigger('submit');
  expect(wrapper.emitted('save')?.[0]?.[0]).toEqual({
    filename: 'report.csv',
    separator: '_',
    prefix: '2026',
    position: 'suffix',
  });
});
