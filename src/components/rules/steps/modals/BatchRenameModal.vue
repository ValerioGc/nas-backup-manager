<script setup lang="ts">

import { computed, ref } from 'vue';
import { vNasModal } from '@valeriogc/nas-base-app';
import { useI18n } from '@/i18n';
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
  // After the name, before its extension: the file keeps opening as it did.
  const dot = base.lastIndexOf('.');
  return dot > 0
    ? `${base.slice(0, dot)}${separator.value}${addition}${base.slice(dot)}`
    : `${base}${separator.value}${addition}`;
});
const canSave = computed(() => filename.value.trim().length > 0 && prefix.value.trim().length > 0);

function save() {
  if (!canSave.value) {
    return;
  }

  emit('save', {
    filename: filename.value.trim(),
    separator: separator.value,
    prefix: prefix.value.trim(),
    position: position.value,
  });
}

</script>

<template>
  <dialog
    v-nas-modal
    class="operation_dialog"
    aria-labelledby="rename-dialog-title"
    @cancel.prevent="emit('close')"
  >
    <header class="dialog_header">
      <h2 id="rename-dialog-title">{{ t.operationModals.renameTitle }}</h2>
      <button
        type="button"
        class="app_button app_button--icon"
        :aria-label="t.close"
        @click="emit('close')"
      >
        <img class="app_icon" src="@/assets/icons/close.svg" alt="" aria-hidden="true" />
      </button>
    </header>

    <form class="dialog_form" @submit.prevent="save">
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

      <footer class="dialog_actions">
        <button type="button" class="app_button" @click="emit('close')">{{ t.cancel }}</button>
        <button type="submit" class="app_button app_button--primary" :disabled="!canSave">
          {{ t.save }}
        </button>
      </footer>
    </form>
  </dialog>
</template>

<style scoped lang="scss">

.operation_dialog {
  width: min(560px, calc(100% - 32px));
  max-width: 100%;
  max-height: min(90vh, 760px);
  overflow: auto;
  padding: 24px;
  border: 1px solid var(--nas-border);
  border-radius: 12px;
  background: var(--nas-surface);
  color: var(--nas-foreground);
  box-shadow: 0 16px 48px #0003;

  &::backdrop {
    background: #0008;
  }
}

.dialog_header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 20px;

  h2 {
    margin: 0;
    font-size: 21px;
  }
}

.dialog_form {
  display: grid;
  gap: 16px;
}

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

.dialog_actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 8px;
}

</style>
