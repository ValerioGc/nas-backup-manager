import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue';
import { useI18n } from '@/i18n';
import {
  deleteBackupRule,
  fetchBackupRules,
  runBackupRule,
  stopBackupRule,
  switchBackupRule,
} from '@/services/backupRules';
import { errorMessage } from '@/services/errors';
import type { ListedRule, SectionCode } from '@/types/backup';

// Refresh interval of the list while a rule runs and the page is visible.
const RUNNING_REFRESH_MS = 3000;

/** Rules grouped by section with their row actions, refreshed while any of them runs. */
export function useBackupRules() {
  const { t } = useI18n();
  const bySection = reactive<Record<SectionCode, ListedRule[]>>({
    scheduled: [],
    cloud: [],
    rtl: [],
  });

  const loading = ref(true);
  const loadError = ref<string | null>(null);
  const acting = ref<string[]>([]);
  const actionError = ref<string | null>(null);

  /** Loads the rules. With `quietly` the list stays visible without the loading state. */
  async function load(quietly = false) {
    if (!quietly) {
      loading.value = true;
    }
    try {
      const rules = await fetchBackupRules();
      for (const section of Object.keys(bySection) as SectionCode[]) {
        bySection[section] = rules.filter((rule) => rule.type === section);
      }
      loadError.value = null;
    } catch (error) {
      if (!quietly) {
        loadError.value = errorMessage(error, t.value.errors.loadRules);
      }
    } finally {
      loading.value = false;
    }
  }

  async function act(rule: ListedRule, action: (id: string) => Promise<void>) {
    if (acting.value.includes(rule.id)) {
      return;
    }

    acting.value = [...acting.value, rule.id];
    actionError.value = null;
    try {
      await action(rule.id);
    } catch (error) {
      actionError.value = errorMessage(error, t.value.errors.action);
    } finally {
      acting.value = acting.value.filter((id) => id !== rule.id);
    }
    await load(true);
  }

  const runOrStop = (rule: ListedRule) => act(rule, rule.running ? stopBackupRule : runBackupRule);
  const setEnabled = (rule: ListedRule, enabled: boolean) =>
    act(rule, (id) => switchBackupRule(id, enabled));
  const remove = (rule: ListedRule) => act(rule, deleteBackupRule);

  const anyRunning = computed(() =>
    Object.values(bySection).some((rules) => rules.some((rule) => rule.running)),
  );
  let refresh: ReturnType<typeof setInterval> | undefined;

  watch(anyRunning, (running) => {
    clearInterval(refresh);
    refresh = running
      ? setInterval(() => {
          if (document.visibilityState === 'visible') {
            void load(true);
          }
        }, RUNNING_REFRESH_MS)
      : undefined;
  });

  onBeforeUnmount(() => clearInterval(refresh));
  onMounted(() => load());

  return {
    bySection,
    loading,
    loadError,
    acting,
    actionError,
    load,
    runOrStop,
    setEnabled,
    remove,
  };
}
