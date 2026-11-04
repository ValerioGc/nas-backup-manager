<script setup lang="ts">

import { computed, ref } from 'vue';
import { useI18n } from '@/i18n';
import { NasSwitch } from '@valeriogc/nas-base-app';
import OperationModal from '@/components/rules/steps/modals/OperationModal.vue';
import type { ArchiveConfiguration } from '@/types/backup';

const props = withDefaults(
  defineProps<{
    initialValue?: ArchiveConfiguration | null;
    keepPassword?: boolean;
  }>(),
  { initialValue: null, keepPassword: false },
);

const emit = defineEmits<{
  close: [];
  save: [configuration: ArchiveConfiguration];
}>();

const { t } = useI18n();
const archiveName = ref(props.initialValue?.archiveName ?? '');
const password = ref(props.initialValue?.password ?? '');
const confirmPassword = ref(password.value);
const encryptFileInfo = ref(props.initialValue?.encryptFileInfo ?? false);
const verifyArchive = ref(props.initialValue?.verifyArchive ?? false);
const showPassword = ref(false);

const passwordMismatch = computed(
  () => confirmPassword.value.length > 0 && password.value !== confirmPassword.value,
);
const keepsPassword = computed(
  () => props.keepPassword && password.value === '' && confirmPassword.value === '',
);
const canSave = computed(
  () =>
    (keepsPassword.value ||
      (password.value.length > 0 && password.value === confirmPassword.value)) &&
    archiveName.value.trim().length > 0,
);

function save() {
  emit('save', {
    archiveName: archiveName.value.trim(),
    password: password.value,
    encryptFileInfo: encryptFileInfo.value,
    verifyArchive: verifyArchive.value,
  });
}

</script>

<template>
  <OperationModal
    :title="t.operationModals.archiveTitle"
    :can-save="canSave"
    @close="emit('close')"
    @save="save"
  >
    <label class="dialog_field" for="archive-name">
      {{ t.operationModals.archiveName }}
      <input
        id="archive-name"
        v-model="archiveName"
        type="text"
        :placeholder="t.operationModals.archiveName"
        required
      />
    </label>

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

    <label class="dialog_field" for="operation-password-confirmation">
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
  </OperationModal>
</template>

<style scoped lang="scss">

.dialog_field {
  @extend %dialog_field;
}

.password_toggle {
  justify-self: start;
}

</style>
