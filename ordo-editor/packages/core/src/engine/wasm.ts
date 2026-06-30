/**
 * WASM-backed studio↔engine conversion (single source of truth).
 *
 * The Rust `ordo-studio-format` crate, compiled to WASM, is the one authoritative
 * implementation of the studio↔engine format conversion. This module wraps it so
 * the rest of the editor can call the conversion synchronously.
 *
 * Only module *initialization* is async (the dynamic WASM import). Once `initWasm()`
 * has resolved, the conversion functions are synchronous. Call `initWasm()` once at
 * application startup (before any conversion) — see the studio/playground entrypoints.
 *
 * Replaces the hand-written TypeScript `adapter.ts` / `reverse-adapter.ts`.
 */

import type { RuleSet } from '../model';

let wasmModule: any = null;
let initPromise: Promise<void> | null = null;

/**
 * Initialize the WASM module (idempotent; safe to call concurrently). Resolves
 * once the module is ready and the conversion functions can be used synchronously.
 */
export async function initWasm(): Promise<void> {
  if (wasmModule) return;
  if (!initPromise) {
    initPromise = (async () => {
      const wasm: any = await import('@ordo-engine/wasm');
      // The default export is the init function for the `web` target.
      if (typeof wasm.default === 'function') {
        await wasm.default();
      }
      wasmModule = wasm;
    })();
  }
  await initPromise;
}

/** True once `initWasm()` has completed and conversions can run synchronously. */
export function isWasmReady(): boolean {
  return wasmModule !== null;
}

/** The initialized WASM module (for callers that need other exports like execute_ruleset). */
export function getWasm(): any {
  if (!wasmModule) {
    throw new Error('WASM module not initialized — call initWasm() first');
  }
  return wasmModule;
}

/**
 * Convert a studio-format ruleset to engine format. Requires `initWasm()` to have
 * completed. Throws if the ruleset is not convertible (e.g. missing start step).
 */
export function convertToEngineFormat(ruleset: RuleSet): any {
  return JSON.parse(getWasm().studio_to_engine_json(JSON.stringify(ruleset)));
}

/**
 * Convert an engine-format ruleset to studio format. Requires `initWasm()` to have
 * completed.
 */
export function convertFromEngineFormat(engine: any): RuleSet {
  return JSON.parse(getWasm().engine_to_studio_json(JSON.stringify(engine))) as RuleSet;
}
