<script setup lang="ts">

import { computed, ref, watch } from 'vue';
import { useNasLauncherContext } from '@/composables/useNasLauncherContext';
import { minuteOf, offAt, randomUuid } from '@valeriogc/nas-base-app';
import { scheduleWeekdays, usePowerSchedule } from '@/composables/usePowerSchedule';
import { filtersComplete, requiredFolders, scheduleComplete } from '@/services/backupRules';

import CommonProgress from '@/components/common/CommonProgress.vue';
import OriginDestinationStep from '@/components/rules/steps/OriginDestinationStep.vue';
import OriginFilteringStep from '@/components/rules/steps/OriginFilteringStep.vue';
import ScheduleStep from '@/components/rules/steps/ScheduleStep.vue';
import FileOperationsStep from '@/components/rules/steps/FileOperationsStep.vue';
import DestinationOptionsStep from '@/components/rules/steps/DestinationOptionsStep.vue';
import arrowLeft from '@/assets/icons/arrow-left.svg?url';
import arrowRight from '@/assets/icons/arrow-right.svg?url';

import type { BackupRule, RuleForm, SectionCode, StepId } from '@/types/backup';

const props = withDefaults(
  defineProps<{
    section: SectionCode;
    initial?: BackupRule | null;
    saving?: boolean;
    saveError?: string | null;
  }>(),
  {
    initial: null,
    saving: false,
    saveError: null,
  },
);

const emit = defineEmits<{
  close: [];
  'dirty-change': [isDirty: boolean];
  save: [rule: BackupRule];
}>();

const { t } = useNasLauncherContext();
const power = usePowerSchedule();

const stepsBySection: Record<SectionCode, StepId[]> = {
  scheduled: ['folders', 'schedule', 'filtering', 'operations', 'destination'],
  cloud: ['folders', 'schedule', 'filtering', 'operations', 'destination'],
  rtl: ['folders', 'filtering', 'operations', 'destination'],
};

function formOf(rule: BackupRule | null): RuleForm {
  return {
    name: rule?.name ?? '',
    folders: rule ? { ...rule.folders } : { origin: null, destination: null, cloud: null },
    sameDestination: rule?.sameDestination ?? false,
    syncMode: rule?.syncMode ?? 'mirror',
    schedule: rule?.schedule
      ? { ...rule.schedule }
      : { frequency: 'daily', time: '', weekday: null, day: null, month: null },
    filters: rule ? rule.filters.map((filter) => ({ ...filter })) : [],
    operations: rule ? rule.operations.filter((operation) => operation !== 'encrypt') : [],
    operationConfigurations: rule
      ? { ...rule.operationConfigurations, encrypt: null }
      : { batchRename: null, archive: null, encrypt: null },
    duplicateResolution: rule ? rule.duplicateResolution : 'overwrite',
    verifyArchivedFiles: rule?.verifyArchivedFiles ?? false,
  };
}

const currentStep = ref(1);
const form = ref<RuleForm>(formOf(props.initial));
const saved = JSON.stringify(form.value);

const steps = computed(() => stepsBySection[props.section]);
const step = computed(() => steps.value[currentStep.value - 1]);
const isLast = computed(() => currentStep.value === steps.value.length);
const scheduled = computed(() => steps.value.includes('schedule'));
const title = computed(() =>
  props.initial ? t.value.editRule : t.value.modalTitles[props.section],
);
const stepTitle = computed(
  () =>
    t.value.stepLabels[
      props.section === 'cloud' && step.value === 'folders' ? 'cloudFolder' : step.value
    ],
);

const offDays = computed(() => {
  const minute = minuteOf(form.value.schedule.time);
  return minute === null
    ? []
    : offAt(power.ranges.value, scheduleWeekdays(form.value.schedule), minute);
});

const ready = computed<Record<StepId, boolean>>(() => {
  const { name, folders, sameDestination, schedule, filters, operations } = form.value;
  const trimmed = name.trim();
  return {
    folders:
      trimmed.length > 0 &&
      trimmed.length <= 80 &&
      requiredFolders(props.section, sameDestination).every((kind) => folders[kind] !== null),
    schedule:
      scheduleComplete(schedule) && power.state.value !== 'loading' && offDays.value.length === 0,
    filtering: filtersComplete(filters),
    operations: !sameDestination || operations.length > 0,
    destination: form.value.duplicateResolution !== null,
  };
});
const canContinue = computed(() => ready.value[step.value]);
const canSave = computed(() => isLast.value && steps.value.every((id) => ready.value[id]));

watch(step, (id) => {
  if (id === 'schedule') {
    void power.load();
  }
});

// A rule writing into its origin uses the origin as destination.
watch(
  () => form.value.sameDestination,
  (same) => {
    if (same) {
      form.value.folders = { ...form.value.folders, destination: null };
    }
  },
);

