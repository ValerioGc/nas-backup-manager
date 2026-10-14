<script setup lang="ts">

import { computed, ref } from 'vue';
import { useI18n } from '@/i18n';
import ArchiveModal from '@/components/rules/steps/modals/ArchiveModal.vue';
import BatchRenameModal from '@/components/rules/steps/modals/BatchRenameModal.vue';
import type {
  ArchiveConfiguration,
  BatchRenameConfiguration,
  CryptConfiguration,
  FileOperation,
  FileOperationConfigurations,
} from '@/types/backup';

import archiveIcon from '@/assets/icons/archive.svg?url';
import renameIcon from '@/assets/icons/rename.svg?url';

/**
 * `required`: the rule writes into its origin and needs an operation. `keepPassword`: the rule is
 * saved already, and an empty password keeps the one it has. Encryption is not offered yet.
 */
defineProps<{ title: string; required?: boolean; keepPassword?: boolean }>();
const selectedOperations = defineModel<FileOperation[]>('operations', { required: true });
const configurations = defineModel<FileOperationConfigurations>('configurations', {
  required: true,
});
const { t } = useI18n();
const activeModal = ref<FileOperation | null>(null);

const operationCards = computed(() => [
  {
    id: 'batchRename' as const,
    icon: renameIcon,
    label: t.value.fileOperations.batchRename,
    description: t.value.fileOperations.batchRenameDescription,
  },
  {
    id: 'archive' as const,
    icon: archiveIcon,
    label: t.value.fileOperations.archive,
    description: t.value.fileOperations.archiveDescription,
  },
]);

function openModal(operation: FileOperation) {
  activeModal.value = operation;
}

function cancelModal() {
  const operation = activeModal.value;
  if (operation && selectedOperations.value.includes(operation)) {
    selectedOperations.value = selectedOperations.value.filter(
      (selected) => selected !== operation,
    );
    configurations.value = { ...configurations.value, [operation]: null };
  }
  activeModal.value = null;
}

function selectOperation(operation: FileOperation) {
  if (!selectedOperations.value.includes(operation)) {
    selectedOperations.value = [...selectedOperations.value, operation];
  }
}

function saveArchive(configuration: ArchiveConfiguration | CryptConfiguration) {
  if (!('archiveName' in configuration)) {
    return;
  }

  configurations.value = { ...configurations.value, archive: configuration };
  selectOperation('archive');
  activeModal.value = null;
}

function saveBatchRename(configuration: BatchRenameConfiguration) {
  configurations.value = { ...configurations.value, batchRename: configuration };
  selectOperation('batchRename');
  activeModal.value = null;
}

</script>

<template>
  <section class="file_operations">
    <h3>{{ title }}</h3>

    <ul class="operation_grid">
      <li v-for="operation in operationCards" :key="operation.id" class="operation_item">
        <button
          type="button"
          class="operation_card"
          :class="{ 'operation_card--selected': selectedOperations.includes(operation.id) }"
          :aria-pressed="selectedOperations.includes(operation.id)"
          aria-haspopup="dialog"
          @click="openModal(operation.id)"
        >
          <img class="operation_icon" :src="operation.icon" alt="" aria-hidden="true" />
          <span class="operation_copy">
            <span class="operation_name">{{ operation.label }}</span>
            <span class="operation_description">{{ operation.description }}</span>
          </span>
        </button>
      </li>
    </ul>

    <p v-if="required && selectedOperations.length === 0" class="operation_note" role="alert">
      {{ t.fileOperations.requiredInPlace }}
    </p>
    <p class="operation_note" role="note">{{ t.fileOperations.notSaved }}</p>

    <ArchiveModal
      v-if="activeModal === 'archive'"
      :initial-value="configurations.archive"
      :keep-password="keepPassword"
      @close="cancelModal"
      @save="saveArchive"
    />
    <BatchRenameModal
      v-if="activeModal === 'batchRename'"
      :initial-value="configurations.batchRename"
      @close="cancelModal"
      @save="saveBatchRename"
    />
  </section>
</template>

<style scoped lang="scss">

.file_operations {
  display: grid;
  gap: 20px;

  h3 {
    margin: 0;
    font-size: 20px;
  }
}

.operation_grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 240px), 1fr));
  gap: 14px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.operation_item {
  min-width: 0;
}

.operation_card {
  display: flex;
  align-items: flex-start;
  width: 100%;
  min-height: 104px;
  gap: 14px;
  padding: 16px;
  border: 1px solid var(--nas-border);
  border-radius: 10px;
  background: var(--nas-surface);
  color: var(--nas-foreground);
  text-align: left;
  cursor: pointer;
  transition:
    border-color 0.16s ease,
    background-color 0.16s ease;

  &:hover {
    border-color: var(--accent, #3c8fbe);
    background: var(--nas-hover);
  }

  &:focus-visible {
    outline: 2px solid var(--accent, #3c8fbe);
    outline-offset: 2px;
  }

  &--selected {
    border-color: var(--accent, #3c8fbe);
    background: var(--nas-hover);
  }
}

.operation_icon {
  width: 24px;
  height: 24px;
  flex: none;
  filter: var(--app-neutral-icon-filter);
  object-fit: contain;
}

.operation_copy {
  display: grid;
  gap: 6px;
}

.operation_name {
  font-weight: 600;
}

.operation_description {
  color: var(--nas-muted);
  font-size: 0.875rem;
  line-height: 1.4;
}

.operation_note {
  margin: 0;
  padding: 12px 14px;
  border-radius: 8px;
  background: var(--nas-sunken);
  color: var(--nas-muted);
  font-size: 0.875rem;
}

</style>
