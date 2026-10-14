<script setup lang="ts">

import { computed, ref, watch, type Component } from 'vue';
import { useNasLauncherContext } from '@/composables/useNasLauncherContext';
import { offAt, randomUuid } from '@valeriogc/nas-base-app';
import { scheduleWeekdays, usePowerSchedule } from '@/composables/usePowerSchedule';
import {
  filtersComplete,
  minuteOf,
  requiredFolders,
  scheduleComplete,
} from '@/services/backupRules';

import CommonProgress from '@/components/common/CommonProgress.vue';
import OriginDestinationStep from '@/components/rules/steps/OriginDestinationStep.vue';
import OriginFilteringStep from '@/components/rules/steps/OriginFilteringStep.vue';
import ScheduleStep from '@/components/rules/steps/ScheduleStep.vue';
import FileOperationsStep from '@/components/rules/steps/FileOperationsStep.vue';
import DestinationOptionsStep from '@/components/rules/steps/DestinationOptionsStep.vue';
import arrowLeft from '@/assets/icons/arrow-left.svg?url';
import arrowRight from '@/assets/icons/arrow-right.svg?url';

import type {
  BackupRule,
  DuplicateResolution,
  FileOperation,
  FileOperationConfigurations,
  NewBackupRule,
  RuleFolders,
  RuleSchedule,
  SectionCode,
  StepLabels,
  SyncMode,
} from '@/types/backup';

const { t } = useNasLauncherContext();

