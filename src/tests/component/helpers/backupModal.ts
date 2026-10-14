import { expect } from 'vitest';
import { flushPromises, type VueWrapper } from '@vue/test-utils';
import { messages } from '@/i18n';

export const backupModalProps = {
  title: 'New backup',
  closeLabel: 'Close',
  backLabel: 'Back',
  nextLabel: 'Next',
  saveLabel: 'Save',
  stepLabels: messages.en.stepLabels,
};

function footerButton(wrapper: VueWrapper, label: string) {
  return wrapper.findAll('footer button').find((item) => item.text() === label)!;
}

// Walks the four steps, adds a filter rule and checks it survives leaving and reopening the step.
export async function expectBackupSteps(wrapper: VueWrapper, firstStep: string) {
  expect(wrapper.get('h2').text()).toBe('New backup');
  expect(wrapper.get('h3').text()).toBe(firstStep);
  await footerButton(wrapper, 'Next').trigger('click');
  expect(wrapper.get('h3').text()).toBe(messages.en.stepLabels.filtering);
  await wrapper.findAll('section button').at(-1)!.trigger('click');
  expect(wrapper.findAll('tbody tr')).toHaveLength(1);
  await footerButton(wrapper, 'Next').trigger('click');
  expect(wrapper.get('h3').text()).toBe(messages.en.stepLabels.operations);
  await footerButton(wrapper, 'Back').trigger('click');
  expect(wrapper.findAll('tbody tr')).toHaveLength(1);
  await footerButton(wrapper, 'Next').trigger('click');
  await footerButton(wrapper, 'Next').trigger('click');
  expect(wrapper.get('h3').text()).toBe(messages.en.stepLabels.destination);
  expect(footerButton(wrapper, 'Save').exists()).toBe(true);
  await wrapper.get('[aria-label="Close"]').trigger('click');
  await flushPromises();
  expect(wrapper.emitted('close')).toHaveLength(1);
}
