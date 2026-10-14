<script setup lang="ts">

import { computed, ref, useId } from 'vue';
import { useI18n } from '@/i18n';
import { requiredFolders } from '@/services/backupRules';
import { pickNasFolder } from '@/services/nasShell';
import { readableFolder } from '@/services/volumes';
import { useVolumeNames } from '@/composables/useVolumeNames';
import NetworkDiskPicker from '@/components/cloud/NetworkDiskPicker.vue';
import { NasSwitch } from '@valeriogc/nas-base-app';
import type { FolderKind, RuleFolders, SectionCode } from '@/types/backup';
import arrowRight from '@/assets/icons/arrow-right.svg?url';
import diskIcon from '@/assets/icons/disk.svg?url';
import folderIcon from '@/assets/icons/folder.svg?url';

const props = defineProps<{
  title: string;
  type: SectionCode;
}>();

const folders = defineModel<RuleFolders>('folders', { required: true });
const name = defineModel<string>('name', { required: true });
const sameDestination = defineModel<boolean>('sameDestination', { default: false });
const nameId = useId();

const { t } = useI18n();
const { names } = useVolumeNames();
const picking = ref(false);
const pickerFailed = ref<FolderKind | null>(null);

// A real-time rule watches its origin.
const folderFields = computed(() =>
  requiredFolders(props.type, sameDestination.value).map((kind) => ({
    kind,
    label:
      props.type === 'rtl' && kind === 'origin' ? t.value.folders.watched : t.value.folders[kind],
  })),
);

/** The folder as the person knows it: the name of its disk, not its id, and the path in it. */
function shown(kind: FolderKind) {
  const path = folders.value[kind];
  return path === null ? null : readableFolder(path, [...names.value]);
}

function setFolder(kind: FolderKind, path: string | null) {
  folders.value = { ...folders.value, [kind]: path };
}

async function pick(kind: FolderKind) {
  picking.value = true;
  pickerFailed.value = null;
  const result = await pickNasFolder(folders.value[kind] ?? undefined);
  picking.value = false;
  const [folder] = result.files;
  if (folder) {
    setFolder(kind, folder.path);
  } else if (result.error) {
    pickerFailed.value = kind;
  }
}

</script>

<template>
  <section class="folder_step">
    <h3 class="folder_title">{{ title }}</h3>

    <label class="rule_name" :for="nameId">
      {{ t.folders.name }}
      <input
        :id="nameId"
        v-model="name"
        name="rule_name"
        type="text"
        maxlength="80"
        required
        :placeholder="t.folders.namePlaceholder"
        data-testid="rule_name"
      />
    </label>

    <div class="folder_grid" :class="{ 'folder_grid--pair': folderFields.length === 2 }">
      <template v-for="(field, index) in folderFields" :key="field.kind">
        <img
          v-if="index > 0"
          class="app_icon folder_arrow"
          :src="arrowRight"
          alt=""
          aria-hidden="true"
        />
        <article class="folder_card" :class="{ 'folder_card--chosen': folders[field.kind] }">
          <header class="folder_head">
            <img class="app_icon" :src="folderIcon" alt="" aria-hidden="true" />
            <h4>{{ field.label }}</h4>
          </header>
          <NetworkDiskPicker
            v-if="field.kind === 'cloud'"
            :model-value="folders.cloud"
            @update:model-value="setFolder('cloud', $event)"
          />
          <template v-else>
            <div class="folder_selection">
              <p
                v-if="shown(field.kind)"
                class="folder_location"
                :title="folders[field.kind] ?? undefined"
              >
                <span v-if="shown(field.kind)?.disk" class="folder_disk">
                  <img class="app_icon" :src="diskIcon" alt="" aria-hidden="true" />
                  {{ shown(field.kind)?.disk }}
                </span>
                <span class="folder_path">/{{ shown(field.kind)?.path }}</span>
              </p>
              <p v-else class="folder_empty">{{ t.folders.notSelected }}</p>
              <button
                type="button"
                class="app_button app_button--small"
                :aria-label="`${folders[field.kind] ? t.folders.change : t.folders.select}: ${field.label}`"
                :disabled="picking"
                @click="pick(field.kind)"
              >
                {{ folders[field.kind] ? t.folders.change : t.folders.select }}
              </button>
            </div>
            <p v-if="pickerFailed === field.kind" class="app_error alert_error" role="alert">
              {{ t.folders.pickerError }}
            </p>
          </template>
        </article>
      </template>
    </div>

    <div v-if="type === 'rtl'" class="same_destination">
      <NasSwitch
        v-model="sameDestination"
        :label="t.folders.sameDestination"
        testid="same_destination"
      />
      <small>{{ t.folders.sameDestinationHint }}</small>
    </div>
  </section>
</template>

<style scoped lang="scss">

.folder_step {
  display: grid;
  gap: 20px;
  min-width: 0;
}

.folder_title {
  margin: 0;
  font-size: 20px;
}

.rule_name {
  display: grid;
  gap: 8px;
  max-width: 480px;
  font-weight: 600;

  input {
    min-height: 42px;
    padding: 10px 12px;
    border: 1px solid var(--nas-border);
    border-radius: 8px;
    background: var(--nas-surface);
    color: var(--nas-foreground);
    font: inherit;
    font-weight: 400;
  }
}

.folder_grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  gap: 12px;

  &--pair {
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    align-items: center;
  }
}

.folder_arrow {
  --app-icon-size: 20px;
  opacity: 0.6;
}

.folder_card {
  display: grid;
  align-content: start;
  gap: 14px;
  min-width: 0;
  height: 100%;
  padding: 18px;
  border: 1px solid var(--nas-border);
  border-radius: 10px;
  background: var(--nas-surface);
  transition: border-color 0.16s ease;

  &--chosen {
    border-color: color-mix(in srgb, var(--accent, #3c8fbe) 45%, var(--nas-border));
  }
}

.folder_head {
  --app-icon-size: 18px;
  display: flex;
  align-items: center;
  gap: 8px;

  h4 {
    margin: 0;
    font-size: 15px;
  }
}

.folder_selection {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  min-width: 0;
}

.folder_location {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
  min-width: 0;
  margin: 0;
}

.folder_disk {
  --app-icon-size: 14px;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 3px 9px;
  border-radius: 999px;
  background: var(--nas-hover);
  font-size: 13px;
  font-weight: 600;
  white-space: nowrap;
}

.folder_path {
  min-width: 0;
  color: var(--nas-muted);
  font-size: 14px;
  overflow-wrap: anywhere;
}

.folder_empty {
  margin: 0;
  color: var(--nas-muted);
  font-size: 14px;
}

.same_destination {
  display: grid;
  gap: 6px;
  padding: 14px 16px;
  border: 1px solid var(--nas-border);
  border-radius: 10px;
  background: var(--nas-surface);

  small {
    display: block;
    color: var(--nas-muted);
    font-size: 13px;
    font-weight: 400;
  }
}

@media (max-width: 760px) {
  .folder_grid--pair {
    grid-template-columns: minmax(0, 1fr);
  }

  .folder_arrow {
    justify-self: center;
    transform: rotate(90deg);
  }
}

</style>
