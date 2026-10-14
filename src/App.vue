<script setup lang="ts">

import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue';
import { NasAppLayout, NasConfirmDialog, type NasCategory } from '@valeriogc/nas-base-app';
import { useNasLauncherContext } from '@/composables/useNasLauncherContext';
import { useNetworkDisks } from '@/composables/useNetworkDisks';
import {
  deleteBackupRule,
  fetchBackupRules,
  runBackupRule,
  saveBackupRule,
  stopBackupRule,
  switchBackupRule,
} from '@/services/backupRules';
import { describeSchedule, scheduleKind } from '@valeriogc/nas-base-app';

import type { BackupRule, ListColumn, ListedRule, ListRow, SectionCode } from '@/types/backup';

import CommonList from '@/components/common/CommonList.vue';
import CommonRuleView from '@/components/rules/CommonRuleView.vue';
import RuleRunStatus from '@/components/rules/RuleRunStatus.vue';
import NetworkDiskNotice from '@/components/cloud/NetworkDiskNotice.vue';
import cloudIcon from '@/assets/icons/cloud.svg?url';
import editIcon from '@/assets/icons/edit.svg?url';
import playIcon from '@/assets/icons/play.svg?url';
import realtimeIcon from '@/assets/icons/realtime.svg?url';
import scheduledIcon from '@/assets/icons/scheduled.svg?url';
import stopIcon from '@/assets/icons/stop.svg?url';
import trashIcon from '@/assets/icons/trash.svg?url';

// While a rule runs, the list is read again this often, and only while the page is in view.
const RUNNING_REFRESH_MS = 3000;

const { locale, t } = useNasLauncherContext();

const sectionCode = ref<SectionCode>('scheduled');
const loadingRules = ref(true);
const loadingError = ref<string | null>(null);
const savingRule = ref(false);
const saveError = ref<string | null>(null);
const ruleDirty = ref(false);
const discardDialogOpen = ref(false);
const pendingSection = ref<SectionCode | null>(null);
const rulesBySection = reactive<Record<SectionCode, ListedRule[]>>({
  scheduled: [],
  cloud: [],
  rtl: [],
});

const sections = computed<NasCategory<SectionCode>[]>(() => [
  { value: 'scheduled', label: t.value.sections.scheduled, icon: scheduledIcon },
  { value: 'rtl', label: t.value.sections.rtl, icon: realtimeIcon },
  { value: 'cloud', label: t.value.sections.cloud, icon: cloudIcon },
]);
const selectedSectionTitle = computed(() => t.value.sections[sectionCode.value]);
const selectedEmptyIcon = computed(
  () =>
    ({
      scheduled: scheduledIcon,
      cloud: cloudIcon,
      rtl: realtimeIcon,
    })[sectionCode.value],
);
const selectedEmptyTitle = computed(() => t.value.emptySections[sectionCode.value]);
const dateTime = computed(
  () => new Intl.DateTimeFormat(locale.value, { dateStyle: 'short', timeStyle: 'short' }),
);

/** When a rule runs next: a real-time one follows its origin, one turned off does not run. */
function nextRun(rule: ListedRule): string {
  if (!rule.enabled) {
    return t.value.ruleOff;
  }
  if (rule.type === 'rtl') {
    return t.value.realTime;
  }
  return rule.nextRunAt === null ? '-' : dateTime.value.format(rule.nextRunAt);
}

const selectedSectionRules = computed<ListRow[]>(() =>
  rulesBySection[sectionCode.value].map((rule) => ({
    id: rule.id,
    name: rule.name,
    schedule: rule.schedule ? scheduleKind(rule.schedule, t.value.frequencyNames) : null,
    enabled: null,
    lastRun: null,
    nextRun: nextRun(rule),
    createdAt: new Date(rule.createdAt).toLocaleDateString(locale.value),
  })),
);

// A real-time rule has no schedule: its section does without the column.
const baseColumns = computed<ListColumn[]>(() => [
  { key: 'name', label: t.value.columns.name },
  ...(sectionCode.value === 'rtl' ? [] : [{ key: 'schedule', label: t.value.columns.schedule }]),
  { key: 'enabled', label: t.value.columns.enabled },
  { key: 'lastRun', label: t.value.columns.lastRun },
  { key: 'nextRun', label: t.value.columns.nextRun },
  { key: 'createdAt', label: t.value.columns.createdAt },
]);

function ruleOf(id: string): ListedRule | undefined {
  return rulesBySection[sectionCode.value].find((rule) => rule.id === id);
}

