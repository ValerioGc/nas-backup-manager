import { ref } from 'vue';
import { requestNetworkDisks } from '@/services/nasShell';
import type { NetworkDisk } from '@valeriogc/nas-base-app';

/** The network shares configured in the disk manager, read through the launcher on demand. */
export function useNetworkDisks() {
  const disks = ref<NetworkDisk[]>([]);
  const loaded = ref(false);
  const loading = ref(false);
  const failed = ref(false);
  let version = 0;

  async function load() {
    const current = ++version;
    loading.value = true;
    const result = await requestNetworkDisks();
    if (current !== version) {
      return;
    }

    disks.value = result.disks;
    failed.value = result.error !== undefined;
    loaded.value = true;
    loading.value = false;
  }

  return { disks, loaded, loading, failed, load };
}
