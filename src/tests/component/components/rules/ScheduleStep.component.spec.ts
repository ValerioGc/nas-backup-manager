import { afterEach, expect, it } from 'vitest';
import { mount, type VueWrapper } from '@vue/test-utils';
import ScheduleStep from '@/components/rules/steps/ScheduleStep.vue';
import { offRanges } from '@valeriogc/nas-base-app';
import type { PowerState } from '@/composables/usePowerSchedule';
import { messages, setLocale } from '@/i18n';
import type { RuleSchedule } from '@/types/backup';

afterEach(() => setLocale('it'));

const daily: RuleSchedule = { frequency: 'daily', time: '', weekday: null, day: null, month: null };
const nightly = offRanges({
  enabled: true,
  rules: [
    { day: 'everyday', time: '23:00', action: 'shutdown' },
    { day: 'everyday', time: '07:00', action: 'start' },
  ],
});

// The step with its schedule bound both ways, as the rule view binds it.
function mountStep(
  schedule: RuleSchedule,
  power: PowerState = 'ready',
  offDays: number[] = [],
  manualStart = false,
) {
  setLocale('en');
  const wrapper: VueWrapper = mount(ScheduleStep, {
    props: {
      title: 'Schedule',
      schedule,
      'onUpdate:schedule': (value: RuleSchedule) => wrapper.setProps({ schedule: value }),
      ranges: nightly,
      power,
      manualStart,
      offDays,
    },
  });
  return wrapper;
}

const current = (wrapper: VueWrapper) => (wrapper.props() as { schedule: RuleSchedule }).schedule;
const byTestId = (wrapper: VueWrapper, id: string) => wrapper.find(`[data-testid="${id}"]`);

it('asks for the fields of each frequency and keeps the time', async () => {
  const wrapper = mountStep({ ...daily, time: '02:00' });
  expect(byTestId(wrapper, 'schedule_weekday').exists()).toBe(false);
  expect(byTestId(wrapper, 'schedule_day').exists()).toBe(false);

  await byTestId(wrapper, 'schedule_frequency').setValue('weekly');
  expect(current(wrapper)).toEqual({ ...daily, time: '02:00', frequency: 'weekly', weekday: 1 });
  await byTestId(wrapper, 'schedule_weekday').setValue('5');
  expect(current(wrapper).weekday).toBe(5);
  expect(byTestId(wrapper, 'schedule_weekday').text()).toContain('Friday');

  await byTestId(wrapper, 'schedule_frequency').setValue('monthly');
  expect(current(wrapper)).toEqual({ ...daily, time: '02:00', frequency: 'monthly', day: 1 });
  expect(byTestId(wrapper, 'schedule_day').findAll('option')).toHaveLength(31);
  await byTestId(wrapper, 'schedule_day').setValue('31');
  expect(wrapper.text()).toContain(messages.en.schedule.shortMonth);

  await byTestId(wrapper, 'schedule_frequency').setValue('yearly');
  expect(current(wrapper)).toMatchObject({ frequency: 'yearly', day: 31, month: 1 });
  await byTestId(wrapper, 'schedule_month').setValue('2');
  expect(current(wrapper)).toMatchObject({ day: 29, month: 2 });
  expect(byTestId(wrapper, 'schedule_day').findAll('option')).toHaveLength(29);
  expect(byTestId(wrapper, 'schedule_month').text()).toContain('February');

  await byTestId(wrapper, 'schedule_frequency').setValue('daily');
  expect(current(wrapper)).toEqual({ ...daily, time: '02:00' });
});

it('draws the off hours of every day and marks the chosen time', async () => {
  const wrapper = mountStep(daily);
  expect(wrapper.text()).toContain(messages.en.schedule.chooseTime);
  expect(wrapper.findAll('.power_day')).toHaveLength(7);
  expect(wrapper.findAll('.power_day--checked')).toHaveLength(7);
  expect(wrapper.find('.power_marker').exists()).toBe(false);
  expect(wrapper.get('.power_day').attributes('aria-label')).toBe(
    'Monday: Off 00:00-07:00, 23:00-24:00',
  );

  await byTestId(wrapper, 'schedule_time').setValue('12:00');
  expect(current(wrapper).time).toBe('12:00');
  expect(wrapper.findAll('.power_marker')).toHaveLength(7);
  expect(wrapper.get('.power_marker').attributes('style')).toContain('left: 50%');
  expect(byTestId(wrapper, 'schedule_off').exists()).toBe(false);
});

it('checks only the weekday of a weekly rule and says when the NAS is off then', () => {
  const wrapper = mountStep(
    { ...daily, frequency: 'weekly', weekday: 2, time: '23:30' },
    'ready',
    [2],
  );

  expect(wrapper.findAll('.power_day--checked')).toHaveLength(1);
  expect(wrapper.findAll('.power_marker--off')).toHaveLength(1);
  expect(byTestId(wrapper, 'schedule_off').text()).toBe(
    messages.en.schedule.offAtTime.replace('{time}', '23:30').replace('{days}', 'Tuesday'),
  );
});

it.each<[PowerState, string]>([
  ['loading', messages.en.schedule.powerLoading],
  ['unavailable', messages.en.schedule.powerUnavailable],
  ['alwaysOn', messages.en.schedule.alwaysOn],
])('tells the %s power schedule apart', (power, text) => {
  const wrapper = mountStep(daily, power);

  expect(wrapper.text()).toContain(text);
  expect(byTestId(wrapper, 'power_week').exists()).toBe(false);
});

it('explains a shutdown that only a person can end', () => {
  const wrapper = mountStep(daily, 'ready', [], true);

  expect(wrapper.text()).toContain(messages.en.schedule.manualStart);
});
