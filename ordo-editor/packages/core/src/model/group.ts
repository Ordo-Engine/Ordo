/**
 * Step group model for visual organization
 * 步骤分组模型，用于可视化组织
 */

/** Group definition for organizing steps visually */
export interface StepGroup {
  /** Unique group ID */
  id: string;
  /** Display name */
  name: string;
  /** Optional description */
  description?: string;
  /** Group color (hex or CSS color) */
  color?: string;
  /** Position in the flow editor */
  position: {
    x: number;
    y: number;
  };
  /** Size of the group box */
  size: {
    width: number;
    height: number;
  };
  /** IDs of steps contained in this group */
  stepIds: string[];
  /** Whether the group is collapsed */
  collapsed?: boolean;
}

// ============================================================================
// Group builder helpers
// ============================================================================

let groupIdCounter = 0;

function generateGroupId(): string {
  return `group_${++groupIdCounter}`;
}

export const StepGroup = {
  /** Reset ID counter (for testing) */
  resetIdCounter() {
    groupIdCounter = 0;
  },

  /** Create a new step group */
  create(options: {
    id?: string;
    name: string;
    description?: string;
    color?: string;
    position?: { x: number; y: number };
    size?: { width: number; height: number };
    stepIds?: string[];
    collapsed?: boolean;
  }): StepGroup {
    return {
      id: options.id || generateGroupId(),
      name: options.name,
      description: options.description,
      color: options.color || '#3c3c3c',
      position: options.position || { x: 0, y: 0 },
      size: options.size || { width: 300, height: 200 },
      stepIds: options.stepIds || [],
      collapsed: options.collapsed,
    };
  },

  /** Add a step to a group */
  addStep(group: StepGroup, stepId: string): StepGroup {
    if (group.stepIds.includes(stepId)) {
      return group;
    }
    return {
      ...group,
      stepIds: [...group.stepIds, stepId],
    };
  },

  /** Remove a step from a group */
  removeStep(group: StepGroup, stepId: string): StepGroup {
    return {
      ...group,
      stepIds: group.stepIds.filter((id) => id !== stepId),
    };
  },

  /** Check if a step is in a group */
  hasStep(group: StepGroup, stepId: string): boolean {
    return group.stepIds.includes(stepId);
  },

  /** Toggle collapsed state */
  toggleCollapsed(group: StepGroup): StepGroup {
    return {
      ...group,
      collapsed: !group.collapsed,
    };
  },
};

/** Predefined group colors */
export const GROUP_COLORS = {
  blue: '#1e3a5f',
  green: '#1e4d2b',
  orange: '#4d3319',
  purple: '#3d1e5f',
  red: '#4d1e1e',
  gray: '#3c3c3c',
} as const;

export type GroupColor = keyof typeof GROUP_COLORS;

