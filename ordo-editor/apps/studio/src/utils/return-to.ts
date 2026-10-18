import type { LocationQuery } from 'vue-router';

/**
 * Reads a `returnTo` query value that points back into the app.
 * Only in-app paths are accepted so the query can't redirect elsewhere.
 */
export function readReturnTo(query: LocationQuery): string | null {
  const value = query.returnTo;
  if (typeof value !== 'string') return null;
  return value.startsWith('/orgs/') ? value : null;
}
