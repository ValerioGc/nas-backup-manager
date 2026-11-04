<script setup lang="ts">

import { computed, ref } from 'vue';
import { NasConfirmDialog, describeSchedule, scheduleKind } from '@valeriogc/nas-base-app';
import { useI18n } from '@/i18n';
import { useDateTime } from '@/composables/useDateTime';
import CommonList from '@/components/common/CommonList.vue';
import RuleRunStatus from '@/components/rules/RuleRunStatus.vue';
import type { ListColumn, ListedRule, SectionCode } from '@/types/backup';

import editIcon from '@/assets/icons/edit.svg?url';
import playIcon from '@/assets/icons/play.svg?url';
import stopIcon from '@/assets/icons/stop.svg?url';
import trashIcon from '@/assets/icons/trash.svg?url';

const props = defineProps<{
  section: SectionCode;
  rules: ListedRule[];
  acting: string[];
  actionError: string | null;
  emptyIcon: string;
  canAdd: boolean;
}>();

const emit = defineEmits<{
  add: [];
  edit: [rule: ListedRule];
  run: [rule: ListedRule];
  switch: [rule: ListedRule, enabled: boolean];
  delete: [rule: ListedRule];
}>();

const { locale, t } = useI18n();
const dateTime = useDateTime();
const pendingDelete = ref<ListedRule | null>(null);

const columns = computed<ListColumn[]>(() => [
  { key: 'name', label: t.value.columns.name },
  ...(props.section === 'rtl' ? [] : [{ key: 'schedule', label: t.value.columns.schedule }]),
  { key: 'enabled', label: t.value.columns.enabled },
  { key: 'lastRun', label: t.value.columns.lastRun },
  { key: 'nextRun', label: t.value.columns.nextRun },
  { key: 'createdAt', label: t.value.columns.createdAt },
]);

function nextRun(rule: ListedRule): string {
  if (!rule.enabled) {
    return t.value.ruleOff;
  }
  if (rule.type === 'rtl') {
    return t.value.realTime;
  }
  return rule.nextRunAt === null ? '-' : dateTime(rule.nextRunAt);
}

const rows = computed(() =>
  props.rules.map((rule) => ({
    id: rule.id,
    rule,
    name: rule.name,
    schedule: rule.schedule ? scheduleKind(rule.schedule, t.value.frequencyNames) : null,
    nextRun: nextRun(rule),
    createdAt: new Date(rule.createdAt).toLocaleDateString(locale.value),
  })),
);

const named = (text: string, rule: ListedRule) => text.replace('{name}', rule.name);
const busy = (rule: ListedRule) => props.acting.includes(rule.id);
const runLabel = (rule: ListedRule) =>
  named(rule.running ? t.value.ruleActions.stop : t.value.ruleActions.run, rule);

function confirmDelete() {
  const rule = pendingDelete.value;
  pendingDelete.value = null;
  if (rule) {
    emit('delete', rule);
  }
}

</script>

<template>
  <CommonList
    :title="t.sections[section]"
    :columns="columns"
    :list="rows"
    :add-label="t.add"
    :empty-icon="emptyIcon"
    :empty-title="t.emptySections[section]"
    :empty-description="t.emptyDescription"
    :actions-label="t.columns.actions"
    :can-add="canAdd"
    @add="emit('add')"
  >
    <template v-if="$slots.notice || actionError" #notice>
      <slot name="notice" />
      <p v-if="actionError" class="app_error alert_error action_error" role="alert">
        {{ actionError }}
      </p>
    </template>

    <template #cell_enabled="{ row: { rule } }">
      <label class="rule_switch">
        <input
          type="checkbox"
          role="switch"
          :name="`rule_enabled_${rule.id}`"
          :checked="rule.enabled"
          :aria-checked="rule.enabled"
          :disabled="busy(rule)"
          :aria-label="named(t.ruleActions.enable, rule)"
          :data-testid="`rule_enabled_${rule.id}`"
          @change="emit('switch', rule, ($event.target as HTMLInputElement).checked)"
        />
        <span class="rule_switch_track" aria-hidden="true"></span>
      </label>
    </template>

    <template #cell_schedule="{ row: { rule, schedule } }">
      <span
        v-if="rule.schedule"
        :title="describeSchedule(rule.schedule, locale, t.scheduleSummary)"
      >
        {{ schedule }}
      </span>
      <template v-else>-</template>
    </template>

    <template #cell_lastRun="{ row: { rule } }">
      <RuleRunStatus :running="rule.running" :last-run="rule.lastRun" />
    </template>

    <template #actions="{ row: { rule } }">
      <div class="rule_actions">
        <button
          v-if="section !== 'cloud'"
          type="button"
          class="app_button app_button--icon"
          :disabled="busy(rule)"
          :aria-label="runLabel(rule)"
          :title="runLabel(rule)"
          :data-testid="`rule_run_${rule.id}`"
          @click="emit('run', rule)"
        >
          <img
            class="app_icon"
            :src="rule.running ? stopIcon : playIcon"
            alt=""
            aria-hidden="true"
          />
        </button>
        <button
          type="button"
          class="app_button app_button--icon"
          :disabled="busy(rule) || rule.running"
          :aria-label="named(t.ruleActions.edit, rule)"
          :title="named(t.ruleActions.edit, rule)"
          :data-testid="`rule_edit_${rule.id}`"
          @click="emit('edit', rule)"
        >
          <img class="app_icon" :src="editIcon" alt="" aria-hidden="true" />
        </button>
        <button
          type="button"
          class="app_button app_button--icon"
          :disabled="busy(rule) || rule.running"
          :aria-label="named(t.ruleActions.delete, rule)"
          :title="named(t.ruleActions.delete, rule)"
          :data-testid="`rule_delete_${rule.id}`"
          @click="pendingDelete = rule"
        >
          <img class="app_icon" :src="trashIcon" alt="" aria-hidden="true" />
        </button>
      </div>
    </template>
  </CommonList>

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

</style>
