import { computed } from 'vue';
import { useI18n } from '@/i18n';

/** Formats Unix milliseconds as a short date and time in the current language. */
export function useDateTime() {
  const { locale } = useI18n();
  const formatter = computed(
    () => new Intl.DateTimeFormat(locale.value, { dateStyle: 'short', timeStyle: 'short' }),
  );
  return (at: number) => formatter.value.format(at);
}
