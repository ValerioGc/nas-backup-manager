import { expect, it, vi } from 'vitest';
import { scheduleWeekdays, usePowerSchedule } from '@/composables/usePowerSchedule';
import { requestPowerSchedule } from '@/services/nasShell';
import type { PowerRule } from '@valeriogc/nas-base-app';

vi.mock('@/services/nasShell', () => ({ requestPowerSchedule: vi.fn() }));

const rule = (day: PowerRule['day'], time: string, action: PowerRule['action']): PowerRule => ({
  day,
  time,
  action,
});

it('checks only its own weekday for a weekly rule', () => {
  const base = { time: '02:00', day: null, month: null };

  expect(scheduleWeekdays({ ...base, frequency: 'weekly', weekday: 3 })).toEqual([3]);
  expect(scheduleWeekdays({ ...base, frequency: 'daily', weekday: null })).toHaveLength(7);
  expect(scheduleWeekdays({ ...base, frequency: 'monthly', weekday: null, day: 1 })).toHaveLength(
    7,
  );
});

it('reads the schedule once and tells how it stands', async () => {
  vi.mocked(requestPowerSchedule).mockResolvedValue({
    schedule: { enabled: true, rules: [rule('everyday', '23:00', 'shutdown')] },
  });
  const power = usePowerSchedule();
  expect(power.state.value).toBe('loading');

  await Promise.all([power.load(), power.load()]);
  await power.load();
  expect(requestPowerSchedule).toHaveBeenCalledTimes(1);
  expect(power.state.value).toBe('ready');
  expect(power.manualStart.value).toBe(true);
  expect(power.ranges.value[0]).toEqual([{ from: 1380, to: 1440 }]);
});

it('says when the schedule is unreadable or never switches the NAS off', async () => {
  vi.mocked(requestPowerSchedule).mockResolvedValueOnce({
    schedule: null,
    error: 'Launcher unavailable',
  });
  const missing = usePowerSchedule();
  await missing.load();
  expect(missing.state.value).toBe('unavailable');

  vi.mocked(requestPowerSchedule).mockResolvedValueOnce({
    schedule: { enabled: true, rules: [rule('everyday', '07:00', 'start')] },
  });
  const alwaysOn = usePowerSchedule();
  await alwaysOn.load();
  expect(alwaysOn.state.value).toBe('alwaysOn');
  expect(alwaysOn.manualStart.value).toBe(false);
});
