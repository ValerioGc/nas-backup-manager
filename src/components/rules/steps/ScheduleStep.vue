<script setup lang="ts">

import { computed } from 'vue';
import { useI18n } from '@/i18n';
import { minuteOf, monthDays } from '@/services/backupRules';
import {
  monthName as sharedMonthName,
  weekdayName as sharedWeekdayName,
  type MinuteRange,
} from '@valeriogc/nas-base-app';
import { scheduleWeekdays, type PowerState } from '@/composables/usePowerSchedule';
import type { RuleSchedule, ScheduleFrequency } from '@/types/backup';

const props = defineProps<{
  title: string;
  ranges: MinuteRange[][];
  power: PowerState;
  manualStart: boolean;
  offDays: number[];
}>();
const schedule = defineModel<RuleSchedule>('schedule', { required: true });

const { locale, t } = useI18n();

const DAY = 1440;
const frequencies: ScheduleFrequency[] = ['daily', 'weekly', 'monthly', 'yearly'];
const weekdays = [1, 2, 3, 4, 5, 6, 7];
const months = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
const scale = ['00:00', '06:00', '12:00', '18:00', '24:00'];

const minute = computed(() => minuteOf(schedule.value.time));
const checkedDays = computed(() => scheduleWeekdays(schedule.value));
const lastDay = computed(() =>
  schedule.value.frequency === 'yearly' ? (monthDays[(schedule.value.month ?? 1) - 1] ?? 31) : 31,
);
const shortMonth = computed(() => (schedule.value.day ?? 0) > 28);
const offMessage = computed(() =>
  t.value.schedule.offAtTime
    .replace('{time}', schedule.value.time)
    .replace('{days}', props.offDays.map((day) => weekdayName(day)).join(', ')),
);

function weekdayName(weekday: number, width: 'long' | 'short' = 'long') {
  return sharedWeekdayName(locale.value, weekday, width);
}

function monthName(month: number) {
  return sharedMonthName(locale.value, month);
}

function timeOf(minutes: number) {
  return `${String(Math.floor(minutes / 60)).padStart(2, '0')}:${String(minutes % 60).padStart(2, '0')}`;
}

function percent(minutes: number) {
  return `${(minutes / DAY) * 100}%`;
}

function dayLabel(index: number) {
  const ranges = props.ranges[index] ?? [];
  const off = ranges.map((range) => `${timeOf(range.from)}-${timeOf(range.to)}`).join(', ');
  const state = off ? `${t.value.schedule.off} ${off}` : t.value.schedule.on;
  return `${weekdayName(index + 1)}: ${state}`;
}

/** Keeps the time and the fields the new frequency shares, and drops the ones it does not use. */
function setFrequency(frequency: ScheduleFrequency) {
  const { time, weekday, day, month } = schedule.value;
  schedule.value = {
    frequency,
    time,
    weekday: frequency === 'weekly' ? (weekday ?? 1) : null,
    day: frequency === 'monthly' || frequency === 'yearly' ? (day ?? 1) : null,
    month: frequency === 'yearly' ? (month ?? 1) : null,
  };
}

function setMonth(month: number) {
  const day = Math.min(schedule.value.day ?? 1, monthDays[month - 1] ?? 31);
  schedule.value = { ...schedule.value, month, day };
}

function setField(field: 'weekday' | 'day', value: number) {
  schedule.value = { ...schedule.value, [field]: value };
}

function setTime(time: string) {
  schedule.value = { ...schedule.value, time };
}

function selected(event: Event) {
  return (event.target as HTMLSelectElement).value;
}

</script>

