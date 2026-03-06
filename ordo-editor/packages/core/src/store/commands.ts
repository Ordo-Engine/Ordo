import type { EditorState } from './editor-store';
import type { Step, Branch, DecisionStep, ActionStep } from '../model/step';
import type { RuleSetConfig, SchemaField } from '../model/ruleset';
import { isDecisionStep } from '../model/step';
import { generateId, deepClone } from '../utils';

// ---------------------------------------------------------------------------
// Command interface
// ---------------------------------------------------------------------------

export interface Command {
  readonly type: string;
  execute(state: EditorState): EditorState;
  describe(): string;
}

// ---------------------------------------------------------------------------
// Step commands
// ---------------------------------------------------------------------------

export class AddStepCommand implements Command {
  readonly type = 'AddStep';
  constructor(
    private step: Step,
    private setAsStart = false,
  ) {}

  execute(state: EditorState): EditorState {
    const ruleset = {
      ...state.ruleset,
      steps: [...state.ruleset.steps, deepClone(this.step)],
      startStepId:
        this.setAsStart || !state.ruleset.startStepId
          ? this.step.id
          : state.ruleset.startStepId,
    };
    return { ...state, ruleset };
  }

  describe(): string {
    return `Add step "${this.step.name}"`;
  }
}

export class RemoveStepCommand implements Command {
  readonly type = 'RemoveStep';
  constructor(private stepId: string) {}

  execute(state: EditorState): EditorState {
    const steps = state.ruleset.steps
      .filter((s) => s.id !== this.stepId)
      .map((s) => RemoveStepCommand.cleanReferences(s, this.stepId));

    const ruleset = {
      ...state.ruleset,
      steps,
      startStepId:
        state.ruleset.startStepId === this.stepId
          ? ''
          : state.ruleset.startStepId,
    };

    return {
      ...state,
      ruleset,
      selectedStepId:
        state.selectedStepId === this.stepId ? null : state.selectedStepId,
      selectedBranchId:
        state.selectedStepId === this.stepId ? null : state.selectedBranchId,
    };
  }

  /** Remove dangling references in other steps pointing to the deleted step */
  private static cleanReferences(step: Step, removedId: string): Step {
    if (step.type === 'decision') {
      const d = step as DecisionStep;
      return {
        ...d,
        branches: d.branches.map((b) =>
          b.nextStepId === removedId ? { ...b, nextStepId: '' } : b,
        ),
        defaultNextStepId:
          d.defaultNextStepId === removedId ? '' : d.defaultNextStepId,
      };
    }
    if (step.type === 'action') {
      const a = step as ActionStep;
      return {
        ...a,
        nextStepId: a.nextStepId === removedId ? '' : a.nextStepId,
      };
    }
    return step;
  }

  describe(): string {
    return `Remove step "${this.stepId}"`;
  }
}

export class UpdateStepCommand implements Command {
  readonly type = 'UpdateStep';
  constructor(
    private stepId: string,
    private updates: Partial<Step>,
  ) {}

  execute(state: EditorState): EditorState {
    const steps = state.ruleset.steps.map((s) =>
      s.id === this.stepId ? ({ ...s, ...this.updates, id: s.id, type: s.type } as Step) : s,
    );
    return { ...state, ruleset: { ...state.ruleset, steps } };
  }

  describe(): string {
    return `Update step "${this.stepId}"`;
  }
}

export class MoveStepCommand implements Command {
  readonly type = 'MoveStep';
  constructor(
    private stepId: string,
    private position: { x: number; y: number },
  ) {}

  execute(state: EditorState): EditorState {
    const steps = state.ruleset.steps.map((s) =>
      s.id === this.stepId ? { ...s, position: { ...this.position } } : s,
    );
    return { ...state, ruleset: { ...state.ruleset, steps } };
  }

  describe(): string {
    return `Move step "${this.stepId}"`;
  }
}

// ---------------------------------------------------------------------------
// Branch commands
// ---------------------------------------------------------------------------

export class AddBranchCommand implements Command {
  readonly type = 'AddBranch';
  constructor(
    private stepId: string,
    private branch: Branch,
  ) {}

  execute(state: EditorState): EditorState {
    const steps = state.ruleset.steps.map((s) => {
      if (s.id === this.stepId && isDecisionStep(s)) {
        return {
          ...s,
          branches: [...s.branches, deepClone(this.branch)],
        };
      }
      return s;
    });
    return { ...state, ruleset: { ...state.ruleset, steps } };
  }

  describe(): string {
    return `Add branch to step "${this.stepId}"`;
  }
}

