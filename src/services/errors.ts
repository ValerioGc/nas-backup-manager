/** Message of a thrown error, or `fallback` when it is not an `Error`. */
export function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error ? error.message : fallback;
}
