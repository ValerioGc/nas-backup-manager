<script setup lang="ts">

import { computed, ref } from 'vue';
import { useI18n } from '@/i18n';
import OperationModal from '@/components/rules/steps/modals/OperationModal.vue';
import type { BatchRenameConfiguration } from '@/types/backup';

const props = defineProps<{ initialValue?: BatchRenameConfiguration | null }>();

const emit = defineEmits<{
  close: [];
  save: [configuration: BatchRenameConfiguration];
}>();

const { t } = useI18n();
const filename = ref(props.initialValue?.filename ?? '');
const separator = ref<BatchRenameConfiguration['separator']>(props.initialValue?.separator ?? '-');
const prefix = ref(props.initialValue?.prefix ?? '');
const position = ref<BatchRenameConfiguration['position']>(
  props.initialValue?.position ?? 'prefix',
);

const exampleName = computed(() => {
  const base = filename.value.trim() || t.value.operationModals.exampleFileName;
  const addition = prefix.value.trim();
  if (!addition) {
    return base;
  }
  if (position.value === 'prefix') {
    return `${addition}${separator.value}${base}`;
  }
  const dot = base.lastIndexOf('.');
  return dot > 0
    ? `${base.slice(0, dot)}${separator.value}${addition}${base.slice(dot)}`
    : `${base}${separator.value}${addition}`;
});
const canSave = computed(() => filename.value.trim().length > 0 && prefix.value.trim().length > 0);

function save() {
  emit('save', {
    filename: filename.value.trim(),
    separator: separator.value,
    prefix: prefix.value.trim(),
    position: position.value,
  });
}

</script>

<template>
  <OperationModal
    :title="t.operationModals.renameTitle"
    :can-save="canSave"
    @close="emit('close')"
    @save="save"
  >
    <label class="dialog_field" for="rename-filename">
      {{ t.operationModals.filename }}
      <input id="rename-filename" v-model="filename" type="text" required />
    </label>

    <label class="dialog_field" for="rename-prefix">
      {{ t.operationModals.renameText }}
      <input id="rename-prefix" v-model="prefix" type="text" required />
    </label>

    <label class="dialog_field" for="rename-separator">
      {{ t.operationModals.separator }}
      <select id="rename-separator" v-model="separator">
        <option value="-">-</option>
        <option value="_">_</option>
      </select>
    </label>

    <label class="dialog_field" for="rename-position">
      {{ t.operationModals.position }}
      <select id="rename-position" v-model="position">
        <option value="prefix">{{ t.operationModals.before }}</option>
        <option value="suffix">{{ t.operationModals.after }}</option>
      </select>
    </label>

    <p class="rename_preview">
      <strong>{{ t.operationModals.preview }}:</strong>
      <code>{{ exampleName }}</code>
    </p>
  </OperationModal>
</template>

<style scoped lang="scss">

.dialog_field {
  @extend %dialog_field;
}

.rename_preview {
  display: grid;
  gap: 6px;
  margin: 0;
  padding: 12px 14px;
  border-radius: 8px;
  background: var(--nas-sunken);

  code {
    overflow-wrap: anywhere;
    color: var(--nas-muted);
  }
}

</style>