export class RemoveBranchCommand implements Command {
  readonly type = 'RemoveBranch';
  constructor(
    private stepId: string,
    private branchId: string,
  ) {}

  execute(state: EditorState): EditorState {
    const steps = state.ruleset.steps.map((s) => {
      if (s.id === this.stepId && isDecisionStep(s)) {
        return {
          ...s,
          branches: s.branches.filter((b) => b.id !== this.branchId),
        };
      }
      return s;
    });
    return {
      ...state,
      ruleset: { ...state.ruleset, steps },
      selectedBranchId:
        state.selectedBranchId === this.branchId ? null : state.selectedBranchId,
    };
  }

  describe(): string {
    return `Remove branch "${this.branchId}" from step "${this.stepId}"`;
  }
}

export class UpdateBranchCommand implements Command {
  readonly type = 'UpdateBranch';
  constructor(
    private stepId: string,
    private branchId: string,
    private updates: Partial<Pick<Branch, 'label' | 'condition' | 'nextStepId'>>,
  ) {}

  execute(state: EditorState): EditorState {
    const steps = state.ruleset.steps.map((s) => {
      if (s.id === this.stepId && isDecisionStep(s)) {
        return {
          ...s,
          branches: s.branches.map((b) =>
            b.id === this.branchId ? { ...b, ...this.updates } : b,
          ),
        };
      }
      return s;
    });
    return { ...state, ruleset: { ...state.ruleset, steps } };
  }

  describe(): string {
    return `Update branch "${this.branchId}"`;
  }
}

export class ReorderBranchCommand implements Command {
  readonly type = 'ReorderBranch';
  constructor(
    private stepId: string,
    private branchId: string,
    private direction: 'up' | 'down',
  ) {}

  execute(state: EditorState): EditorState {
    const steps = state.ruleset.steps.map((s) => {
      if (s.id === this.stepId && isDecisionStep(s)) {
        const branches = [...s.branches];
        const idx = branches.findIndex((b) => b.id === this.branchId);
        if (idx === -1) return s;
        const targetIdx = this.direction === 'up' ? idx - 1 : idx + 1;
        if (targetIdx < 0 || targetIdx >= branches.length) return s;
        [branches[idx], branches[targetIdx]] = [branches[targetIdx], branches[idx]];
        return { ...s, branches };
      }
      return s;
    });
    return { ...state, ruleset: { ...state.ruleset, steps } };
  }

  describe(): string {
    return `Reorder branch "${this.branchId}" ${this.direction}`;
  }
}

// ---------------------------------------------------------------------------
// Connection commands
// ---------------------------------------------------------------------------

export class ConnectStepsCommand implements Command {
  readonly type = 'ConnectSteps';
  constructor(
    private fromStepId: string,
    private toStepId: string,
    private branchId?: string,
  ) {}

  execute(state: EditorState): EditorState {
    const steps = state.ruleset.steps.map((s) => {
      if (s.id !== this.fromStepId) return s;

      if (s.type === 'decision') {
        if (this.branchId) {
          return {
            ...s,
            branches: s.branches.map((b) =>
              b.id === this.branchId ? { ...b, nextStepId: this.toStepId } : b,
            ),
          };
        }
        return { ...s, defaultNextStepId: this.toStepId };
      }

      if (s.type === 'action') {
        return { ...s, nextStepId: this.toStepId };
      }

      return s;
    });
    return { ...state, ruleset: { ...state.ruleset, steps } };
  }

  describe(): string {
    return `Connect "${this.fromStepId}" → "${this.toStepId}"`;
  }
}

export class DisconnectStepsCommand implements Command {
  readonly type = 'DisconnectSteps';
  constructor(
    private fromStepId: string,
    private toStepId: string,
    private branchId?: string,
  ) {}

  execute(state: EditorState): EditorState {
    const steps = state.ruleset.steps.map((s) => {
      if (s.id !== this.fromStepId) return s;

      if (s.type === 'decision') {
        if (this.branchId) {
          return {
            ...s,
            branches: s.branches.map((b) =>
              b.id === this.branchId && b.nextStepId === this.toStepId
                ? { ...b, nextStepId: '' }
                : b,
            ),
          };
        }
        if (s.defaultNextStepId === this.toStepId) {
          return { ...s, defaultNextStepId: '' };
        }
      }

      if (s.type === 'action' && s.nextStepId === this.toStepId) {
        return { ...s, nextStepId: '' };
      }

      return s;
    });
    return { ...state, ruleset: { ...state.ruleset, steps } };
  }