<template>
  <section class="schedule_step">
    <h3>{{ title }}</h3>

    <fieldset class="schedule_group">
      <legend>{{ t.schedule.frequency }}</legend>
      <div class="schedule_fields">
        <label class="schedule_label">
          {{ t.schedule.frequencyLabel }}
          <select
            class="schedule_input"
            data-testid="schedule_frequency"
            :value="schedule.frequency"
            @change="setFrequency(selected($event) as ScheduleFrequency)"
          >
            <option v-for="frequency in frequencies" :key="frequency" :value="frequency">
              {{ t.schedule[frequency] }}
            </option>
          </select>
        </label>

        <label v-if="schedule.frequency === 'weekly'" class="schedule_label">
          {{ t.schedule.weekday }}
          <select
            class="schedule_input"
            data-testid="schedule_weekday"
            :value="schedule.weekday"
            @change="setField('weekday', Number(selected($event)))"
          >
            <option v-for="weekday in weekdays" :key="weekday" :value="weekday">
              {{ weekdayName(weekday) }}
            </option>
          </select>
        </label>

        <label v-if="schedule.frequency === 'yearly'" class="schedule_label">
          {{ t.schedule.month }}
          <select
            class="schedule_input"
            data-testid="schedule_month"
            :value="schedule.month"
            @change="setMonth(Number(selected($event)))"
          >
            <option v-for="month in months" :key="month" :value="month">
              {{ monthName(month) }}
            </option>
          </select>
        </label>

        <label
          v-if="schedule.frequency === 'monthly' || schedule.frequency === 'yearly'"
          class="schedule_label"
        >
          {{ t.schedule.day }}
          <select
            class="schedule_input"
            data-testid="schedule_day"
            :value="schedule.day"
            @change="setField('day', Number(selected($event)))"
          >
            <option v-for="day in lastDay" :key="day" :value="day">{{ day }}</option>
          </select>
        </label>

        <label class="schedule_label">
          {{ t.schedule.time }}
          <input
            class="schedule_input"
            data-testid="schedule_time"
            type="time"
            required
            :value="schedule.time"
            @input="setTime(($event.target as HTMLInputElement).value)"
          />
        </label>
      </div>
      <p v-if="shortMonth" class="schedule_note" role="note">{{ t.schedule.shortMonth }}</p>
    </fieldset>

    <fieldset class="schedule_group">
      <legend>{{ t.schedule.powerTitle }}</legend>
      <output v-if="power === 'loading'" class="schedule_note">{{
        t.schedule.powerLoading
      }}</output>
      <p v-else-if="power === 'unavailable'" class="schedule_note" data-testid="power_unavailable">
        {{ t.schedule.powerUnavailable }}
      </p>
      <p v-else-if="power === 'alwaysOn'" class="schedule_note" data-testid="power_always_on">
        {{ t.schedule.alwaysOn }}
      </p>
      <template v-else>
        <ul class="power_week" data-testid="power_week">
          <li
            v-for="(day, index) in ranges"
            :key="index"
            class="power_day"
            :class="{ 'power_day--checked': checkedDays.includes(index + 1) }"
            :aria-label="dayLabel(index)"
          >
            <span class="power_day_name" aria-hidden="true">{{
              weekdayName(index + 1, 'short')
            }}</span>
            <span class="power_bar" aria-hidden="true">
              <span
                v-for="range in day"
                :key="range.from"
                class="power_off"
                :style="{ left: percent(range.from), width: percent(range.to - range.from) }"
              ></span>
              <span
                v-if="minute !== null && checkedDays.includes(index + 1)"
                class="power_marker"
                :class="{ 'power_marker--off': offDays.includes(index + 1) }"
                :style="{ left: percent(minute) }"
              ></span>
            </span>
          </li>
        </ul>
        <div class="power_scale" aria-hidden="true">
          <span v-for="mark in scale" :key="mark">{{ mark }}</span>
        </div>
        <p class="power_legend" aria-hidden="true">
          <span class="power_key power_key--on"></span>{{ t.schedule.on }}
          <span class="power_key power_key--off"></span>{{ t.schedule.off }}
        </p>
        <p v-if="manualStart" class="schedule_note" role="note">{{ t.schedule.manualStart }}</p>
      </template>
    </fieldset>

    <p v-if="offDays.length" class="app_error alert_error" role="alert" data-testid="schedule_off">
      {{ offMessage }}
    </p>
    <p v-else-if="minute === null" class="schedule_note" role="note">{{ t.schedule.chooseTime }}</p>
  </section>
</template>

<style scoped lang="scss">

.schedule_step {
  display: grid;
  gap: 20px;

  h3 {
    margin: 0;
    font-size: 20px;
  }

  .app_error {
    margin: 0;
  }
}

.schedule_group {
  @extend %option_group;
  display: grid;
  gap: 14px;
}

.schedule_fields {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 200px), 1fr));
  gap: 16px;
}

.schedule_label {
  display: grid;
  gap: 8px;
  font-weight: 600;
}

.schedule_input {
  width: 100%;
  min-height: 42px;
  padding: 10px 12px;
  border: 1px solid var(--nas-border);
  border-radius: 8px;
  background: var(--nas-surface);
  color: var(--nas-foreground);
  font: inherit;
  font-weight: 400;

  &:focus-visible {
    outline: 2px solid var(--accent, #316fea);
    outline-offset: 2px;
  }
}

.schedule_note {
  margin: 0;
  color: var(--nas-muted);
  font-size: 0.875rem;
}

.power_week {
  display: grid;
  gap: 6px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.power_day {
  display: grid;
  grid-template-columns: 48px 1fr;
  align-items: center;
  gap: 12px;
  color: var(--nas-muted);
  font-size: 0.875rem;

  &--checked {
    color: var(--nas-foreground);
    font-weight: 600;
  }
}

.power_bar {
  position: relative;
  overflow: hidden;
  height: 14px;
  border-radius: 7px;
  background: color-mix(in srgb, var(--accent, var(--nas-accent)) 35%, transparent);
}

.power_off {
  position: absolute;
  top: 0;
  bottom: 0;
  background: var(--nas-muted);
}

.power_marker {
  position: absolute;
  top: 0;
  bottom: 0;
  width: 3px;
  margin-left: -1px;
  background: var(--nas-foreground);

  &--off {
    background: var(--nas-error);
  }
}

.power_scale {
  display: flex;
  justify-content: space-between;
  margin-left: 60px;
  color: var(--nas-muted);
  font-size: 0.75rem;
}

.power_legend {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0;
  font-size: 0.875rem;
}

.power_key {
  width: 14px;
  height: 14px;
  border-radius: 4px;

  &--on {
    background: color-mix(in srgb, var(--accent, var(--nas-accent)) 35%, transparent);
  }

  &--off {
    margin-left: 8px;
    background: var(--nas-muted);
  }
}

</style>
