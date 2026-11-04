<script setup lang="ts">

import { useId } from 'vue';
import { vNasModal } from '@valeriogc/nas-base-app';
import { useI18n } from '@/i18n';
import closeIcon from '@/assets/icons/close.svg?url';

defineProps<{ title: string; canSave: boolean }>();
const emit = defineEmits<{ close: []; save: [] }>();

const { t } = useI18n();
const titleId = useId();

</script>

<template>
  <dialog
    v-nas-modal
    class="operation_dialog"
    :aria-labelledby="titleId"
    @cancel.prevent="emit('close')"
  >
    <header class="dialog_header">
      <h2 :id="titleId">{{ title }}</h2>
      <button
        type="button"
        class="app_button app_button--icon"
        :aria-label="t.close"
        @click="emit('close')"
      >
        <img class="app_icon" :src="closeIcon" alt="" aria-hidden="true" />
      </button>
    </header>

    <form class="dialog_form" @submit.prevent="canSave && emit('save')">
      <slot></slot>

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
  @extend %fx_between;
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

.dialog_actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 8px;
}

</style>
