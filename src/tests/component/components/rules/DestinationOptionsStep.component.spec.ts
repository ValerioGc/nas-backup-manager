import { expect, it } from 'vitest';
import { mount } from '@vue/test-utils';
import DestinationOptionsStep from '@/components/rules/steps/DestinationOptionsStep.vue';
import { messages } from '@/i18n';

it('offers duplicate handling and backup verification controls', async () => {
  const wrapper = mount(DestinationOptionsStep, {
    props: {
      title: 'Destination options',
      duplicateResolution: null,
      verifyArchivedFiles: false,
    },
  });

  expect(wrapper.get('h3').text()).toBe('Destination options');
  expect(wrapper.findAll('select option').map((option) => option.attributes('value'))).toEqual([
    '',
    'overwrite',
    'ignore',
    'copy',
  ]);
  expect(wrapper.text()).toContain(messages.it.destinationOptions.verifyArchivedFiles);

  await wrapper.get('select').setValue('copy');
  await wrapper.get('input[type="checkbox"]').setValue(true);

  expect(wrapper.emitted('update:duplicateResolution')?.at(-1)).toEqual(['copy']);
  expect(wrapper.emitted('update:verifyArchivedFiles')?.at(-1)).toEqual([true]);
});

it('chooses between a mirror and copies, except for a rule that works in its origin', async () => {
  const wrapper = mount(DestinationOptionsStep, {
    props: {
      title: 'Destination options',
      duplicateResolution: 'overwrite',
      verifyArchivedFiles: false,
      syncMode: 'mirror',
    },
  });
  expect(wrapper.get('[data-testid="sync_mirror"]').element).toHaveProperty('checked', true);
  expect(wrapper.text()).toContain(messages.it.destinationOptions.mirrorHint);
  await wrapper.get('[data-testid="sync_copy"]').setValue(true);
  expect(wrapper.emitted('update:syncMode')?.at(-1)).toEqual(['copy']);

  await wrapper.setProps({ inPlace: true });
  expect(wrapper.find('[data-testid="sync_mirror"]').exists()).toBe(false);
});
