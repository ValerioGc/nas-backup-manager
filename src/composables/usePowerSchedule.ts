import { computed, ref } from 'vue';
import { offRanges, type PowerSchedule } from '@valeriogc/nas-base-app';
import { requestPowerSchedule } from '@/services/nasShell';
import type { RuleSchedule } from '@/types/backup';
import type { PowerState } from '@/types/nas';

/**
 * Weekdays a schedule can fall on. A weekly rule has its own, the others any day, since a day of
 * the month changes weekday from month to month.
 */
export function scheduleWeekdays(schedule: RuleSchedule): number[] {
  return schedule.frequency === 'weekly' && schedule.weekday !== null
    ? [schedule.weekday]
    : [1, 2, 3, 4, 5, 6, 7];
}

/** Power schedule of the NAS, read through the launcher when first needed. */
export function usePowerSchedule() {
  const schedule = ref<PowerSchedule | null>(null);
  const loaded = ref(false);
  const loading = ref(false);
  const failed = ref(false);
  const ranges = computed(() => offRanges(schedule.value));
  const state = computed<PowerState>(() => {
    if (!loaded.value) {
      return 'loading';
    }
    if (failed.value) {
      return 'unavailable';
    }
    return ranges.value.some((day) => day.length > 0) ? 'ready' : 'alwaysOn';
  });

  const manualStart = computed(
    () => state.value === 'ready' && !schedule.value?.rules.some((rule) => rule.action === 'start'),
  );

  async function load() {
    if (loading.value || loaded.value) {
      return;
    }
    loading.value = true;
    const result = await requestPowerSchedule();
    schedule.value = result.schedule;
    failed.value = result.error !== undefined;
    loaded.value = true;
    loading.value = false;
  }

  return { ranges, state, manualStart, load };
}
