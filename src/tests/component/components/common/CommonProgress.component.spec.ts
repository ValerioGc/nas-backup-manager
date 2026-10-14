import { expect, it } from 'vitest';
import { mount } from '@vue/test-utils';
import CommonProgress from '@/components/common/CommonProgress.vue';

it('marks the steps before the current one as completed', () => {
  const wrapper = mount(CommonProgress, { props: { totalSteps: 4, currentStep: 3 } });
  const progress = wrapper.get('progress');
  expect(progress.attributes('max')).toBe('3');
  expect(progress.attributes('value')).toBe('2');
  expect(progress.attributes('aria-valuetext')).toBe('3 / 4');
  const steps = wrapper.findAll('li');
  expect(steps.map((step) => step.text())).toEqual(['1', '2', '3', '4']);
  expect(steps.map((step) => step.classes('completed'))).toEqual([true, true, false, false]);
  expect(steps[2]!.classes('current')).toBe(true);
});

it('keeps the step count and the current step inside their bounds', () => {
  const wrapper = mount(CommonProgress, { props: { totalSteps: 3.7, currentStep: 9 } });
  const progress = wrapper.get('progress');
  expect(progress.attributes('aria-valuetext')).toBe('3 / 3');
  expect(progress.attributes('value')).toBe(progress.attributes('max'));
});

it('shows a single step without a bar', async () => {
  const wrapper = mount(CommonProgress, { props: { totalSteps: 0, currentStep: -2 } });
  expect(wrapper.find('progress').exists()).toBe(false);
  expect(wrapper.findAll('li').map((step) => step.text())).toEqual(['1']);
  expect(wrapper.get('ol').classes('single')).toBe(true);
  await wrapper.setProps({ totalSteps: 2, currentStep: 1 });
  expect(wrapper.get('progress').attributes('value')).toBe('0');
});
