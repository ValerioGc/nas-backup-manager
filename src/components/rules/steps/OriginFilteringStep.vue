<script setup lang="ts">

import { computed } from 'vue';
import { useI18n } from '@/i18n';
import { extensionsValid } from '@/services/backupRules';
import type {
  OriginFilterAction,
  OriginFilterOperator,
  OriginFilterRule,
  OriginFilterStatus,
} from '@/types/backup';

import addIcon from '@/assets/icons/add.svg?url';
import trashIcon from '@/assets/icons/trash.svg?url';

const { t } = useI18n();

defineProps<{ title: string }>();
const rules = defineModel<OriginFilterRule[]>('rules', { required: true });

const actions = computed<{ value: OriginFilterAction; label: string }[]>(() => [
  { value: 'include', label: t.value.filter.include },
  { value: 'exclude', label: t.value.filter.exclude },
]);

const statusOptions = computed<{ value: OriginFilterStatus; label: string }[]>(() => [
  { value: 'updated', label: t.value.filter.updated },
  { value: 'created', label: t.value.filter.created },
  { value: 'size', label: t.value.filter.size },
  { value: 'name', label: t.value.filter.fileName },
]);

const dateOperators = computed<{ value: Exclude<OriginFilterOperator, null>; label: string }[]>(
  () => [
    { value: 'before', label: t.value.filter.before },
    { value: 'after', label: t.value.filter.after },
    { value: 'greater', label: t.value.filter.greater },
    { value: 'less', label: t.value.filter.less },
  ],
);
const sizeOperators = computed(() =>
  dateOperators.value.filter((option) => option.value === 'greater' || option.value === 'less'),
);

const dateValues = computed(() => [
  { value: 'today', label: t.value.filter.today },
  { value: 'yesterday', label: t.value.filter.yesterday },
  { value: 'lastWeek', label: t.value.filter.lastWeek },
  { value: 'lastMonth', label: t.value.filter.lastMonth },
  { value: 'thisMonth', label: t.value.filter.thisMonth },
]);

const statusDefaults: Record<OriginFilterStatus, Pick<OriginFilterRule, 'operator' | 'value'>> = {
  updated: { operator: 'before', value: 'today' },
  created: { operator: 'before', value: 'today' },
  size: { operator: 'greater', value: '' },
  name: { operator: null, value: '' },
};

function operatorsFor(status: OriginFilterStatus) {
  if (status === 'size') {
    return sizeOperators.value;
  }

  return status === 'name' ? [] : dateOperators.value;
}

function filterLabel(label: string, id: number) {
  return `${label}, ${t.value.filter.rule.replace('{id}', String(id))}`;
}

function eventValue(event: Event): string {
  return (event.target as HTMLInputElement | HTMLSelectElement).value;
}

function updateRule(id: number, changes: Partial<Omit<OriginFilterRule, 'id'>>) {
  rules.value = rules.value.map((rule) => (rule.id === id ? { ...rule, ...changes } : rule));
}

function addRule() {
  const id = Math.max(0, ...rules.value.map((rule) => rule.id)) + 1;

  rules.value = [
    ...rules.value,
    { id, action: 'include', fileType: '*', status: 'updated', ...statusDefaults.updated },
  ];
}

function removeRule(id: number) {
  rules.value = rules.value.filter((rule) => rule.id !== id);
}

function setAction(id: number, value: string) {
  const selected = actions.value.find((option) => option.value === value);
  if (selected) {
    updateRule(id, { action: selected.value });
  }
}

function setStatus(id: number, value: string) {
  const selected = statusOptions.value.find((option) => option.value === value);
  if (!selected) {
    return;
  }

  updateRule(id, { status: selected.value, ...statusDefaults[selected.value] });
}

function setOperator(id: number, status: OriginFilterStatus, value: string) {
  const selected = operatorsFor(status).find((option) => option.value === value);
  if (selected) {
    updateRule(id, { operator: selected.value });
  }
}

</script>