  describe(): string {
    return `Disconnect "${this.fromStepId}" → "${this.toStepId}"`;
  }
}

// ---------------------------------------------------------------------------
// Start step
// ---------------------------------------------------------------------------

export class SetStartStepCommand implements Command {
  readonly type = 'SetStartStep';
  constructor(private stepId: string) {}

  execute(state: EditorState): EditorState {
    return {
      ...state,
      ruleset: { ...state.ruleset, startStepId: this.stepId },
    };
  }

  describe(): string {
    return `Set start step to "${this.stepId}"`;
  }
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

export class UpdateConfigCommand implements Command {
  readonly type = 'UpdateConfig';
  constructor(private updates: Partial<RuleSetConfig>) {}

  execute(state: EditorState): EditorState {
    return {
      ...state,
      ruleset: {
        ...state.ruleset,
        config: { ...state.ruleset.config, ...this.updates },
      },
    };
  }

  describe(): string {
    const keys = Object.keys(this.updates).join(', ');
    return `Update config: ${keys}`;
  }
}

// ---------------------------------------------------------------------------
// Schema
// ---------------------------------------------------------------------------

export class SetSchemaCommand implements Command {
  readonly type = 'SetSchema';
  constructor(
    private inputSchema?: SchemaField[],
    private outputSchema?: SchemaField[],
  ) {}

  execute(state: EditorState): EditorState {
    const config = { ...state.ruleset.config };
    if (this.inputSchema !== undefined) config.inputSchema = this.inputSchema;
    if (this.outputSchema !== undefined) config.outputSchema = this.outputSchema;
    return {
      ...state,
      ruleset: { ...state.ruleset, config },
    };
  }

  describe(): string {
    return 'Set schema';
  }
}

// ---------------------------------------------------------------------------
// Batch
// ---------------------------------------------------------------------------

export class BatchCommand implements Command {
  readonly type = 'Batch';
  constructor(
    private commands: Command[],
    private label?: string,
  ) {}

  execute(state: EditorState): EditorState {
    return this.commands.reduce(
      (s, cmd) => cmd.execute(s),
      state,
    );
  }

  describe(): string {
    return this.label ?? `Batch (${this.commands.length} commands)`;
  }
}

// ---------------------------------------------------------------------------
// Clipboard: paste step with new IDs
// ---------------------------------------------------------------------------

export class PasteStepCommand implements Command {
  readonly type = 'PasteStep';
  constructor(
    private step: Step,
    private offset?: { x: number; y: number },
  ) {}

  execute(state: EditorState): EditorState {
    const newStep = deepClone(this.step);
    newStep.id = generateId('step');
    newStep.name = `${newStep.name} (copy)`;
    if (newStep.position && this.offset) {
      newStep.position = {
        x: newStep.position.x + (this.offset.x ?? 40),
        y: newStep.position.y + (this.offset.y ?? 40),
      };
    }
    if (newStep.type === 'decision') {
      for (const branch of newStep.branches) {
        branch.id = generateId('branch');
      }
    }

    const ruleset = {
      ...state.ruleset,
      steps: [...state.ruleset.steps, newStep],
    };
    return { ...state, ruleset, selectedStepId: newStep.id };
  }

  describe(): string {
    return `Paste step "${this.step.name}"`;
  }
}

// ---------------------------------------------------------------------------
// Selection (non-mutating convenience — still goes through history so undo
// restores selection state too)
// ---------------------------------------------------------------------------

export class SelectStepCommand implements Command {
  readonly type = 'SelectStep';
  constructor(
    private stepId: string | null,
    private branchId: string | null = null,
  ) {}

  execute(state: EditorState): EditorState {
    return {
      ...state,
      selectedStepId: this.stepId,
      selectedBranchId: this.branchId,
    };
  }

  describe(): string {
    return this.stepId ? `Select step "${this.stepId}"` : 'Deselect step';
  }
}

// ---------------------------------------------------------------------------
// Import decision table (replaces steps from a table compilation result)
// ---------------------------------------------------------------------------

export class ImportDecisionTableCommand implements Command {
  readonly type = 'ImportDecisionTable';
  constructor(
    private steps: Step[],
    private startStepId: string,
  ) {}

  execute(state: EditorState): EditorState {
    return {
      ...state,
      ruleset: {
        ...state.ruleset,
        steps: deepClone(this.steps),
        startStepId: this.startStepId,
      },
      selectedStepId: null,
      selectedBranchId: null,
    };
  }

  describe(): string {
    return 'Import decision table';
  }
}
