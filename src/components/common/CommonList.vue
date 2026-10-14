<script lang="ts" setup>

import { NasEmptyState } from '@valeriogc/nas-base-app';
import type { ListColumn, ListRow } from '@/types/backup';
import addIcon from '@/assets/icons/add.svg?url';

withDefaults(
  defineProps<{
    title: string;
    columns: ListColumn[];
    list: ListRow[];
    addLabel: string;
    emptyIcon: string;
    emptyTitle: string;
    emptyDescription: string;
    /** The heading of the actions of each row, when the `actions` slot draws them. */
    actionsLabel?: string;
    /** Whether a rule can be added: not while the section misses what its rules need. */
    canAdd?: boolean;
  }>(),
  { actionsLabel: undefined, canAdd: true },
);

const emit = defineEmits<{ add: [] }>();

</script>

<template>
  <section class="list_section">
    <header class="list_header">
      <h1>{{ title }}</h1>

      <button
        v-if="canAdd"
        type="button"
        class="add_button app_button app_button--primary"
        data-testid="rule_add"
        @click="emit('add')"
      >
        <img class="app_icon" :src="addIcon" alt="" aria-hidden="true" />
        {{ addLabel }}
      </button>
    </header>

    <!-- What the section needs before its rules can work -->
    <div v-if="$slots.notice" class="list_notice">
      <slot name="notice" />
    </div>

    <NasEmptyState
      v-if="list.length === 0"
      class="empty_state"
      :title="emptyTitle"
      :description="emptyDescription"
    >
      <template #icon>
        <span class="empty_badge" aria-hidden="true">
          <img class="empty_icon" :src="emptyIcon" alt="" />
        </span>
      </template>
    </NasEmptyState>

    <div v-else class="list_scroll">
      <table>
        <thead>
          <tr>
            <th v-for="column in columns" :key="column.key" scope="col">
              {{ column.label }}
            </th>
            <th v-if="$slots.actions" scope="col" class="list_actions">{{ actionsLabel }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="row in list" :key="row.id">
            <td v-for="column in columns" :key="column.key">
              <slot :name="`cell_${column.key}`" :row="row">{{ row[column.key] ?? '-' }}</slot>
            </td>
            <td v-if="$slots.actions" class="list_actions">
              <slot name="actions" :row="row" />
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </section>
</template>

<style scoped lang="scss">

.list_header {
  @extend %fx_between;
  gap: 16px;
  margin-bottom: 24px;
}

h1 {
  margin: 0;
  font-size: 24px;
}

.list_notice {
  margin-bottom: 24px;
}

.add_button {
  --app-icon-size: 16px;
}

.list_scroll {
  overflow-x: auto;
}

th,
td {
  padding: 12px;
  vertical-align: middle;
}

th {
  white-space: nowrap;
}

.list_actions {
  width: 1%;
  text-align: right;
  white-space: nowrap;
}

.empty_state {
  min-height: 280px;
  border: 1px dashed var(--nas-border);
  border-radius: 12px;
  background: var(--nas-surface);
}

.empty_badge {
  display: grid;
  place-items: center;
  width: 48px;
  height: 48px;
  border-radius: 50%;
  background: var(--nas-hover);
}

// A black stroke like the other icons: the theme gives it its colour, not to the circle around it.
.empty_icon {
  width: 24px;
  height: 24px;
  filter: var(--app-neutral-icon-filter);
}

</style>