const named = (text: string, rule: ListedRule | undefined) =>
  text.replace('{name}', rule?.name ?? '');

const runLabel = (id: string) =>
  named(ruleOf(id)?.running ? t.value.ruleActions.stop : t.value.ruleActions.run, ruleOf(id));

const activeView = ref<string>('list');
// The rule being edited, or none for a new one.
const editing = ref<ListedRule | null>(null);
const pendingDelete = ref<ListedRule | null>(null);
// The rules an action is under way for, which their buttons wait for.
const acting = ref<string[]>([]);
const actionError = ref<string | null>(null);
const networkDisks = useNetworkDisks();
const networkDisksMissing = computed(
  () =>
    sectionCode.value === 'cloud' &&
    networkDisks.loaded.value &&
    !networkDisks.loading.value &&
    !networkDisks.failed.value &&
    networkDisks.disks.value.length === 0,
);
const networkDisksFailed = computed(
  () => sectionCode.value === 'cloud' && networkDisks.failed.value && !networkDisks.loading.value,
);
// A cloud rule needs a network disk: none can be added before one is configured.
const canAddRule = computed(
  () =>
    sectionCode.value !== 'cloud' ||
    (networkDisks.loaded.value && networkDisks.disks.value.length > 0),
);

// The Cloud page says where to add a network disk when none is configured yet.
watch([sectionCode, activeView], ([section, view]) => {
  if (section === 'cloud' && view === 'list') {
    void networkDisks.load();
  }
});

function requestSectionChange(section: SectionCode) {
  if (section === sectionCode.value || savingRule.value) {
    return;
  }
  if (activeView.value !== 'rule') {
    sectionCode.value = section;
    return;
  }
  if (ruleDirty.value) {
    pendingSection.value = section;
    discardDialogOpen.value = true;
    return;
  }

  sectionCode.value = section;
  activeView.value = 'list';
  editing.value = null;
  saveError.value = null;
}

function requestRuleClose() {
  if (savingRule.value) {
    return;
  }
  if (ruleDirty.value) {
    pendingSection.value = null;
    discardDialogOpen.value = true;
    return;
  }

  activeView.value = 'list';
  editing.value = null;
  saveError.value = null;
}

function cancelDiscard() {
  discardDialogOpen.value = false;
  pendingSection.value = null;
}

function discardRule() {
  const nextSection = pendingSection.value;
  pendingSection.value = null;
  discardDialogOpen.value = false;
  ruleDirty.value = false;
  saveError.value = null;
  activeView.value = 'list';
  editing.value = null;
  if (nextSection) {
    sectionCode.value = nextSection;
  }
}

/** Reads the rules; `quietly` while the list is already shown, without its loading state. */
async function loadRules(quietly = false) {
  if (!quietly) {
    loadingRules.value = true;
  }
  try {
    const rules = await fetchBackupRules();
    for (const section of Object.keys(rulesBySection) as SectionCode[]) {
      rulesBySection[section] = rules.filter((rule) => rule.type === section);
    }
    loadingError.value = null;
  } catch (error) {
    if (!quietly) {
      loadingError.value = error instanceof Error ? error.message : t.value.errors.loadRules;
    }
  } finally {
    loadingRules.value = false;
  }
}

async function addSavedRule(rule: BackupRule) {
  if (savingRule.value) {
    return;
  }

  savingRule.value = true;
  saveError.value = null;
  try {
    await saveBackupRule(rule, editing.value !== null);
    ruleDirty.value = false;
    activeView.value = 'list';
    editing.value = null;
    await loadRules(true);
  } catch (error) {
    saveError.value = error instanceof Error ? error.message : t.value.errors.saveRule;
  } finally {
    savingRule.value = false;
  }
}

function newRule() {
  editing.value = null;
  activeView.value = 'rule';
}

function editRule(id: string) {
  editing.value = ruleOf(id) ?? null;
  activeView.value = editing.value ? 'rule' : 'list';
}

/** An action on one rule, after which the list is read again. */
async function act(id: string, action: () => Promise<void>) {
  if (acting.value.includes(id)) {
    return;
  }
  acting.value = [...acting.value, id];
  actionError.value = null;
  try {
    await action();
  } catch (error) {
    actionError.value = error instanceof Error ? error.message : t.value.errors.action;
  } finally {
    acting.value = acting.value.filter((item) => item !== id);
  }
  await loadRules(true);
}

function runOrStop(id: string) {
  const rule = ruleOf(id);
  void act(id, () => (rule?.running ? stopBackupRule(id) : runBackupRule(id)));
}

