import { expect, it } from 'vitest';
import { describeSchedule, monthName, scheduleKind, weekdayName } from '@valeriogc/nas-base-app';
import { messages } from '@/i18n';
import type { RuleSchedule } from '@/types/backup';

const at = (schedule: Partial<RuleSchedule>): RuleSchedule => ({
  frequency: 'daily',
  time: '03:00',
  weekday: null,
  day: null,
  month: null,
  ...schedule,
});

it('says in words when a rule runs', () => {
  const it_ = messages.it.scheduleSummary;
  expect(describeSchedule(at({}), 'it', it_)).toBe('Ogni giorno alle 03:00');
  expect(describeSchedule(at({ frequency: 'weekly', weekday: 1 }), 'it', it_)).toBe(
    'Ogni lunedì alle 03:00',
  );
  expect(describeSchedule(at({ frequency: 'monthly', day: 15 }), 'it', it_)).toBe(
    'Ogni mese il giorno 15 alle 03:00',
  );
  expect(describeSchedule(at({ frequency: 'yearly', day: 24, month: 12 }), 'it', it_)).toBe(
    'Ogni anno il 24 dicembre alle 03:00',
  );
  expect(
    describeSchedule(
      at({ frequency: 'yearly', day: 1, month: 2 }),
      'en',
      messages.en.scheduleSummary,
    ),
  ).toBe('Every year on February 1 at 03:00');
});

it('names weekdays and months in the language of the page', () => {
  expect(weekdayName('en', 7)).toBe('Sunday');
  expect(weekdayName('it', 3, 'short')).toBe('mer');
  expect(monthName('it', 1)).toBe('gennaio');
});

it('gives the kind of a schedule and its time for the list', () => {
  expect(scheduleKind(at({ frequency: 'weekly', weekday: 1 }), messages.it.frequencyNames)).toBe(
    'Settimanale · 03:00',
  );
  expect(scheduleKind(at({}), messages.en.frequencyNames)).toBe('Daily · 03:00');
});
