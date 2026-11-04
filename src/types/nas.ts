/** NAS disk with the id that starts the logical paths of its folders, and its name. */
export type VolumeName = { id: string; name: string };

/** Folder as shown on the page, with the name of its disk and the path inside it. */
export type ReadableFolder = { disk: string | null; path: string };

export type PowerState = 'loading' | 'unavailable' | 'alwaysOn' | 'ready';
