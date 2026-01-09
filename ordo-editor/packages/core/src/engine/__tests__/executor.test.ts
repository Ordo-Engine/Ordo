/**
 * Executor tests
 * 执行器测试
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { RuleExecutor } from '../executor';
import type { RuleSet, TerminalStep } from '../../model';

// Mock the WASM module
vi.mock('@ordo/wasm/dist/ordo_wasm', () => ({
  default: vi.fn().mockResolvedValue(undefined),
  execute_ruleset: vi.fn(),
  validate_ruleset: vi.fn(),
  eval_expression: vi.fn(),
}));

describe('RuleExecutor', () => {
  let executor: RuleExecutor;
  let mockWasm: any;

  beforeEach(async () => {
    executor = new RuleExecutor();
    
    // Get mock module
    mockWasm = await import('@ordo/wasm/dist/ordo_wasm');
  });

  describe('WASM Execution', () => {
    it('should execute a simple ruleset via WASM', async () => {
      const ruleset: RuleSet = {
        config: { name: 'test' },
        startStepId: 'start',
        steps: [
          {
            id: 'start',
            name: 'Start',
            type: 'terminal',
            code: 'SUCCESS',
          } as TerminalStep,
        ],
      };

      const input = { user: 'test' };

      // Mock WASM response
      mockWasm.execute_ruleset.mockReturnValue(
        JSON.stringify({
          code: 'SUCCESS',
          message: 'Execution completed',
          output: {},
          duration_us: 100,
        })
      );

      const result = await executor.execute(ruleset, input, { mode: 'wasm' });

      expect(result.code).toBe('SUCCESS');
      expect(result.duration_us).toBe(100);
      expect(mockWasm.execute_ruleset).toHaveBeenCalled();
    });

    it('should handle WASM execution errors', async () => {
      const ruleset: RuleSet = {
        config: { name: 'test' },
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

      mockWasm.execute_ruleset.mockImplementation(() => {
        throw new Error('WASM error');
      });

      await expect(
        executor.execute(ruleset, {}, { mode: 'wasm' })
      ).rejects.toThrow('WASM execution failed');
    });
  });

  describe('Validation', () => {
    it('should validate via WASM', async () => {
      const ruleset: RuleSet = {
        config: { name: 'test' },
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

      mockWasm.validate_ruleset.mockReturnValue(
        JSON.stringify({ valid: true })
      );

      const result = await executor.validate(ruleset, { mode: 'wasm' });

      expect(result.valid).toBe(true);
      expect(mockWasm.validate_ruleset).toHaveBeenCalled();
    });

    it('should detect compatibility errors before WASM validation', async () => {
      const ruleset: RuleSet = {
        config: { name: 'invalid' },
        startStepId: '',
        steps: [],
      };

      const result = await executor.validate(ruleset, { mode: 'wasm' });

      expect(result.valid).toBe(false);
      expect(result.errors).toBeDefined();
      expect(result.errors!.length).toBeGreaterThan(0);
    });
  });

  describe('Expression Evaluation', () => {
    it('should evaluate expressions via WASM', async () => {
      mockWasm.eval_expression.mockReturnValue(
        JSON.stringify({
          result: 42,
          parsed: 'Binary(+, 40, 2)',
        })
      );

      const result = await executor.evalExpression('40 + 2', {}, { mode: 'wasm' });

      expect(result.result).toBe(42);
      expect(mockWasm.eval_expression).toHaveBeenCalled();
    });
  });
});

