<script setup lang="ts">

import { onMounted } from 'vue';
import { useI18n } from '@/i18n';
import { useNetworkDisks } from '@/composables/useNetworkDisks';
import NetworkDiskNotice from '@/components/cloud/NetworkDiskNotice.vue';

const selected = defineModel<string | null>({ required: true });

const { t } = useI18n();
const { disks, loaded, loading, failed, load } = useNetworkDisks();

onMounted(load);

</script>

<template>
  <output v-if="!loaded || loading" class="disk_status">
    {{ t.networkDisks.loading }}
  </output>
  <div v-else-if="failed" class="disk_status" role="alert">
    <span class="app_error alert_error" role="alert">{{ t.networkDisks.loadError }}</span>
    <button type="button" class="app_button app_button--small" @click="load">
      {{ t.retry }}
    </button>
  </div>
  <NetworkDiskNotice v-else-if="disks.length === 0" />
  <fieldset v-else class="disk_list" :aria-label="t.networkDisks.label">
    <label
      v-for="disk in disks"
      :key="disk.mount"
      class="disk_option"
      :class="{ disk_option_disabled: disk.readOnly }"
    >
      <input
        v-model="selected"
        type="radio"
        name="network-disk"
        :value="disk.mount"
        :disabled="disk.readOnly"
      />
      <span class="disk_text">
        <strong>{{ disk.name }}</strong>
        <small>{{ disk.source ?? disk.mount }}</small>
      </span>
      <span v-if="disk.readOnly" class="disk_badge">{{ t.networkDisks.readOnly }}</span>
    </label>
  </fieldset>
</template>

<style scoped lang="scss">

.disk_status {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 12px;
  margin: 0;
  color: var(--nas-muted);
}

.disk_list {
  display: grid;
  gap: 8px;
  min-width: 0;
  margin: 0;
  padding: 0;
  border: 0;
}

.disk_option {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  border: 1px solid var(--nas-border);
  border-radius: 8px;
  cursor: pointer;

  &:has(input:checked) {
    border-color: var(--accent, #3c8fbe);
    background: var(--accent-soft);
  }
}

.disk_option_disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

.disk_text {
  display: grid;
  min-width: 0;

  small {
    overflow: hidden;
    color: var(--nas-muted);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
}

.disk_badge {
  margin-left: auto;
  color: var(--nas-muted);
  font-size: 0.75rem;
}

</style>
