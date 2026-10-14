/** A disk of the NAS: the id that starts the logical paths of its folders, and its name. */
export type VolumeName = { id: string; name: string };

/** A folder as the page shows it: the name of its disk and the path inside it. */
export type ReadableFolder = { disk: string | null; path: string };

function isVolumeName(value: unknown): value is VolumeName {
  return (
    typeof value === 'object' &&
    value !== null &&
    typeof (value as VolumeName).id === 'string' &&
    typeof (value as VolumeName).name === 'string'
  );
}

/** The names of the disks; none when the backend cannot tell, and the ids are shown instead. */
export async function fetchVolumeNames(): Promise<VolumeName[]> {
  try {
    const response = await fetch('api/volumes');
    const names: unknown = response.ok ? await response.json() : [];
    return Array.isArray(names) ? names.filter(isVolumeName) : [];
  } catch {
    return [];
  }
}

/** `/<disk id>/Photos/2026` as the name of the disk and `Photos/2026`. */
export function readableFolder(path: string, names: VolumeName[]): ReadableFolder {
  const [first = '', ...rest] = path.split('/').filter(Boolean);
  const volume = names.find((item) => item.id === first);
  // A path on no known disk is shown as it is.
  return volume
    ? { disk: volume.name, path: rest.join('/') }
    : { disk: null, path: [first, ...rest].filter(Boolean).join('/') };
}
