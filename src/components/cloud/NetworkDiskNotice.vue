<script setup lang="ts">

import { useI18n } from '@/i18n';
import { openNasApp } from '@/services/nasShell';
import cloudIcon from '@/assets/icons/cloud.svg?url';

defineProps<{ failed?: boolean }>();

const emit = defineEmits<{ retry: [] }>();

const { t } = useI18n();

function openNetworkDisks() {
  openNasApp('nas-disk-manager', '?kind=network');
}

</script>

<template>
  <div v-if="failed" class="app_error alert_error disks_failed" role="alert">
    <span>{{ t.networkDisks.loadError }}</span>
    <button
      type="button"
      class="app_button app_button--small"
      data-testid="network_disks_retry"
      @click="emit('retry')"
    >
      {{ t.retry }}
    </button>
  </div>
  <aside v-else class="network_notice" data-testid="network_disk_notice">
    <img class="app_icon notice_icon" :src="cloudIcon" alt="" aria-hidden="true" />
    <div class="notice_text">
      <strong>{{ t.networkDisks.noneTitle }}</strong>
      <p>{{ t.networkDisks.noneDescription }}</p>
      <button
        type="button"
        class="app_button app_button--primary app_button--small"
        @click="openNetworkDisks"
      >
        {{ t.networkDisks.open }}
      </button>
    </div>
  </aside>
</template>

<style scoped lang="scss">

.disks_failed {
  align-items: center;

  span {
    flex: 1;
  }
}

.network_notice {
  display: flex;
  align-items: flex-start;
  gap: 16px;
  padding: 18px;
  border: 1px solid var(--nas-border);
  border-left: 4px solid var(--accent, #3c8fbe);
  border-radius: 10px;
  background: var(--nas-surface);
}

.notice_icon {
  --app-icon-size: 28px;
}

.notice_text {
  min-width: 0;

  p {
    margin: 6px 0 14px;
    color: var(--nas-muted);
    line-height: 1.5;
  }
}

</style>
