<script setup lang="ts">

import ArchiveModal from '@/components/rules/steps/modals/ArchiveModal.vue';
import type { ArchiveConfiguration, CryptConfiguration } from '@/types/backup';

defineProps<{ initialValue?: CryptConfiguration | null }>();

const emit = defineEmits<{
  close: [];
  save: [configuration: CryptConfiguration];
}>();

function save(configuration: ArchiveConfiguration | CryptConfiguration) {
  if ('mode' in configuration) {
    emit('save', configuration);
  }
}

</script>

<template>
  <ArchiveModal variant="crypt" :initial-value="initialValue" @close="emit('close')" @save="save" />
</template>
