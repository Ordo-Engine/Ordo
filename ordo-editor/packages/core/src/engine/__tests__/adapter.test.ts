import { describe, expect, it } from 'vitest';
import { convertToEngineFormat, isEngineCompatible, validateEngineCompatibility } from '../adapter';
import { convertFromEngineFormat } from '../reverse-adapter';
import {
  Expr,
  type ActionStep,
  type DecisionStep,
  type RuleSet,
  type TerminalStep,
} from '../../model';

describe('Format Adapter', () => {
  describe('convertToEngineFormat', () => {
    it('converts terminal steps into flattened engine format', () => {
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
      expect(engineRuleset.steps['start']).toMatchObject({
        id: 'start',
        name: 'Start Step',
        type: 'terminal',
        result: {
          code: 'SUCCESS',
          message: '',
          output: [],
          data: null,
        },
      });
    });

    it('converts decision steps with branches', () => {
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
                condition: {
                  type: 'simple',
                  left: Expr.variable('$.age'),
                  operator: 'gt',
                  right: Expr.number(18),
                },
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

      const decisionStep = convertToEngineFormat(editorRuleset).steps['decide'];

      expect(decisionStep).toMatchObject({
        id: 'decide',
        type: 'decision',
        default_next: 'minor',
      });
      expect(decisionStep.branches).toEqual([
        {
          condition: 'age > 18',
          next_step: 'adult',
          actions: [],
        },
      ]);
    });

    it('converts action steps with assignments and external calls', () => {
      const editorRuleset: RuleSet = {
        config: { name: 'action-test' },
        startStepId: 'action',
        steps: [
          {
            id: 'action',
            name: 'Action',
            type: 'action',
            assignments: [{ name: 'result', value: Expr.string('done') }],
            externalCalls: [
              {
                type: 'http',
                target: 'PATCH https://api.example.com/score',
                params: {
                  applicantId: Expr.variable('$.applicant.id'),
                  score: Expr.number(720),
                },
                resultVariable: 'http_result',
                timeout: 1500,
              },
              {
                type: 'function',
                target: 'demo.echo#echo',
                params: {
                  payload: Expr.object({
                    amount: Expr.variable('$.amount'),
                    approved: Expr.boolean(true),
                  }),
                },
                resultVariable: 'echo_result',
              },
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

      const actionStep = convertToEngineFormat(editorRuleset).steps['action'];

      expect(actionStep).toMatchObject({
        id: 'action',
        type: 'action',
        next_step: 'end',
      });
      expect(actionStep.actions).toEqual([
        {
          action: 'set_variable',
          name: 'result',
          value: { Literal: 'done' },
          description: '',
        },
        {
          action: 'external_call',
          service: 'network.http',
          method: 'patch',
          params: [
            ['url', { Literal: 'https://api.example.com/score' }],
            [
              'json_body',
              {
                Object: [
                  ['applicantId', { Field: 'applicant.id' }],
                  ['score', { Literal: 720 }],
                ],
              },
            ],
          ],
          result_variable: 'http_result',
          timeout_ms: 1500,
          description: '',
        },
        {
          action: 'external_call',
          service: 'demo.echo',
          method: 'echo',
          params: [
            [
              'payload',
              {
                Object: [
                  ['amount', { Field: 'amount' }],
                  ['approved', { Literal: true }],
                ],
              },
            ],
          ],
          result_variable: 'echo_result',
          timeout_ms: 0,
          description: '',
        },
      ]);
    });
  });

  describe('convertFromEngineFormat', () => {
    it('reconstructs external calls from engine action steps', () => {
      const editorRuleset = convertFromEngineFormat({
        config: {
          name: 'reverse-test',
          version: '1.0.0',
          description: '',
          entry_step: 'start',
        },
        steps: {
          start: {
            id: 'start',
            name: 'Start',
            type: 'action',
            next_step: 'done',
            actions: [
              {
                action: 'external_call',
                service: 'network.http',
                method: 'get',
                params: [['url', { Literal: 'https://api.example.com/users' }]],
                result_variable: 'users',
                timeout_ms: 2000,
              },
              {
                action: 'external_call',
                service: 'demo.echo',
                method: 'echo',
                params: [['payload', { Field: 'input.payload' }]],
                result_variable: 'echo',
              },
            ],
          },
          done: {
            id: 'done',
            name: 'Done',
            type: 'terminal',
            result: {
              code: 'OK',
              message: '',
              output: [],
              data: null,
            },
          },
        },
      });

      const actionStep = editorRuleset.steps[0] as ActionStep;
      expect(actionStep.externalCalls).toEqual([
        {
          type: 'http',
          target: 'GET https://api.example.com/users',
          params: undefined,
          resultVariable: 'users',
          timeout: 2000,
        },
        {
          type: 'function',
          target: 'demo.echo#echo',
          params: {
            payload: { type: 'variable', path: 'input.payload' },
          },
          resultVariable: 'echo',
          timeout: undefined,
        },
      ]);
    });
  });

  describe('validateEngineCompatibility', () => {
    it('passes validation for a valid ruleset', () => {
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

    it('detects missing startStepId', () => {
      const ruleset: RuleSet = {
        config: { name: 'invalid' },
        startStepId: '',
        steps: [],
      };

      const errors = validateEngineCompatibility(ruleset);
      expect(errors.length).toBeGreaterThan(0);
      expect(errors[0]).toContain('startStepId');
    });

    it('detects missing step IDs', () => {
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

    it('detects non-existent step references', () => {
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
                condition: {
                  type: 'simple',
                  left: Expr.variable('$.x'),
                  operator: 'eq',
                  right: Expr.number(1),
                },
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
