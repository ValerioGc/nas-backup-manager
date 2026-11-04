import type { ReadableFolder, VolumeName } from '@/types/nas';

function isVolumeName(value: unknown): value is VolumeName {
  return (
    typeof value === 'object' &&
    value !== null &&
    typeof (value as VolumeName).id === 'string' &&
    typeof (value as VolumeName).name === 'string'
  );
}

export async function fetchVolumeNames(): Promise<VolumeName[]> {
  try {
    const response = await fetch('api/volumes');
    const names: unknown = response.ok ? await response.json() : [];
    return Array.isArray(names) ? names.filter(isVolumeName) : [];
  } catch {
    return [];
  }
}

/** Splits `/<disk id>/Photos/2026` into the disk name and `Photos/2026`, or keeps a path on an unknown disk. */
export function readableFolder(path: string, names: VolumeName[]): ReadableFolder {
  const [first = '', ...rest] = path.split('/').filter(Boolean);
  const volume = names.find((item) => item.id === first);
  return volume
    ? { disk: volume.name, path: rest.join('/') }
    : { disk: null, path: [first, ...rest].filter(Boolean).join('/') };
}
