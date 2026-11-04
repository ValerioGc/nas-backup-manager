import { readonly, ref } from 'vue';
import { fetchVolumeNames } from '@/services/volumes';
import type { VolumeName } from '@/types/nas';

const names = ref<VolumeName[]>([]);
let loading: Promise<void> | null = null;

export function useVolumeNames() {
  loading ??= fetchVolumeNames().then((found) => {
    names.value = found;
  });

  return { names: readonly(names), loaded: loading };
}

export function forgetVolumeNames() {
  names.value = [];
  loading = null;
}
