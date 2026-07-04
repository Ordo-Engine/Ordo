/**
 * Share-by-URL — serialize the active file into a compressed URL fragment so a
 * ruleset (or decision table) can be shared with a single link. The whole
 * payload lives in `location.hash`, so nothing hits a server.
 */
import { compressToEncodedURIComponent, decompressFromEncodedURIComponent } from 'lz-string';
import type { RuleSet, DecisionTable, DocumentType } from '@ordo-engine/editor-core';

const HASH_PREFIX = '#share=';
/** Guard against links that most browsers/servers would truncate (~2000 char URLs). */
const MAX_ENCODED_LENGTH = 16000;

export interface SharePayload {
  /** Schema version, so an older link can be rejected cleanly if the shape changes. */
  v: 1;
  name: string;
  documentType: DocumentType;
  ruleset: RuleSet;
  /** Present only for decision-table documents. */
  decisionTable?: DecisionTable;
}

/** Encode a payload into the compact string that goes after `#share=`. */
export function encodeShare(payload: SharePayload): string | null {
  const encoded = compressToEncodedURIComponent(JSON.stringify(payload));
  if (encoded.length > MAX_ENCODED_LENGTH) return null;
  return encoded;
}

/** Decode a `#share=…` hash back into a payload, or null if absent/malformed. */
export function decodeShare(hash: string): SharePayload | null {
  if (!hash.startsWith(HASH_PREFIX)) return null;
  try {
    const json = decompressFromEncodedURIComponent(hash.slice(HASH_PREFIX.length));
    if (!json) return null;
    const payload = JSON.parse(json) as SharePayload;
    if (payload?.v !== 1 || !payload.ruleset || !payload.documentType) return null;
    return payload;
  } catch {
    return null;
  }
}

/** Build a full, shareable URL for the given payload (null if too large to encode). */
export function buildShareUrl(payload: SharePayload): string | null {
  const encoded = encodeShare(payload);
  if (encoded === null) return null;
  const { origin, pathname } = window.location;
  return `${origin}${pathname}${HASH_PREFIX}${encoded}`;
}
