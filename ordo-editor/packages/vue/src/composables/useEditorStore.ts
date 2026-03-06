import { shallowRef, readonly, computed, onUnmounted } from 'vue';
import type { DeepReadonly, ShallowRef } from 'vue';
import {
  EditorStore,
  createSchemaContext,
} from '@ordo-engine/editor-core';
import type {
  EditorState,
  Command,
  SchemaContext,
} from '@ordo-engine/editor-core';
import type { RuleSet } from '@ordo-engine/editor-core';

export interface UseEditorStoreReturn {
  state: Readonly<ShallowRef<DeepReadonly<EditorState>>>;
  dispatch: (command: Command) => void;
  batch: (fn: () => void) => void;
  undo: () => boolean;
  redo: () => boolean;
  canUndo: ReturnType<typeof computed<boolean>>;
  canRedo: ReturnType<typeof computed<boolean>>;
  schemaContext: ReturnType<typeof computed<SchemaContext | null>>;
  store: EditorStore;
}

/**
 * Bridges an `EditorStore` into Vue 3 reactivity.
 *
 * The returned `state` ref is updated whenever the store notifies its listeners
 * (after dispatch, undo, redo, or replaceRuleset). The subscription is
 * automatically cleaned up when the component is unmounted.
 */
export function useEditorStore(
  initialRuleset: RuleSet,
  options?: { maxHistory?: number },
): UseEditorStoreReturn {
  const store = new EditorStore(initialRuleset, options);
  const state = shallowRef<EditorState>(store.getState() as EditorState);

  const unsub = store.subscribe((newState) => {
    state.value = newState;
  });

  onUnmounted(unsub);

  const canUndo = computed(() => store.canUndo());
  const canRedo = computed(() => store.canRedo());

  const schemaContext = computed<SchemaContext | null>(() => {
    const schema = state.value.ruleset.config.inputSchema;
    if (!schema || schema.length === 0) return null;
    return createSchemaContext(schema);
  });

  return {
    state: readonly(state) as Readonly<ShallowRef<DeepReadonly<EditorState>>>,
    dispatch: store.dispatch.bind(store),
    batch: store.batch.bind(store),
    undo: store.undo.bind(store),
    redo: store.redo.bind(store),
    canUndo,
    canRedo,
    schemaContext,
    store,
  };
}
