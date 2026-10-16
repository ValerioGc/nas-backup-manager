<script setup lang="ts">

import { useI18n } from '@/i18n';
import { NasSwitch } from '@valeriogc/nas-base-app';
import type { DuplicateResolution, SyncMode } from '@/types/backup';

/** `inPlace`: the rule writes into its origin, which a mirror would make no sense of. */
defineProps<{ title: string; inPlace?: boolean }>();
const { t } = useI18n();
const duplicateResolution = defineModel<DuplicateResolution | null>('duplicateResolution', {
  required: true,
});
const verifyArchivedFiles = defineModel<boolean>('verifyArchivedFiles', { required: true });
const syncMode = defineModel<SyncMode>('syncMode', { default: 'mirror' });

</script>

<template>
  <section class="destination_options">
    <h3>{{ title }}</h3>

    <fieldset v-if="!inPlace" class="destination_group">
      <legend>{{ t.destinationOptions.syncTitle }}</legend>
      <label class="verification_option">
        <input
          v-model="syncMode"
          name="sync_mode"
          type="radio"
          value="mirror"
          data-testid="sync_mirror"
        />
        <span>
          {{ t.destinationOptions.mirror }}
          <small class="option_hint">{{ t.destinationOptions.mirrorHint }}</small>
        </span>
      </label>
      <label class="verification_option">
        <input
          v-model="syncMode"
          name="sync_mode"
          type="radio"
          value="copy"
          data-testid="sync_copy"
        />
        <span>{{ t.destinationOptions.copyOnly }}</span>
      </label>
    </fieldset>

    <fieldset class="destination_group">
      <legend>{{ t.destinationOptions.conflictTitle }}</legend>
      <label class="destination_label" for="duplicate-resolution">
        {{ t.destinationOptions.conflictLabel }}
        <select id="duplicate-resolution" v-model="duplicateResolution" class="destination_select">
          <option value="" disabled>{{ t.destinationOptions.selectResolution }}</option>
          <option value="overwrite">{{ t.destinationOptions.overwrite }}</option>
          <option value="ignore">{{ t.destinationOptions.ignore }}</option>
          <option value="copy">{{ t.destinationOptions.copy }}</option>
        </select>
      </label>
    </fieldset>

    <fieldset class="destination_group">
      <legend>{{ t.destinationOptions.finalOptions }}</legend>
      <NasSwitch
        v-model="verifyArchivedFiles"
        :label="t.destinationOptions.verifyArchivedFiles"
        testid="verify_archived_files"
      />
    </fieldset>
  </section>
</template>

<style scoped lang="scss">

.destination_options {
  display: grid;
  gap: 20px;

  h3 {
    margin: 0;
    font-size: 20px;
  }
}

.destination_group {
  @extend %option_group;
}

.option_hint {
  display: block;
  color: var(--nas-muted);
  font-size: 13px;
  font-weight: 400;
}

.destination_label {
  display: grid;
  gap: 8px;
  font-weight: 600;
}

.destination_select {
  width: 100%;
  min-height: 42px;
  padding: 10px 12px;
  border: 1px solid var(--nas-border);
  border-radius: 8px;
  background: var(--nas-surface);
  color: var(--nas-foreground);
  font: inherit;

  &:focus-visible {
    outline: 2px solid var(--accent, #316fea);
    outline-offset: 2px;
  }
}

</style>