const props = withDefaults(
  defineProps<{
    section: SectionCode;
    /** The rule being edited; none for a new one. */
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

const currentStep = ref(1);

type RuleStep = {
  component: Component;
  label: keyof StepLabels;
};

const commonSteps: RuleStep[] = [
  { component: OriginFilteringStep, label: 'filtering' },
  { component: FileOperationsStep, label: 'operations' },
  { component: DestinationOptionsStep, label: 'destination' },
];

// A real-time rule runs on every change: it has no schedule step.
const scheduleStep: RuleStep = { component: ScheduleStep, label: 'schedule' };
const stepsBySection: Record<SectionCode, RuleStep[]> = {
  scheduled: [{ component: OriginDestinationStep, label: 'folders' }, scheduleStep, ...commonSteps],
  cloud: [{ component: OriginDestinationStep, label: 'cloudFolder' }, scheduleStep, ...commonSteps],
  rtl: [{ component: OriginDestinationStep, label: 'folders' }, ...commonSteps],
};

const selectedTitle = computed(() =>
  props.initial ? t.value.editRule : t.value.modalTitles[props.section],
);
const sectionSteps = computed(() => stepsBySection[props.section]);
const activeStep = computed(() => {
  const step = sectionSteps.value[currentStep.value - 1];
  if (!step) {
    throw new Error(`No backup rule step at index ${currentStep.value}.`);
  }
  return { ...step, title: t.value.stepLabels[step.label] };
});

const newSchedule = (): RuleSchedule => ({
  frequency: 'daily',
  time: '',
  weekday: null,
  day: null,
  month: null,
});
const emptyConfigurations = (): FileOperationConfigurations => ({
  batchRename: null,
  archive: null,
  encrypt: null,
});

const name = ref('');
const folders = ref<RuleFolders>({ origin: null, destination: null, cloud: null });
const sameDestination = ref(false);
const syncMode = ref<SyncMode>('mirror');
const schedule = ref<RuleSchedule>(newSchedule());
const filters = ref<NewBackupRule['filters']>([]);
const fileOperations = ref<FileOperation[]>([]);
const fileOperationConfigurations = ref<FileOperationConfigurations>(emptyConfigurations());
const duplicateResolution = ref<DuplicateResolution | null>('overwrite');
const verifyArchivedFiles = ref(false);

/** Everything the rule holds, to tell whether it changed. */
const snapshot = () =>
  JSON.stringify([
    name.value,
    folders.value,
    sameDestination.value,
    syncMode.value,
    schedule.value,
    filters.value,
    fileOperations.value,
    fileOperationConfigurations.value,
    duplicateResolution.value,
    verifyArchivedFiles.value,
  ]);
let saved = '';

/** Starts from a rule, or from nothing. Encryption is not offered yet: an old rule loses it. */
function fill(rule: BackupRule | null) {
  currentStep.value = 1;
  name.value = rule?.name ?? '';
  folders.value = rule ? { ...rule.folders } : { origin: null, destination: null, cloud: null };
  sameDestination.value = rule?.sameDestination ?? false;
  syncMode.value = rule?.syncMode ?? 'mirror';
  schedule.value = rule?.schedule ? { ...rule.schedule } : newSchedule();
  filters.value = rule ? rule.filters.map((filter) => ({ ...filter })) : [];
  fileOperations.value = rule ? rule.operations.filter((operation) => operation !== 'encrypt') : [];
  fileOperationConfigurations.value = rule
    ? { ...rule.operationConfigurations, encrypt: null }
    : emptyConfigurations();
  duplicateResolution.value = rule ? rule.duplicateResolution : 'overwrite';
  verifyArchivedFiles.value = rule?.verifyArchivedFiles ?? false;
  saved = snapshot();
}

fill(props.initial);

const foldersChosen = computed(() =>
  requiredFolders(props.section, sameDestination.value).every(
    (kind) => folders.value[kind] !== null,
  ),
);
const named = computed(() => name.value.trim().length > 0 && name.value.trim().length <= 80);
const scheduled = computed(() => sectionSteps.value.includes(scheduleStep));
const power = usePowerSchedule();
// The days the NAS is off at the chosen time: a rule cannot run while it is off.
const offDays = computed(() => {
  const minute = minuteOf(schedule.value.time);
  return minute === null ? [] : offAt(power.ranges.value, scheduleWeekdays(schedule.value), minute);
});
const scheduleReady = computed(
  () =>
    !scheduled.value ||
    (scheduleComplete(schedule.value) &&
      power.state.value !== 'loading' &&
      offDays.value.length === 0),
);
// Writing into its own origin, a rule has to do something to the files.
const operationsReady = computed(() => !sameDestination.value || fileOperations.value.length > 0);
const filtersReady = computed(() => filtersComplete(filters.value));
const canContinue = computed(() => {
  switch (activeStep.value.component) {
    case OriginDestinationStep:
      return named.value && foldersChosen.value;
    case ScheduleStep:
      return scheduleReady.value;
    case OriginFilteringStep:
      return filtersReady.value;
    case FileOperationsStep:
      return operationsReady.value;
    default:
      return true;
  }
});
const canSave = computed(
  () =>
    currentStep.value === sectionSteps.value.length &&
    named.value &&
    foldersChosen.value &&
    scheduleReady.value &&
    filtersReady.value &&
    operationsReady.value &&
    duplicateResolution.value !== null,
);

// Read without `activeStep`: on a change of section this runs before the step is reset.
watch(
  () => sectionSteps.value[currentStep.value - 1]?.component,
  (component) => {
    if (component === ScheduleStep) {
      void power.load();
    }
  },
);

// The destination of a rule that writes into its origin is the origin itself.
watch(sameDestination, (same) => {
  if (same) {
    folders.value = { ...folders.value, destination: null };
  }
});

watch(
  [
    name,
    folders,
    sameDestination,
    syncMode,
    schedule,
    filters,
    fileOperations,
    fileOperationConfigurations,
    duplicateResolution,
    verifyArchivedFiles,
  ],
  () => emit('dirty-change', snapshot() !== saved),
  { deep: true, immediate: true },
);

watch(
  () => props.section,
  () => fill(null),
);

function previous() {
  if (!props.saving && currentStep.value > 1) {
    currentStep.value -= 1;
  }
}

function next() {
  if (!props.saving && canContinue.value && currentStep.value < sectionSteps.value.length) {
    currentStep.value += 1;
  }
}

function saveRule() {
  if (!props.saving && canSave.value) {
    emit('save', {
      id: props.initial?.id ?? randomUuid(),
      type: props.section,
      name: name.value.trim(),
      enabled: props.initial?.enabled ?? true,
      folders: { ...folders.value },
      sameDestination: props.section === 'rtl' && sameDestination.value,
      syncMode: sameDestination.value ? 'copy' : syncMode.value,
      schedule: scheduled.value ? { ...schedule.value } : null,
      filters: filters.value.map((filter) => ({ ...filter })),
      operations: [...fileOperations.value],
      operationConfigurations: {
        batchRename: fileOperationConfigurations.value.batchRename
          ? { ...fileOperationConfigurations.value.batchRename }
          : null,
        archive: fileOperationConfigurations.value.archive
          ? { ...fileOperationConfigurations.value.archive }
          : null,
        encrypt: null,
      },
      duplicateResolution: duplicateResolution.value,
      verifyArchivedFiles: verifyArchivedFiles.value,
      createdAt: props.initial?.createdAt ?? new Date().toISOString(),
    });
  }
}

</script>

<template>
  <div class="rule" :aria-busy="saving" @cancel="emit('close')" @close="emit('close')">
    <div v-if="saving" class="loading_overlay" aria-hidden="true"></div>
    <p v-if="saveError" class="save_error app_error alert_error" role="alert">{{ saveError }}</p>

    <header class="rule_header">
      <h2 id="backup-rule-title">{{ selectedTitle }}</h2>
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

    <CommonProgress :total-steps="sectionSteps.length" :current-step="currentStep" />

    <div class="rule_body">
      <OriginDestinationStep
        v-if="activeStep.component === OriginDestinationStep"
        v-model:folders="folders"
        v-model:name="name"
        v-model:same-destination="sameDestination"
        :title="activeStep.title"
        :type="section"
      />
      <ScheduleStep
        v-else-if="activeStep.component === ScheduleStep"
        v-model:schedule="schedule"
        :title="activeStep.title"
        :ranges="power.ranges.value"
        :power="power.state.value"
        :manual-start="power.manualStart.value"
        :off-days="offDays"
      />
      <OriginFilteringStep
        v-else-if="activeStep.component === OriginFilteringStep"
        v-model:rules="filters"
        :title="activeStep.title"
      />
      <FileOperationsStep
        v-else-if="activeStep.component === FileOperationsStep"
        v-model:operations="fileOperations"
        v-model:configurations="fileOperationConfigurations"
        :title="activeStep.title"
        :required="sameDestination"
        :keep-password="Boolean(initial?.operationConfigurations.archive)"
      />
      <DestinationOptionsStep
        v-else-if="activeStep.component === DestinationOptionsStep"
        v-model:duplicate-resolution="duplicateResolution"
        v-model:verify-archived-files="verifyArchivedFiles"
        v-model:sync-mode="syncMode"
        :in-place="sameDestination"
        :title="activeStep.title"
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
        v-if="currentStep < sectionSteps.length"
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
