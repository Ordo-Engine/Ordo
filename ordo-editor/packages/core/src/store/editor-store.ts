import type { RuleSet } from '../model/ruleset';
import type { Step } from '../model/step';
import type { SchemaContext } from './schema-context';
import type { Command } from './commands';
import { HistoryManager } from './history-manager';
import { deepClone } from '../utils';

export interface EditorState {
  ruleset: RuleSet;
  selectedStepId: string | null;
  selectedBranchId: string | null;
  clipboard: Step | null;
  schemaContext: SchemaContext | null;
}

export type EditorListener = (state: EditorState) => void;

/**
 * Routes commands to the store and maintains a log of dispatched commands.
 * Exists as a separate concern so middleware (logging, validation) can be
 * inserted between dispatch and execution.
 */
export class CommandBus {
  private middlewares: Array<(cmd: Command, next: () => void) => void> = [];

  use(middleware: (cmd: Command, next: () => void) => void): void {
    this.middlewares.push(middleware);
  }

  dispatch(command: Command, executor: (cmd: Command) => void): void {
    const chain = [...this.middlewares];
    let index = 0;

    const next = (): void => {
      if (index < chain.length) {
        chain[index++](command, next);
      } else {
        executor(command);
      }
    };

    next();
  }
}

/**
 * Central state container for the rule editor.
 * Every mutation goes through dispatch(command), enabling undo/redo,
 * batching, and full testability without any UI framework dependency.
 */
export class EditorStore {
  private state: EditorState;
  private history: HistoryManager;
  private commandBus: CommandBus;
  private listeners: Set<EditorListener> = new Set();
  private batchDepth = 0;
  private batchCommands: Command[] = [];

  constructor(initialRuleset: RuleSet, options?: { maxHistory?: number }) {
    this.state = {
      ruleset: deepClone(initialRuleset),
      selectedStepId: null,
      selectedBranchId: null,
      clipboard: null,
      schemaContext: null,
    };
    this.history = new HistoryManager(options?.maxHistory ?? 100);
    this.history.push(this.state);
    this.commandBus = new CommandBus();
  }

  getState(): Readonly<EditorState> {
    return this.state;
  }

  getCommandBus(): CommandBus {
    return this.commandBus;
  }

  dispatch(command: Command): void {
    this.commandBus.dispatch(command, (cmd) => {
      if (this.batchDepth > 0) {
        this.batchCommands.push(cmd);
        this.state = cmd.execute(this.state);
        return;
      }

      this.state = cmd.execute(this.state);
      this.history.push(this.state);
      this.notify();
    });
  }

  /**
   * Group multiple dispatches into a single undo unit.
   * All commands dispatched inside `fn` are recorded but only one
   * history snapshot is taken after `fn` completes.
   */
  batch(fn: () => void): void {
    this.batchDepth++;
    this.batchCommands = [];
    try {
      fn();
    } finally {
      this.batchDepth--;
      if (this.batchDepth === 0 && this.batchCommands.length > 0) {
        this.history.push(this.state);
        this.batchCommands = [];
        this.notify();
      }
    }
  }

  subscribe(listener: EditorListener): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  undo(): boolean {
    const prev = this.history.undo();
    if (prev) {
      this.state = prev;
      this.notify();
      return true;
    }
    return false;
  }

  redo(): boolean {
    const next = this.history.redo();
    if (next) {
      this.state = next;
      this.notify();
      return true;
    }
    return false;
  }

  canUndo(): boolean {
    return this.history.canUndo();
  }

  canRedo(): boolean {
    return this.history.canRedo();
  }

  /**
   * Replace the entire ruleset (e.g. on external load).
   * Pushes a new history entry.
   */
  replaceRuleset(ruleset: RuleSet): void {
    this.state = {
      ...this.state,
      ruleset: deepClone(ruleset),
      selectedStepId: null,
      selectedBranchId: null,
    };
    this.history.push(this.state);
    this.notify();
  }

  private notify(): void {
    for (const listener of this.listeners) {
      try {
        listener(this.state);
      } catch (err) {
        console.error('[EditorStore] listener error:', err);
      }
    }
  }
}
