import { flushPromises, type VueWrapper } from '@vue/test-utils';
import { vi } from 'vitest';
import { requestPowerSchedule } from '@/services/nasShell';
import type { PowerSchedule } from '@valeriogc/nas-base-app';

// The launcher's power schedule, mocked by the spec: by default the NAS never switches off.
export function answerPowerSchedule(schedule: PowerSchedule = { enabled: false, rules: [] }) {
  vi.mocked(requestPowerSchedule).mockResolvedValue({ schedule });
}

// Chooses the time on the schedule step of a rule, once the power schedule has been read.
export async function chooseSchedule(wrapper: VueWrapper, time = '02:00') {
  await flushPromises();
  await wrapper.get('[data-testid="schedule_time"]').setValue(time);
}
