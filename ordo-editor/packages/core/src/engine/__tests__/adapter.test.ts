/**
 * Adapter tests
 * 格式转换器测试
 */

import { describe, it, expect } from 'vitest';
import { convertToEngineFormat, validateEngineCompatibility, isEngineCompatible } from '../adapter';
import type { RuleSet, DecisionStep, ActionStep, TerminalStep } from '../../model';

describe('Format Adapter', () => {
  describe('convertToEngineFormat', () => {
    it('should convert a simple ruleset', () => {
      const editorRuleset: RuleSet = {
        config: {
          name: 'test-rule',
          version: '1.0.0',
          description: 'Test ruleset',
        },
        startStepId: 'start',
        steps: [
          {
            id: 'start',
            name: 'Start Step',
            type: 'terminal',
            code: 'SUCCESS',
          } as TerminalStep,
        ],
      };

      const engineRuleset = convertToEngineFormat(editorRuleset);

      expect(engineRuleset.config.name).toBe('test-rule');
      expect(engineRuleset.config.entry_step).toBe('start');
      expect(engineRuleset.steps['start']).toBeDefined();
      expect(engineRuleset.steps['start'].kind).toHaveProperty('Terminal');
    });

    it('should convert decision step with branches', () => {
      const editorRuleset: RuleSet = {
        config: { name: 'decision-test' },
        startStepId: 'decide',
        steps: [
          {
            id: 'decide',
            name: 'Decision',
            type: 'decision',
            branches: [
              {
                id: 'branch1',
                condition: { type: 'simple', field: 'age', operator: '>', value: 18 },
                nextStepId: 'adult',
              },
            ],
            defaultNextStepId: 'minor',
          } as DecisionStep,
          {
            id: 'adult',
            name: 'Adult',
            type: 'terminal',
            code: 'ADULT',
          } as TerminalStep,
          {
            id: 'minor',
            name: 'Minor',
            type: 'terminal',
            code: 'MINOR',
          } as TerminalStep,
        ],
      };

      const engineRuleset = convertToEngineFormat(editorRuleset);
      const decisionStep = engineRuleset.steps['decide'];

      expect(decisionStep.kind).toHaveProperty('Decision');
      if ('Decision' in decisionStep.kind) {
        expect(decisionStep.kind.Decision.branches).toHaveLength(1);
        expect(decisionStep.kind.Decision.default_next).toBe('minor');
      }
    });

    it('should convert action step with assignments', () => {
      const editorRuleset: RuleSet = {
        config: { name: 'action-test' },
        startStepId: 'action',
        steps: [
          {
            id: 'action',
            name: 'Action',
            type: 'action',
            assignments: [
              { name: 'result', value: { type: 'literal', value: 'done' } },
            ],
            nextStepId: 'end',
          } as ActionStep,
          {
            id: 'end',
            name: 'End',
            type: 'terminal',
            code: 'DONE',
          } as TerminalStep,
        ],
      };

      const engineRuleset = convertToEngineFormat(editorRuleset);
      const actionStep = engineRuleset.steps['action'];

      expect(actionStep.kind).toHaveProperty('Action');
      if ('Action' in actionStep.kind) {
        expect(actionStep.kind.Action.actions).toHaveLength(1);
        expect(actionStep.kind.Action.next_step).toBe('end');
      }
    });
  });

  describe('validateEngineCompatibility', () => {
    it('should pass validation for valid ruleset', () => {
      const ruleset: RuleSet = {
        config: { name: 'valid' },
        startStepId: 'start',
        steps: [
          {
            id: 'start',
            name: 'Start',
            type: 'terminal',
            code: 'OK',
          } as TerminalStep,
        ],
      };

      const errors = validateEngineCompatibility(ruleset);
      expect(errors).toHaveLength(0);
      expect(isEngineCompatible(ruleset)).toBe(true);
    });

    it('should detect missing startStepId', () => {
      const ruleset: RuleSet = {
        config: { name: 'invalid' },
        startStepId: '',
        steps: [],
      };

      const errors = validateEngineCompatibility(ruleset);
      expect(errors.length).toBeGreaterThan(0);
      expect(errors[0]).toContain('startStepId');
    });

    it('should detect missing step IDs', () => {
      const ruleset: RuleSet = {
        config: { name: 'invalid' },
        startStepId: 'start',
        steps: [
          {
            id: '',
            name: 'No ID',
            type: 'terminal',
            code: 'FAIL',
          } as TerminalStep,
        ],
      };

      const errors = validateEngineCompatibility(ruleset);
      expect(errors.length).toBeGreaterThan(0);
      expect(errors.some((e) => e.includes('missing id'))).toBe(true);
    });

    it('should detect non-existent step references', () => {
      const ruleset: RuleSet = {
        config: { name: 'invalid' },
        startStepId: 'start',
        steps: [
          {
            id: 'start',
            name: 'Start',
            type: 'decision',
            branches: [
              {
                id: 'branch1',
                condition: { type: 'simple', field: 'x', operator: '==', value: 1 },
                nextStepId: 'nonexistent',
              },
            ],
            defaultNextStepId: 'also-nonexistent',
          } as DecisionStep,
        ],
      };

      const errors = validateEngineCompatibility(ruleset);
      expect(errors.length).toBeGreaterThan(0);
      expect(errors.some((e) => e.includes('non-existent'))).toBe(true);
    });
  });
});