<template>
  <section class="origin_filter">
    <h3 class="filter_title">{{ title }}</h3>

    <div class="filter_scroll">
      <table class="filter_table">
        <thead>
          <tr>
            <th scope="col">{{ t.filter.action }}</th>
            <th scope="col">{{ t.filter.fileType }}</th>
            <th scope="col">{{ t.filter.status }}</th>
            <th scope="col">{{ t.filter.operator }}</th>
            <th scope="col">{{ t.filter.value }}</th>
            <th scope="col">{{ t.filter.actions }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="rule in rules" :key="rule.id">
            <td>
              <select
                :name="`filter_action_${rule.id}`"
                :aria-label="filterLabel(t.filter.action, rule.id)"
                :value="rule.action"
                @change="setAction(rule.id, eventValue($event))"
              >
                <option v-for="option in actions" :key="option.value" :value="option.value">
                  {{ option.label }}
                </option>
              </select>
            </td>
            <td>
              <input
                type="text"
                :name="`filter_files_${rule.id}`"
                :aria-label="filterLabel(t.filter.fileType, rule.id)"
                :aria-invalid="!extensionsValid(rule.fileType)"
                :title="extensionsValid(rule.fileType) ? undefined : t.filter.invalidExtensions"
                :placeholder="t.filter.extensionsPlaceholder"
                :value="rule.fileType"
                @input="updateRule(rule.id, { fileType: eventValue($event) })"
              />
            </td>
            <td>
              <select
                :name="`filter_status_${rule.id}`"
                :aria-label="filterLabel(t.filter.status, rule.id)"
                :value="rule.status"
                @change="setStatus(rule.id, eventValue($event))"
              >
                <option v-for="option in statusOptions" :key="option.value" :value="option.value">
                  {{ option.label }}
                </option>
              </select>
            </td>
            <td>
              <select
                v-if="rule.status !== 'name'"
                :name="`filter_operator_${rule.id}`"
                :aria-label="filterLabel(t.filter.operator, rule.id)"
                :value="rule.operator ?? ''"
                @change="setOperator(rule.id, rule.status, eventValue($event))"
              >
                <option
                  v-for="option in operatorsFor(rule.status)"
                  :key="option.value"
                  :value="option.value"
                >
                  {{ option.label }}
                </option>
              </select>
              <span v-else class="operator_empty">-</span>
            </td>
            <td>
              <select
                v-if="rule.status === 'updated' || rule.status === 'created'"
                :name="`filter_day_${rule.id}`"
                :aria-label="filterLabel(t.filter.value, rule.id)"
                :value="rule.value"
                @change="updateRule(rule.id, { value: eventValue($event) })"
              >
                <option v-for="option in dateValues" :key="option.value" :value="option.value">
                  {{ option.label }}
                </option>
              </select>
              <input
                v-else-if="rule.status === 'size'"
                type="number"
                min="0"
                step="any"
                :name="`filter_size_${rule.id}`"
                :placeholder="t.filter.sizeMb"
                :title="t.filter.sizeMb"
                :aria-label="filterLabel(t.filter.sizeMb, rule.id)"
                :value="rule.value"
                @input="updateRule(rule.id, { value: eventValue($event) })"
              />
              <input
                v-else
                type="text"
                :name="`filter_name_${rule.id}`"
                :aria-label="filterLabel(t.filter.fileName, rule.id)"
                :value="rule.value"
                @input="updateRule(rule.id, { value: eventValue($event) })"
              />
            </td>
            <td>
              <button
                type="button"
                class="app_button app_button--icon filter_remove"
                :aria-label="t.filter.removeFilter.replace('{id}', String(rule.id))"
                :title="t.filter.remove"
                @click="removeRule(rule.id)"
              >
                <img class="app_icon" :src="trashIcon" alt="" aria-hidden="true" />
              </button>
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <button type="button" class="filter_add app_button app_button--primary" @click="addRule">
      <img class="app_icon" :src="addIcon" alt="" aria-hidden="true" />
      {{ t.filter.add }}
    </button>
  </section>
</template>

<style scoped lang="scss">

.origin_filter {
  min-width: 0;
}

.filter_title {
  margin: 0 0 16px;
  font-size: 20px;
}

.filter_scroll {
  overflow-x: auto;
  padding-bottom: 4px;
}

.filter_table {
  min-width: 760px;
  border-collapse: separate;
  border-spacing: 0 8px;

  th {
    padding: 0 12px 4px;
    border-bottom: 0;
    color: var(--nas-muted);
    font-size: 0.875rem;
    font-weight: 600;
    white-space: nowrap;
  }

  td {
    padding: 10px 12px;
    border-top: 1px solid var(--nas-border);
    border-bottom: 1px solid var(--nas-border);
    background: var(--nas-surface);

    &:first-child {
      border-left: 1px solid var(--nas-border);
      border-radius: 8px 0 0 8px;
    }

    &:last-child {
      border-right: 1px solid var(--nas-border);
      border-radius: 0 8px 8px 0;
    }
  }

  select,
  input {
    width: 100%;
    min-width: 0;
    min-height: 40px;
    padding: 8px 10px;
    border: 1px solid var(--nas-border);
    border-radius: 7px;
    background: var(--nas-surface);
    color: var(--nas-foreground);
    font: inherit;
  }

  input[aria-invalid='true'] {
    border-color: var(--nas-error);
  }
}

.operator_empty {
  display: inline-block;
  padding: 8px;
  color: var(--nas-muted);
  text-align: center;
}

.filter_add {
  --app-icon-size: 16px;
  margin-top: 8px;
}

.filter_remove {
  --app-icon-size: 18px;
  color: var(--nas-error);

  &:hover:not(:disabled) {
    border-color: var(--nas-error);
    background: color-mix(in srgb, var(--nas-error) 10%, transparent);
  }
}

</style>