watch(form, () => emit('dirty-change', JSON.stringify(form.value) !== saved), {
  deep: true,
  immediate: true,
});

function previous() {
  if (!props.saving && currentStep.value > 1) {
    currentStep.value -= 1;
  }
}

function next() {
  if (!props.saving && canContinue.value && !isLast.value) {
    currentStep.value += 1;
  }
}

function saveRule() {
  if (props.saving || !canSave.value) {
    return;
  }

  const rule = form.value;
  emit('save', {
    ...rule,
    id: props.initial?.id ?? randomUuid(),
    type: props.section,
    name: rule.name.trim(),
    enabled: props.initial?.enabled ?? true,
    sameDestination: props.section === 'rtl' && rule.sameDestination,
    syncMode: rule.sameDestination ? 'copy' : rule.syncMode,
    schedule: scheduled.value ? rule.schedule : null,
    createdAt: props.initial?.createdAt ?? new Date().toISOString(),
  });
}

</script>

<template>
  <div class="rule" :aria-busy="saving">
    <div v-if="saving" class="loading_overlay" aria-hidden="true"></div>
    <p v-if="saveError" class="save_error app_error alert_error" role="alert">{{ saveError }}</p>

    <header class="rule_header">
      <h2>{{ title }}</h2>
      <button
        type="button"
        class="close_button app_button app_button--icon"
        :aria-label="t.close"
        :disabled="saving"
        @click="emit('close')"
      >
        <img class="app_icon" src="@/assets/icons/close.svg" alt="" />
      </button>
    </header>

    <CommonProgress :total-steps="steps.length" :current-step="currentStep" />

    <div class="rule_body">
      <h3 class="step_title">{{ stepTitle }}</h3>
      <OriginDestinationStep
        v-if="step === 'folders'"
        v-model:folders="form.folders"
        v-model:name="form.name"
        v-model:same-destination="form.sameDestination"
        :type="section"
      />
      <ScheduleStep
        v-else-if="step === 'schedule'"
        v-model:schedule="form.schedule"
        :ranges="power.ranges.value"
        :power="power.state.value"
        :manual-start="power.manualStart.value"
        :off-days="offDays"
      />
      <OriginFilteringStep v-else-if="step === 'filtering'" v-model:rules="form.filters" />
      <FileOperationsStep
        v-else-if="step === 'operations'"
        v-model:operations="form.operations"
        v-model:configurations="form.operationConfigurations"
        :required="form.sameDestination"
        :keep-password="Boolean(initial?.operationConfigurations.archive)"
      />
      <DestinationOptionsStep
        v-else-if="step === 'destination'"
        v-model:duplicate-resolution="form.duplicateResolution"
        v-model:verify-archived-files="form.verifyArchivedFiles"
        v-model:sync-mode="form.syncMode"
        :in-place="form.sameDestination"
      />
    </div>

    <footer class="rule_footer">
      <button
        type="button"
        class="app_button"
        :disabled="saving || currentStep <= 1"
        @click="previous"
      >
        <img class="app_icon" :src="arrowLeft" alt="" aria-hidden="true" />
        {{ t.back }}
      </button>

      <button
        v-if="!isLast"
        type="button"
        class="app_button app_button--primary"
        :disabled="saving || !canContinue"
        @click="next"
      >
        {{ t.next }}
        <img class="app_icon" :src="arrowRight" alt="" aria-hidden="true" />
      </button>

      <button
        v-else
        type="button"
        class="app_button app_button--primary"
        :disabled="saving || !canSave"
        @click="saveRule"
      >
        {{ t.save }}
      </button>
    </footer>
  </div>
</template>

<style scoped lang="scss">

.rule {
  position: relative;
  width: min(100%, 1080px);
  margin: 0 auto;
  padding: 8px 0 24px;
  color: var(--nas-foreground);

  &_header,
  &_footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }

  &_header {
    margin-bottom: 28px;
    padding-bottom: 20px;
    border-bottom: 1px solid var(--nas-border);
  }

  &_body {
    margin-top: 28px;
  }

  &_footer {
    margin-top: 32px;
    padding-top: 20px;
    border-top: 1px solid var(--nas-border);
  }
}

h2 {
  margin: 0;
  font-size: 26px;
}

.step_title {
  margin: 0 0 20px;
  font-size: 20px;
}

.close_button {
  --app-icon-size: 18px;
  flex: none;
}

.rule_footer .app_button {
  --app-icon-size: 16px;
}

.loading_overlay {
  position: absolute;
  inset: 0;
  background: #ffffff80;
  z-index: 1;
}

.save_error {
  margin: 0 0 16px;
}

</style>
