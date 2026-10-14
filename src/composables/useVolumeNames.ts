import { readonly, ref } from 'vue';
import { fetchVolumeNames, type VolumeName } from '@/services/volumes';

// Read once for the whole page: the disks do not change while a rule is being written.
const names = ref<VolumeName[]>([]);
let loading: Promise<void> | null = null;

export function useVolumeNames() {
  loading ??= fetchVolumeNames().then((found) => {
    names.value = found;
  });
  return { names: readonly(names), loaded: loading };
}

/** Forgets what was read, so that the next page reads the disks again (tests). */
export function forgetVolumeNames() {
  names.value = [];
  loading = null;
}
