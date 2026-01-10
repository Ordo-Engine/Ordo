/**
 * Event bus for editor components communication
 * 编辑器组件间通信的事件总线
 */

import { RuleSet, Step, Condition, Expr } from '../model';
import { ValidationError } from '../validator';

/** Event types */
export type EditorEventType =
  // RuleSet events
  | 'ruleset:load'
  | 'ruleset:save'
  | 'ruleset:change'
  | 'ruleset:validate'
  // Step events
  | 'step:add'
  | 'step:remove'
  | 'step:update'
  | 'step:select'
  | 'step:deselect'
  | 'step:move'
  // Branch events
  | 'branch:add'
  | 'branch:remove'
  | 'branch:update'
  // Connection events
  | 'connection:add'
  | 'connection:remove'
  // Editor state events
  | 'editor:mode-change'
  | 'editor:zoom'
  | 'editor:pan'
  | 'editor:undo'
  | 'editor:redo'
  // Validation events
  | 'validation:start'
  | 'validation:complete'
  | 'validation:error';

/** Event payloads */
export interface EditorEventPayloads {
  'ruleset:load': { ruleset: RuleSet };
  'ruleset:save': { ruleset: RuleSet };
  'ruleset:change': { ruleset: RuleSet; previousRuleset: RuleSet };
  'ruleset:validate': { ruleset: RuleSet };

  'step:add': { step: Step; ruleset: RuleSet };
  'step:remove': { stepId: string; ruleset: RuleSet };
  'step:update': { step: Step; previousStep: Step; ruleset: RuleSet };
  'step:select': { stepId: string };
  'step:deselect': { stepId: string };
  'step:move': { stepId: string; position: { x: number; y: number } };

  'branch:add': { stepId: string; branch: { id: string; condition: Condition; nextStepId: string } };
  'branch:remove': { stepId: string; branchId: string };
  'branch:update': { stepId: string; branchId: string; branch: { condition?: Condition; nextStepId?: string } };

  'connection:add': { fromStepId: string; toStepId: string; branchId?: string };
  'connection:remove': { fromStepId: string; toStepId: string; branchId?: string };

  'editor:mode-change': { mode: 'flow' | 'form' };
  'editor:zoom': { zoom: number };
  'editor:pan': { x: number; y: number };
  'editor:undo': { historyIndex: number };
  'editor:redo': { historyIndex: number };

  'validation:start': { ruleset: RuleSet };
  'validation:complete': { valid: boolean; errors: ValidationError[] };
  'validation:error': { error: Error };
}

/** Event handler type */
export type EditorEventHandler<T extends EditorEventType> = (
  payload: EditorEventPayloads[T]
) => void;

/** Event subscription */
export interface EventSubscription {
  unsubscribe: () => void;
}

/**
 * Event bus for editor components
 */
export class EditorEventBus {
  private handlers: Map<EditorEventType, Set<EditorEventHandler<EditorEventType>>> = new Map();

  /**
   * Subscribe to an event
   */
  on<T extends EditorEventType>(
    event: T,
    handler: EditorEventHandler<T>
  ): EventSubscription {
    if (!this.handlers.has(event)) {
      this.handlers.set(event, new Set());
    }
    this.handlers.get(event)!.add(handler as EditorEventHandler<EditorEventType>);

    return {
      unsubscribe: () => this.off(event, handler),
    };
  }

  /**
   * Subscribe to an event (one-time)
   */
  once<T extends EditorEventType>(
    event: T,
    handler: EditorEventHandler<T>
  ): EventSubscription {
    const wrappedHandler: EditorEventHandler<T> = (payload) => {
      this.off(event, wrappedHandler);
      handler(payload);
    };
    return this.on(event, wrappedHandler);
  }

  /**
   * Unsubscribe from an event
   */
  off<T extends EditorEventType>(
    event: T,
    handler: EditorEventHandler<T>
  ): void {
    const handlers = this.handlers.get(event);
    if (handlers) {
      handlers.delete(handler as EditorEventHandler<EditorEventType>);
    }
  }

  /**
   * Emit an event
   */
  emit<T extends EditorEventType>(event: T, payload: EditorEventPayloads[T]): void {
    const handlers = this.handlers.get(event);
    if (handlers) {
      handlers.forEach((handler) => {
        try {
          handler(payload);
        } catch (error) {
          console.error(`Error in event handler for "${event}":`, error);
        }
      });
    }
  }

  /**
   * Remove all handlers for an event
   */
  removeAllListeners(event?: EditorEventType): void {
    if (event) {
      this.handlers.delete(event);
    } else {
      this.handlers.clear();
    }
  }

  /**
   * Get the number of listeners for an event
   */
  listenerCount(event: EditorEventType): number {
    return this.handlers.get(event)?.size ?? 0;
  }
}

/** Global event bus instance */
let globalEventBus: EditorEventBus | null = null;

/**
 * Get the global event bus instance
 */
export function getEventBus(): EditorEventBus {
  if (!globalEventBus) {
    globalEventBus = new EditorEventBus();
  }
  return globalEventBus;
}

/**
 * Create a new event bus instance
 */
export function createEventBus(): EditorEventBus {
  return new EditorEventBus();
}

// Fix unused import warning
export type { Expr };

