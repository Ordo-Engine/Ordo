/**
 * Utility functions
 * 工具函数
 */

/**
 * Generate a unique ID
 */
export function generateId(prefix = 'id'): string {
  const timestamp = Date.now().toString(36);
  const random = Math.random().toString(36).substring(2, 8);
  return `${prefix}_${timestamp}_${random}`;
}

/**
 * Deep clone an object
 */
export function deepClone<T>(obj: T): T {
  return JSON.parse(JSON.stringify(obj));
}

/**
 * Check if two objects are deeply equal
 */
export function deepEqual(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (typeof a !== typeof b) return false;
  if (a === null || b === null) return a === b;
  if (typeof a !== 'object') return a === b;

  const aObj = a as Record<string, unknown>;
  const bObj = b as Record<string, unknown>;

  const aKeys = Object.keys(aObj);
  const bKeys = Object.keys(bObj);

  if (aKeys.length !== bKeys.length) return false;

  return aKeys.every((key) => deepEqual(aObj[key], bObj[key]));
}

/**
 * Debounce a function
 */
export function debounce<T extends (...args: unknown[]) => unknown>(
  fn: T,
  delay: number
): (...args: Parameters<T>) => void {
  let timeoutId: ReturnType<typeof setTimeout> | null = null;

  return (...args: Parameters<T>) => {
    if (timeoutId) {
      clearTimeout(timeoutId);
    }
    timeoutId = setTimeout(() => {
      fn(...args);
      timeoutId = null;
    }, delay);
  };
}

/**
 * Throttle a function
 */
export function throttle<T extends (...args: unknown[]) => unknown>(
  fn: T,
  limit: number
): (...args: Parameters<T>) => void {
  let lastCall = 0;

  return (...args: Parameters<T>) => {
    const now = Date.now();
    if (now - lastCall >= limit) {
      lastCall = now;
      fn(...args);
    }
  };
}

/**
 * Create a simple history stack for undo/redo
 */
export class HistoryStack<T> {
  private stack: T[] = [];
  private currentIndex = -1;
  private maxSize: number;

  constructor(maxSize = 50) {
    this.maxSize = maxSize;
  }

  /**
   * Push a new state
   */
  push(state: T): void {
    // Remove any states after current index (clear redo stack)
    this.stack = this.stack.slice(0, this.currentIndex + 1);

    // Add new state
    this.stack.push(deepClone(state));
    this.currentIndex = this.stack.length - 1;

    // Trim if exceeds max size
    if (this.stack.length > this.maxSize) {
      this.stack.shift();
      this.currentIndex--;
    }
  }

  /**
   * Undo - go back one state
   */
  undo(): T | undefined {
    if (this.canUndo()) {
      this.currentIndex--;
      return deepClone(this.stack[this.currentIndex]);
    }
    return undefined;
  }

  /**
   * Redo - go forward one state
   */
  redo(): T | undefined {
    if (this.canRedo()) {
      this.currentIndex++;
      return deepClone(this.stack[this.currentIndex]);
    }
    return undefined;
  }

  /**
   * Check if undo is available
   */
  canUndo(): boolean {
    return this.currentIndex > 0;
  }

  /**
   * Check if redo is available
   */
  canRedo(): boolean {
    return this.currentIndex < this.stack.length - 1;
  }

  /**
   * Get current state
   */
  current(): T | undefined {
    if (this.currentIndex >= 0) {
      return deepClone(this.stack[this.currentIndex]);
    }
    return undefined;
  }

  /**
   * Clear history
   */
  clear(): void {
    this.stack = [];
    this.currentIndex = -1;
  }

  /**
   * Get history size
   */
  size(): number {
    return this.stack.length;
  }
}

/**
 * Parse a path string (e.g., "$.user.name" or "data.items[0].value")
 */
export function parsePath(path: string): string[] {
  if (!path) return [];

  // Remove leading $ or $.
  let cleanPath = path;
  if (cleanPath.startsWith('$.')) {
    cleanPath = cleanPath.slice(2);
  } else if (cleanPath.startsWith('$')) {
    cleanPath = cleanPath.slice(1);
  }

  if (!cleanPath) return [];

  // Split by . and []
  const parts: string[] = [];
  let current = '';
  let inBracket = false;

  for (const char of cleanPath) {
    if (char === '[') {
      if (current) {
        parts.push(current);
        current = '';
      }
      inBracket = true;
    } else if (char === ']') {
      if (current) {
        parts.push(current);
        current = '';
      }
      inBracket = false;
    } else if (char === '.' && !inBracket) {
      if (current) {
        parts.push(current);
        current = '';
      }
    } else {
      current += char;
    }
  }

  if (current) {
    parts.push(current);
  }

  return parts;
}

/**
 * Get value from object by path
 */
export function getByPath(obj: unknown, path: string): unknown {
  const parts = parsePath(path);
  let current: unknown = obj;

  for (const part of parts) {
    if (current === null || current === undefined) {
      return undefined;
    }
    if (typeof current !== 'object') {
      return undefined;
    }
    current = (current as Record<string, unknown>)[part];
  }

  return current;
}

/**
 * Set value in object by path
 */
export function setByPath(obj: unknown, path: string, value: unknown): unknown {
  const parts = parsePath(path);
  if (parts.length === 0) return value;

  const result = deepClone(obj) as Record<string, unknown>;
  let current: Record<string, unknown> = result;

  for (let i = 0; i < parts.length - 1; i++) {
    const part = parts[i];
    if (current[part] === undefined || current[part] === null) {
      // Create object or array based on next part
      const nextPart = parts[i + 1];
      current[part] = /^\d+$/.test(nextPart) ? [] : {};
    }
    current = current[part] as Record<string, unknown>;
  }

  current[parts[parts.length - 1]] = value;
  return result;
}