function switchRule(id: string, enabled: boolean) {
  void act(id, () => switchBackupRule(id, enabled));
}

function confirmDelete() {
  const rule = pendingDelete.value;
  pendingDelete.value = null;
  if (rule) {
    void act(rule.id, () => deleteBackupRule(rule.id));
  }
}

const anyRunning = computed(() =>
  Object.values(rulesBySection).some((rules) => rules.some((rule) => rule.running)),
);
let refresh: ReturnType<typeof setInterval> | undefined;

watch(anyRunning, (running) => {
  clearInterval(refresh);
  refresh = running
    ? setInterval(() => {
        if (document.visibilityState === 'visible') {
          void loadRules(true);
        }
      }, RUNNING_REFRESH_MS)
    : undefined;
});

onBeforeUnmount(() => clearInterval(refresh));

onMounted(() => loadRules());

</script>

<template>
  <!-- A section change goes through requestSectionChange: a rule being edited may be dirty. -->
  <NasAppLayout
    :model-value="sectionCode"
    :categories="sections"
    :navigation-label="t.type"
    @update:model-value="requestSectionChange"
  >
    <template #icon="{ category }">
      <img class="app_icon category_icon" :src="category.icon" alt="" aria-hidden="true" />
    </template>
    <div v-if="loadingError" class="error_state" role="alert">
      <span class="error_icon" aria-hidden="true">!</span>
      <div class="error_content">
        <h2>{{ t.errors.loadTitle }}</h2>
        <p class="error_message">{{ loadingError }}</p>
      </div>
      <button
        type="button"
        class="retry_button app_button app_button--primary"
        :disabled="loadingRules"
        @click="loadRules()"
      >
        {{ loadingRules ? t.loadingRules : t.retry }}
      </button>
    </div>
    <p v-else-if="loadingRules" class="loading_state">
      <output>{{ t.loadingRules }}</output>
    </p>

    <CommonList
      v-if="activeView === 'list' && !loadingRules && !loadingError"
      :title="selectedSectionTitle"
      :columns="baseColumns"
      :list="selectedSectionRules"
      :add-label="t.add"
      :empty-icon="selectedEmptyIcon"
      :empty-title="selectedEmptyTitle"
      :empty-description="t.emptyDescription"
      :actions-label="t.columns.actions"
      :can-add="canAddRule"
      @add="newRule"
    >
      <template v-if="networkDisksMissing || networkDisksFailed || actionError" #notice>
        <NetworkDiskNotice v-if="networkDisksMissing" />
        <div v-if="networkDisksFailed" class="app_error alert_error disks_failed" role="alert">
          <span>{{ t.networkDisks.loadError }}</span>
          <button
            type="button"
            class="app_button app_button--small"
            data-testid="network_disks_retry"
            @click="networkDisks.load()"
          >
            {{ t.retry }}
          </button>
        </div>
        <p v-if="actionError" class="app_error alert_error action_error" role="alert">
          {{ actionError }}
        </p>
      </template>

      <template #cell_enabled="{ row }">
        <label class="rule_switch">
          <input
            type="checkbox"
            role="switch"
            :name="`rule_enabled_${row.id}`"
            :checked="ruleOf(row.id)?.enabled"
            :aria-checked="ruleOf(row.id)?.enabled ?? false"
            :disabled="acting.includes(row.id)"
            :aria-label="named(t.ruleActions.enable, ruleOf(row.id))"
            :data-testid="`rule_enabled_${row.id}`"
            @change="switchRule(row.id, ($event.target as HTMLInputElement).checked)"
          />
          <span class="rule_switch_track" aria-hidden="true"></span>
        </label>
      </template>

      <template #cell_schedule="{ row }">
        <span
          v-if="ruleOf(row.id)?.schedule"
          :title="describeSchedule(ruleOf(row.id)!.schedule!, locale, t.scheduleSummary)"
        >
          {{ row.schedule }}
        </span>
        <template v-else>-</template>
      </template>

      <template #cell_lastRun="{ row }">
        <RuleRunStatus
          :running="ruleOf(row.id)?.running ?? false"
          :last-run="ruleOf(row.id)?.lastRun ?? null"
        />
      </template>

      <template #actions="{ row }">
        <div class="rule_actions">
          <button
            v-if="sectionCode !== 'cloud'"
            type="button"
            class="app_button app_button--icon"
            :disabled="acting.includes(row.id)"
            :aria-label="runLabel(row.id)"
            :title="runLabel(row.id)"
            :data-testid="`rule_run_${row.id}`"
            @click="runOrStop(row.id)"
          >
            <img
              class="app_icon"
              :src="ruleOf(row.id)?.running ? stopIcon : playIcon"
              alt=""
              aria-hidden="true"
            />
          </button>
          <button
            type="button"
            class="app_button app_button--icon"
            :disabled="acting.includes(row.id) || ruleOf(row.id)?.running"
            :aria-label="named(t.ruleActions.edit, ruleOf(row.id))"
            :title="named(t.ruleActions.edit, ruleOf(row.id))"
            :data-testid="`rule_edit_${row.id}`"
            @click="editRule(row.id)"
          >
            <img class="app_icon" :src="editIcon" alt="" aria-hidden="true" />
          </button>
          <button
            type="button"
            class="app_button app_button--icon"
            :disabled="acting.includes(row.id) || ruleOf(row.id)?.running"
            :aria-label="named(t.ruleActions.delete, ruleOf(row.id))"
            :title="named(t.ruleActions.delete, ruleOf(row.id))"
            :data-testid="`rule_delete_${row.id}`"
            @click="pendingDelete = ruleOf(row.id) ?? null"
          >
            <img class="app_icon" :src="trashIcon" alt="" aria-hidden="true" />
          </button>
        </div>
      </template>
    </CommonList>

    <CommonRuleView
      v-else-if="activeView !== 'list'"
      :key="editing?.id ?? 'new'"
      :section="sectionCode"
      :initial="editing"
      :saving="savingRule"
      :save-error="saveError"
      @dirty-change="ruleDirty = $event"
      @save="addSavedRule"
      @close="requestRuleClose"
    />
  </NasAppLayout>

  <NasConfirmDialog
    v-if="discardDialogOpen"
    :title="t.confirmDiscardTitle"
    :message="t.confirmDiscardMessage"
    :cancel-label="t.cancel"
    :confirm-label="t.discardChanges"
    @cancel="cancelDiscard"
    @confirm="discardRule"
  />

  <NasConfirmDialog
    v-if="pendingDelete"
    :title="t.deleteTitle"
    :message="named(t.deleteMessage, pendingDelete)"
    :cancel-label="t.cancel"
    :confirm-label="t.deleteConfirm"
    @cancel="pendingDelete = null"
    @confirm="confirmDelete"
  />
