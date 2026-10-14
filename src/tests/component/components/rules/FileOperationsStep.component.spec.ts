import { expect, it } from 'vitest';
import { mount } from '@vue/test-utils';
import FileOperationsStep from '@/components/rules/steps/FileOperationsStep.vue';

it('selects an operation only after its required settings are saved', async () => {
  const wrapper = mount(FileOperationsStep, {
    props: {
      title: 'File operations',
      operations: [],
      configurations: { batchRename: null, archive: null, encrypt: null },
    },
  });
  const cards = wrapper.findAll('.operation_card');

  expect(wrapper.get('h3').text()).toBe('File operations');
  // Rename and archive: encryption is not offered yet.
  expect(cards).toHaveLength(2);
  expect(cards.every((card) => card.attributes('aria-pressed') === 'false')).toBe(true);
  expect(wrapper.find('.operation_note').exists()).toBe(true);

  await cards[1].trigger('click');
  expect(wrapper.find('#archive-name').exists()).toBe(true);
  expect(wrapper.emitted('update:operations')).toBeUndefined();
  expect(wrapper.get('button[type="submit"]').attributes('disabled')).toBeDefined();

  await wrapper.get('#archive-name').setValue('daily');
  await wrapper.get('#operation-password').setValue('secret');
  await wrapper.get('#operation-password-confirmation').setValue('secret');
  await wrapper.findAll('input[type="checkbox"]').at(1)!.setValue(true);
  await wrapper.get('form').trigger('submit');

  expect(wrapper.emitted('update:operations')?.at(-1)).toEqual([['archive']]);
  expect(wrapper.emitted('update:configurations')?.at(-1)?.[0]).toMatchObject({
    archive: {
      archiveName: 'daily',
      password: 'secret',
      verifyArchive: true,
    },
  });
  expect(wrapper.find('dialog').exists()).toBe(false);
});

it('does not select an operation when its modal is cancelled', async () => {
  const wrapper = mount(FileOperationsStep, {
    props: {
      title: 'File operations',
      operations: [],
      configurations: { batchRename: null, archive: null, encrypt: null },
    },
  });

  await wrapper.findAll('.operation_card')[1]!.trigger('click');
  await wrapper.get('.dialog_actions button[type="button"]').trigger('click');

  expect(wrapper.emitted('update:operations')).toBeUndefined();
  expect(wrapper.emitted('update:configurations')).toBeUndefined();
  expect(wrapper.find('dialog').exists()).toBe(false);
});

it('deselects an existing operation when its reconfiguration is cancelled', async () => {
  const wrapper = mount(FileOperationsStep, {
    props: {
      title: 'File operations',
      operations: ['archive'],
      configurations: {
        batchRename: null,
        archive: {
          archiveName: 'daily',
          password: 'secret',
          encryptFileInfo: false,
          verifyArchive: false,
        },
        encrypt: null,
      },
    },
  });

  await wrapper.findAll('.operation_card')[1]!.trigger('click');
  await wrapper.get('.dialog_actions button[type="button"]').trigger('click');

  expect(wrapper.emitted('update:operations')?.at(-1)).toEqual([[]]);
  expect(wrapper.emitted('update:configurations')?.at(-1)?.[0]).toEqual({
    batchRename: null,
    archive: null,
    encrypt: null,
  });
});

it('asks for an operation when the rule works in its origin, and keeps a saved password', async () => {
  const wrapper = mount(FileOperationsStep, {
    props: {
      title: 'File operations',
      operations: [],
      configurations: {
        batchRename: null,
        archive: {
          archiveName: 'daily',
          password: '',
          encryptFileInfo: false,
          verifyArchive: false,
        },
        encrypt: null,
      },
      required: true,
      keepPassword: true,
    },
  });
  expect(wrapper.get('[role="alert"]').text()).toContain('operazione');

  await wrapper.findAll('.operation_card')[1]!.trigger('click');
  expect(wrapper.get('#operation-password').attributes('placeholder')).toBe(
    'Campo vuoto per mantenere la password salvata',
  );
  expect(wrapper.get('button[type="submit"]').attributes('disabled')).toBeUndefined();
  await wrapper.get('form').trigger('submit');
  expect(wrapper.emitted('update:configurations')?.at(-1)?.[0]).toMatchObject({
    archive: { archiveName: 'daily', password: '' },
  });
});
