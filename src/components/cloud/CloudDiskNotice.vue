<script setup lang="ts">

import { useI18n } from '@/i18n';
import { openNasApp } from '@/services/nasShell';
import cloudIcon from '@/assets/icons/cloud.svg?url';

const { t } = useI18n();

function openCloudDisks() {
  openNasApp('nas-disk-manager', '?kind=cloud');
}

</script>

<template>
  <aside class="cloud_notice" data-testid="cloud_disk_notice">
    <img class="app_icon notice_icon" :src="cloudIcon" alt="" aria-hidden="true" />
    <div class="notice_text">
      <strong>{{ t.cloudDisks.noneTitle }}</strong>
      <p>{{ t.cloudDisks.noneDescription }}</p>
      <button
        type="button"
        class="app_button app_button--primary app_button--small"
        data-testid="cloud_disks_open"
        @click="openCloudDisks"
      >
        {{ t.cloudDisks.open }}
      </button>
    </div>
  </aside>
</template>

<style scoped lang="scss">

.cloud_notice {
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
