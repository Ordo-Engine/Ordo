import type { EditorState } from './editor-store';
import { deepClone } from '../utils';

/**
 * Manages undo/redo history using state snapshots.
 * Wraps the snapshotting logic so EditorStore doesn't have to.
 */
export class HistoryManager {
  private stack: EditorState[] = [];
  private currentIndex = -1;
  private maxSize: number;

  constructor(maxSize = 100) {
    this.maxSize = maxSize;
  }

  push(state: EditorState): void {
    this.stack = this.stack.slice(0, this.currentIndex + 1);
    this.stack.push(deepClone(state));
    this.currentIndex = this.stack.length - 1;

    if (this.stack.length > this.maxSize) {
      this.stack.shift();
      this.currentIndex--;
    }
  }

  undo(): EditorState | undefined {
    if (!this.canUndo()) return undefined;
    this.currentIndex--;
    return deepClone(this.stack[this.currentIndex]);
  }

  redo(): EditorState | undefined {
    if (!this.canRedo()) return undefined;
    this.currentIndex++;
    return deepClone(this.stack[this.currentIndex]);
  }

  canUndo(): boolean {
    return this.currentIndex > 0;
  }

  canRedo(): boolean {
    return this.currentIndex < this.stack.length - 1;
  }

  current(): EditorState | undefined {
    if (this.currentIndex >= 0) {
      return deepClone(this.stack[this.currentIndex]);
    }
    return undefined;
  }

  clear(): void {
    this.stack = [];
    this.currentIndex = -1;
  }

  size(): number {
    return this.stack.length;
  }

  undoCount(): number {
    return this.currentIndex;
  }

  redoCount(): number {
    return this.stack.length - 1 - this.currentIndex;
  }
}
