<script setup lang="ts">

import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { useI18n } from '@/i18n';
import { NasSwitch } from '@valeriogc/nas-base-app';
import type { ArchiveConfiguration, CryptConfiguration } from '@/types/backup';

type Variant = 'archive' | 'crypt';
type InitialConfiguration = ArchiveConfiguration | CryptConfiguration;

const props = withDefaults(
  defineProps<{
    variant?: Variant;
    initialValue?: InitialConfiguration | null;
    /** The rule is saved already: an empty password keeps the one it has. */
    keepPassword?: boolean;
  }>(),
  { variant: 'archive', initialValue: null, keepPassword: false },
);

const emit = defineEmits<{
  close: [];
  save: [configuration: InitialConfiguration];
}>();

const { t } = useI18n();
const dialog = ref<HTMLDialogElement | null>(null);
const archiveInitial =
  props.variant === 'archive' && props.initialValue && 'archiveName' in props.initialValue
    ? props.initialValue
    : null;
const cryptInitial =
  props.variant === 'crypt' && props.initialValue && 'mode' in props.initialValue
    ? props.initialValue
    : null;
const archiveName = ref(archiveInitial?.archiveName ?? '');
const password = ref(archiveInitial?.password ?? cryptInitial?.password ?? '');
const confirmPassword = ref(password.value);
const encryptFileInfo = ref(archiveInitial?.encryptFileInfo ?? false);
const verifyArchive = ref(archiveInitial?.verifyArchive ?? false);
const mode = ref<CryptConfiguration['mode']>(cryptInitial?.mode ?? 'encrypt');
const output = ref<CryptConfiguration['output']>(cryptInitial?.output ?? 'single');
const showPassword = ref(false);

const modalTitle = computed(() =>
  props.variant === 'archive'
    ? t.value.operationModals.archiveTitle
    : t.value.operationModals.cryptTitle,
);
const needsConfirmation = computed(() => props.variant === 'archive' || mode.value === 'encrypt');
const passwordMismatch = computed(
  () =>
    needsConfirmation.value &&
    confirmPassword.value.length > 0 &&
    password.value !== confirmPassword.value,
);
const keepsPassword = computed(
  () => props.keepPassword && password.value === '' && confirmPassword.value === '',
);
const canSave = computed(
  () =>
    (keepsPassword.value ||
      (password.value.length > 0 &&
        (!needsConfirmation.value ||
          (confirmPassword.value.length > 0 && password.value === confirmPassword.value)))) &&
    (props.variant !== 'archive' || archiveName.value.trim().length > 0),
);

onMounted(() => {
  if (dialog.value && !dialog.value.open) {
    if (typeof dialog.value.showModal === 'function') {
      dialog.value.showModal();
    } else {
      dialog.value.setAttribute('open', '');
    }
  }
});

onBeforeUnmount(() => {
  if (dialog.value?.open) {
    if (typeof dialog.value.close === 'function') {
      dialog.value.close();
    } else {
      dialog.value.removeAttribute('open');
    }
  }
});

function save() {
  if (!canSave.value) {
    return;
  }

  if (props.variant === 'archive') {
    emit('save', {
      archiveName: archiveName.value.trim(),
      password: password.value,
      encryptFileInfo: encryptFileInfo.value,
      verifyArchive: verifyArchive.value,
    });
    return;
  }

  emit('save', { mode: mode.value, output: output.value, password: password.value });
}

</script>

<template>
  <dialog
    ref="dialog"
    class="operation_dialog"
    aria-labelledby="operation-dialog-title"
    @cancel.prevent="emit('close')"
  >
    <header class="dialog_header">
      <h2 id="operation-dialog-title">{{ modalTitle }}</h2>
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
      <label v-if="variant === 'archive'" class="dialog_field" for="archive-name">
        {{ t.operationModals.archiveName }}
        <input
          id="archive-name"
          v-model="archiveName"
          type="text"
          :placeholder="t.operationModals.archiveName"
          required
        />
      </label>

      <template v-if="variant === 'crypt'">
        <label class="dialog_field" for="crypt-mode">
          {{ t.operationModals.mode }}
          <select id="crypt-mode" v-model="mode">
            <option value="encrypt">{{ t.operationModals.encrypt }}</option>
            <option value="decrypt">{{ t.operationModals.decrypt }}</option>
          </select>
        </label>

        <label class="dialog_field" for="crypt-output">
          {{ t.operationModals.output }}
          <select id="crypt-output" v-model="output">
            <option value="single">{{ t.operationModals.outputFiles }}</option>
            <option value="folder">{{ t.operationModals.outputFolders }}</option>
            <option value="singlefile">{{ t.operationModals.outputSingleFile }}</option>
          </select>
        </label>
      </template>

      <label class="dialog_field" for="operation-password">
        {{ t.operationModals.password }}
        <input
          id="operation-password"
          v-model="password"
          :type="showPassword ? 'text' : 'password'"
          :placeholder="keepPassword ? t.operationModals.keepPassword : t.operationModals.password"
          autocomplete="new-password"
          required
        />
      </label>

      <label v-if="needsConfirmation" class="dialog_field" for="operation-password-confirmation">
        {{ t.operationModals.confirmPassword }}
        <input
          id="operation-password-confirmation"
          v-model="confirmPassword"
          :type="showPassword ? 'text' : 'password'"
          :placeholder="t.operationModals.confirmPassword"
          autocomplete="new-password"
          :aria-invalid="passwordMismatch"
          required
        />
        <span v-if="passwordMismatch" class="app_error alert_error" role="alert">
          {{ t.operationModals.passwordMismatch }}
        </span>
      </label>

      <button
        type="button"
        class="app_button app_button--small password_toggle"
        @click="showPassword = !showPassword"
      >
        {{ showPassword ? t.operationModals.hidePassword : t.operationModals.showPassword }}
      </button>

      <template v-if="variant === 'archive'">
        <NasSwitch
          v-model="encryptFileInfo"
          :label="t.operationModals.encryptFileInfo"
          testid="encrypt_file_info"
        />
        <NasSwitch
          v-model="verifyArchive"
          :label="t.operationModals.verifyArchive"
          testid="verify_archive"
        />
      </template>

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
  display: grid;
  gap: 8px;
  font-weight: 600;

  input,
  select {
    width: 100%;
    min-height: 42px;
    padding: 9px 12px;
    border: 1px solid var(--nas-border);
    border-radius: 8px;
    background: var(--nas-surface);
    color: var(--nas-foreground);
    font: inherit;
  }

  input:focus-visible,
  select:focus-visible {
    outline: 2px solid var(--accent, #3c8fbe);
    outline-offset: 2px;
  }
}

.password_toggle {
  justify-self: start;
}

.dialog_actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 8px;
}

</style>
