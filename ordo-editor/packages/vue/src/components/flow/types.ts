/**
 * Flow editor type definitions
 * 流程编辑器类型定义
 */

/** Pin type - execution flow or data */
export type PinType = 'exec' | 'data';

/** Pin direction */
export type PinDirection = 'input' | 'output';

/** Pin definition */
export interface Pin {
  /** Unique pin ID */
  id: string;
  /** Pin type */
  type: PinType;
  /** Pin direction */
  direction: PinDirection;
  /** Display label (for branches) */
  label?: string;
  /** Condition expression (for tooltip) */
  condition?: string;
  /** Data type (for data pins) */
  dataType?: string;
  /** Whether this is a default branch */
  isDefault?: boolean;
  /** Value expression (for data pins) */
  value?: string;
}

/** Pin colors */
export const PIN_COLORS = {
  // Execution flow
  execInput: '#cccccc',      // White/light gray
  execDefault: '#666666',    // Gray (default branch)
  execBranch: '#b76e00',     // Orange (conditional branch)
  
  // Data flow
  dataPin: '#4a9eff',        // Blue
  
  // Hover states
  execHover: '#ffffff',
  dataHover: '#6bb3ff',
} as const;

/** Pin sizes */
export const PIN_SIZES = {
  exec: { width: 10, height: 10 },
  data: { width: 8, height: 8 },
} as const;

/** Edge/connection colors */
export const EDGE_COLORS = {
  exec: '#888888',           // Gray for execution flow
  execBranch: '#b76e00',     // Orange for conditional branches
  data: '#4a9eff',           // Blue for data flow
  selected: '#ffffff',       // White when selected
} as const;

