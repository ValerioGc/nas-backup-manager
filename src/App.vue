<script setup lang="ts">

import { computed, ref } from 'vue';
import { NasAppLayout, NasConfirmDialog, type NasCategory } from '@valeriogc/nas-base-app';
import { useBackupRules } from '@/composables/useBackupRules';
import { useNasLauncherContext } from '@/composables/useNasLauncherContext';
import { saveBackupRule } from '@/services/backupRules';
import { errorMessage } from '@/services/errors';
import type { BackupRule, ListedRule, SectionCode } from '@/types/backup';

import LoadError from '@/components/common/LoadError.vue';
import CloudDiskNotice from '@/components/cloud/CloudDiskNotice.vue';
import CommonRuleView from '@/components/rules/CommonRuleView.vue';
import RuleList from '@/components/rules/RuleList.vue';
import cloudIcon from '@/assets/icons/cloud.svg?url';
import realtimeIcon from '@/assets/icons/realtime.svg?url';
import scheduledIcon from '@/assets/icons/scheduled.svg?url';

const { t } = useNasLauncherContext();
const { bySection, loading, loadError, acting, actionError, load, runOrStop, setEnabled, remove } =
  useBackupRules();

const sectionIcons: Record<SectionCode, string> = {
  scheduled: scheduledIcon,
  rtl: realtimeIcon,
  cloud: cloudIcon,
};

const sections = computed<NasCategory<SectionCode>[]>(() =>
  (Object.keys(sectionIcons) as SectionCode[]).map((value) => ({
    value,
    label: t.value.sections[value],
    icon: sectionIcons[value],
  })),
);
const sectionCode = ref<SectionCode>('scheduled');

const formOpen = ref(false);
const editing = ref<ListedRule | null>(null);
const ruleDirty = ref(false);
const saving = ref(false);
const saveError = ref<string | null>(null);

const pendingLeave = ref<{ section: SectionCode | null } | null>(null);

const inCloud = computed(() => sectionCode.value === 'cloud');

function openRule(rule: ListedRule | null) {
  editing.value = rule;
  formOpen.value = true;
}

function closeRule(section: SectionCode | null = null) {
  formOpen.value = false;
  editing.value = null;
  ruleDirty.value = false;
  saveError.value = null;
  if (section) {
    sectionCode.value = section;
  }
}

function leaveRule(section: SectionCode | null) {
  if (ruleDirty.value) {
    pendingLeave.value = { section };
  } else {
    closeRule(section);
  }
}

function requestSectionChange(section: SectionCode) {
  if (section === sectionCode.value || saving.value) {
    return;
  }
  if (formOpen.value) {
    leaveRule(section);
  } else {
    sectionCode.value = section;
  }
}

function requestRuleClose() {
  if (!saving.value) {
    leaveRule(null);
  }
}

function discardRule() {
  const section = pendingLeave.value?.section ?? null;
  pendingLeave.value = null;
  closeRule(section);
}

async function saveRule(rule: BackupRule) {
  if (saving.value) {
    return;
  }

  saving.value = true;
  saveError.value = null;
  try {
    await saveBackupRule(rule, editing.value !== null);
    closeRule();
    await load(true);
  } catch (error) {
    saveError.value = errorMessage(error, t.value.errors.saveRule);
  } finally {
    saving.value = false;
  }
}

</script>

<template>
  <NasAppLayout
    :model-value="sectionCode"
    :categories="sections"
    :navigation-label="t.type"
    @update:model-value="requestSectionChange"
  >
    <template #icon="{ category }">
      <img class="app_icon category_icon" :src="category.icon" alt="" aria-hidden="true" />
    </template>

    <LoadError
      v-if="loadError"
      :title="t.errors.loadTitle"
      :message="loadError"
      :action="loading ? t.loadingRules : t.retry"
      :busy="loading"
      @retry="load()"
    />
    <p v-else-if="loading" class="loading_state">
      <output>{{ t.loadingRules }}</output>
    </p>

    <CommonRuleView
      v-else-if="formOpen"
      :key="editing?.id ?? 'new'"
      :section="sectionCode"
      :initial="editing"
      :saving="saving"
      :save-error="saveError"
      @dirty-change="ruleDirty = $event"
      @save="saveRule"
      @close="requestRuleClose"
    />

    <RuleList
      v-else
      :section="sectionCode"
      :rules="bySection[sectionCode]"
      :acting="acting"
      :action-error="actionError"
      :empty-icon="sectionIcons[sectionCode]"
      :can-add="!inCloud"
      @add="openRule(null)"
      @edit="openRule"
      @run="runOrStop"
      @switch="setEnabled"
      @delete="remove"
    >
      <template v-if="inCloud" #notice>
        <CloudDiskNotice />
      </template>
    </RuleList>
  </NasAppLayout>

  <NasConfirmDialog
    v-if="pendingLeave"
    :title="t.confirmDiscardTitle"
    :message="t.confirmDiscardMessage"
    :cancel-label="t.cancel"
    :confirm-label="t.discardChanges"
    @cancel="pendingLeave = null"
    @confirm="discardRule"
  />
</template>

<style scoped lang="scss">

.loading_state {
  color: var(--nas-muted);
}

</style>
