<script setup lang="ts">

import { computed } from 'vue';

const props = defineProps<{
  totalSteps: number;
  currentStep: number;
}>();

const stepCount = computed(() => Math.max(1, Math.trunc(props.totalSteps)));
const activeStep = computed(() =>
  Math.min(stepCount.value, Math.max(1, Math.trunc(props.currentStep))),
);

const steps = computed(() => Array.from({ length: stepCount.value }, (_, index) => index + 1));

</script>

<template>
  <div class="progress">
    <progress
      v-if="stepCount > 1"
      class="progress_track"
      :max="stepCount - 1"
      :value="activeStep - 1"
      :aria-valuetext="`${activeStep} / ${stepCount}`"
    ></progress>

    <ol class="progress_steps" :class="{ single: stepCount === 1 }" aria-hidden="true">
      <li
        v-for="step in steps"
        :key="step"
        class="progress_step"
        :class="{ completed: step < activeStep, current: step === activeStep }"
      >
        {{ step }}
      </li>
    </ol>
  </div>
</template>

<style scoped lang="scss">

.progress {
  position: relative;
  width: 100%;

  &_track {
    position: absolute;
    top: 50%;
    left: 18px;
    display: block;
    width: calc(100% - 36px);
    height: 3px;
    border: 0;
    transform: translateY(-50%);
    appearance: none;
    background: var(--nas-border);

    &::-webkit-progress-bar {
      background: var(--nas-border);
    }

    &::-webkit-progress-value {
      background: var(--accent, #3c8fbe);
      transition: width 0.25s ease;
    }

    &::-moz-progress-bar {
      background: var(--accent, #3c8fbe);
    }
  }

  &_steps {
    position: relative;
    display: flex;
    justify-content: space-between;
    margin: 0;
    padding: 0;
    list-style: none;

    &.single {
      justify-content: center;
    }
  }

  &_step {
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    flex: 0 0 36px;
    border: 2px solid var(--nas-border);
    border-radius: 50%;
    background: var(--nas-surface);
    color: var(--nas-muted);
    font-weight: 700;
    transition:
      background-color 0.25s ease,
      border-color 0.25s ease,
      color 0.25s ease;

    &.completed,
    &.current {
      border-color: var(--accent, #3c8fbe);
      background: var(--accent, #3c8fbe);
      color: var(--nas-surface);
    }

    &.current {
      box-shadow:
        0 0 0 3px var(--nas-surface),
        0 0 0 5px var(--accent, #3c8fbe);
    }
  }
}

</style>
