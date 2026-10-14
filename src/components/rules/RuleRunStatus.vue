<script setup lang="ts">

import { computed, ref } from 'vue';
import { useI18n } from '@/i18n';
import type { LastRun } from '@/types/backup';

const props = defineProps<{
  running: boolean;
  lastRun: LastRun | null;
}>();

const { locale, t } = useI18n();

const state = computed(() =>
  props.running ? 'running' : (props.lastRun?.status ?? ('never' as const)),
);

/** What the icon means, with the date and time of the last run and, when it failed, why. */
const details = computed(() => {
  if (props.running) {
    return [t.value.runStatus.running];
  }
  const last = props.lastRun;
  if (!last) {
    return [t.value.never];
  }
  const date = new Intl.DateTimeFormat(locale.value, {
    dateStyle: 'short',
    timeStyle: 'short',
  }).format(last.at);
  return [
    t.value.runStatus[last.status],
    t.value.lastRunAt.replace('{date}', date),
    ...(last.message ? [last.message] : []),
  ];
});

// Fixed to the window, where the scrolling table cannot cut it: above the icon, centred on it.
const place = ref<{ top: string; left: string } | null>(null);

function show(event: Event) {
  const icon = (event.currentTarget as HTMLElement).getBoundingClientRect();
  place.value = { top: `${icon.top - 8}px`, left: `${icon.left + icon.width / 2}px` };
}

</script>

<template>
  <button
    type="button"
    class="run_status"
    :aria-label="details.join('. ')"
    :data-testid="`run_status_${state}`"
    @mouseenter="show"
    @focus="show"
    @click="show"
    @keydown.esc="place = null"
    @mouseleave="place = null"
    @blur="place = null"
  >
    <span class="run_icon" :class="`run_icon--${state}`" aria-hidden="true"></span>
    <span v-if="place" class="run_tooltip" role="tooltip" :style="place">
      <span v-for="(line, index) in details" :key="index" class="run_tooltip_line">
        {{ line }}
      </span>
    </span>
  </button>
</template>

<style scoped lang="scss">

.run_status {
  position: relative;
  display: inline-flex;
  align-items: center;
  padding: 0;
  border: 0;
  border-radius: 50%;
  background: none;
  color: inherit;
  font: inherit;
  cursor: help;
  outline: none;

  &:focus-visible {
    box-shadow: 0 0 0 2px var(--accent, #3c8fbe);
  }
}

.run_icon {
  display: block;
  width: 22px;
  height: 22px;
  mask: var(--run-icon) center / contain no-repeat;
  background: var(--run-color);

  &--ok {
    --run-icon: url('@/assets/icons/status-ok.svg');
    --run-color: var(--tone-success);
  }

  &--failed {
    --run-icon: url('@/assets/icons/status-failed.svg');
    --run-color: var(--tone-error);
  }

  &--interrupted {
    --run-icon: url('@/assets/icons/status-interrupted.svg');
    --run-color: var(--tone-warning);
  }

  &--never {
    width: 14px;
    height: 2px;
    mask: none;
    background: var(--nas-muted);
  }

  &--running {
    width: 18px;
    height: 18px;
    margin: 2px;
    mask: none;
    border: 2px solid var(--nas-border);
    border-top-color: var(--accent, #3c8fbe);
    border-radius: 50%;
    background: none;
    animation: run_spin 0.9s linear infinite;
  }
}

.run_tooltip {
  position: fixed;
  z-index: 20;
  display: grid;
  gap: 2px;
  width: max-content;
  max-width: 320px;
  padding: 8px 10px;
  transform: translate(-50%, -100%);
  border: 1px solid var(--nas-border);
  border-radius: 8px;
  background: var(--nas-surface);
  box-shadow: 0 8px 24px #00000026;
  color: var(--nas-foreground);
  font-size: 13px;
  font-weight: 400;
  line-height: 1.4;
  text-align: left;
  white-space: normal;
  overflow-wrap: anywhere;
  pointer-events: none;
}

.run_tooltip_line:first-child {
  font-weight: 600;
}

@keyframes run_spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .run_icon--running {
    animation: none;
  }
}

</style>