</template>

<style scoped lang="scss">

.error_state {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: 16px;
  width: min(100%, 720px);
  margin: 24px auto;
  padding: 20px 24px;
  border: 1px solid var(--nas-border);
  border-left: 4px solid var(--nas-error);
  border-radius: 12px;
  background: var(--nas-surface);
  box-shadow: 0 8px 24px #0000000d;
}

.error_icon {
  display: grid;
  place-items: center;
  width: 38px;
  height: 38px;
  border-radius: 50%;
  background: color-mix(in srgb, var(--nas-error) 12%, transparent);
  color: var(--nas-error);
  font-size: 23px;
  font-weight: 700;
}

.error_content {
  min-width: 0;

  h2 {
    margin: 0;
    font-size: 18px;
  }
}

.error_message {
  overflow-wrap: anywhere;
  margin: 4px 0 0;
  color: var(--nas-muted);
}

.retry_button {
  justify-self: end;
}

.loading_state {
  color: var(--nas-muted);
}

.disks_failed {
  align-items: center;

  span {
    flex: 1;
  }
}

.action_error {
  margin: 0;
  overflow-wrap: anywhere;
}

.rule_actions {
  display: inline-flex;
  gap: 6px;
}

.rule_switch {
  position: relative;
  display: inline-flex;
  cursor: pointer;

  input {
    position: absolute;
    inset: 0;
    margin: 0;
    opacity: 0;
    cursor: inherit;
  }

  input:disabled {
    cursor: default;
  }
}

.rule_switch_track {
  position: relative;
  pointer-events: none;
  width: 36px;
  height: 20px;
  border-radius: 10px;
  background: var(--nas-border);
  transition: background-color 0.16s ease;

  &::after {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--nas-surface);
    content: '';
    transition: transform 0.16s ease;
  }

  input:checked + & {
    background: var(--accent, #3c8fbe);
  }

  input:checked + &::after {
    transform: translateX(16px);
  }

  input:focus-visible + & {
    outline: 2px solid var(--accent, #3c8fbe);
    outline-offset: 2px;
  }

  input:disabled + & {
    opacity: 0.5;
  }
}

@media (max-width: 700px) {
  .error_state {
    grid-template-columns: auto minmax(0, 1fr);
    padding: 18px;
  }

  .retry_button {
    grid-column: 2;
    justify-self: start;
  }
}

</style>
