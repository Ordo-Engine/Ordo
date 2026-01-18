/**
 * Rule Executor - Multi-mode execution (WASM + HTTP + JIT)
 * 规则执行器 - 多模式执行（WASM + HTTP + JIT）
 */

import type { RuleSet, JITSchema, JITRulesetAnalysis } from '../model';
import type { ExecutionResult, ValidationResult, EvalResult } from './types';
import { convertToEngineFormat, validateEngineCompatibility } from './adapter';

/** Execution options */
export interface ExecutionOptions {
  /** Execution mode: wasm (local), http (server), jit (server with JIT compilation) */
  mode?: 'wasm' | 'http' | 'jit';
  /** HTTP endpoint (for HTTP and JIT modes) */
  httpEndpoint?: string;
  /** Whether to include execution trace */
  includeTrace?: boolean;
  /** Execution timeout in milliseconds */
  timeout?: number;
  /** JIT Schema for JIT mode */
  jitSchema?: JITSchema;
}

/** Expression evaluation options */
export interface EvalOptions {
  /** Execution mode */
  mode?: 'wasm' | 'http';
  /** HTTP endpoint (for HTTP mode) */
  httpEndpoint?: string;
}

/** JIT execution result with performance metrics */
export interface JITExecutionResult extends ExecutionResult {
  /** JIT-specific metrics */
  jit_metrics?: {
    /** Whether JIT compilation was used */
    jit_enabled: boolean;
    /** Number of JIT-compiled expressions */
    compiled_expressions: number;
    /** JIT compilation time in microseconds */
    compilation_time_us: number;
    /** Estimated speedup factor */
    speedup_factor: number;
  };
}

/**
 * Rule Executor
 * 规则执行器
 */
export class RuleExecutor {
  private wasmModule: any = null;
  private wasmInitialized = false;

  /**
   * Initialize WASM module (lazy loading)
   * 初始化 WASM 模块（懒加载）
   */
  async initWasm(): Promise<void> {
    if (this.wasmInitialized) return;

    try {
      // Dynamic import of WASM module
      const wasm = await import('@ordo-engine/wasm');
      // Initialize WASM - the default export is the init function
      if (typeof wasm.default === 'function') {
        await wasm.default();
      }
      this.wasmModule = wasm;
      this.wasmInitialized = true;
    } catch (error) {
      throw new Error(
        `Failed to initialize WASM module: ${
          error instanceof Error ? error.message : 'Unknown error'
        }`
      );
    }
  }

  /**
   * Execute a ruleset
   * 执行规则集
   */
  async execute(
    ruleset: RuleSet,
    input: Record<string, any>,
    options: ExecutionOptions = {}
  ): Promise<ExecutionResult> {
    // Convert to engine format
    const engineRuleset = convertToEngineFormat(ruleset);

    // Validate compatibility
    const errors = validateEngineCompatibility(ruleset);
    if (errors.length > 0) {
      throw new Error(`Compatibility errors:\n${errors.join('\n')}`);
    }

    // Choose execution mode
    const mode = options.mode || 'wasm';
    if (mode === 'jit') {
      return this.executeViaJit(engineRuleset, input, options);
    } else if (mode === 'http' || options.httpEndpoint) {
      return this.executeViaHttp(engineRuleset, input, options);
    } else {
      return this.executeViaWasm(engineRuleset, input, options);
    }
  }

  /**
   * Execute via JIT-enabled HTTP API
   * Uses Schema-Aware JIT compilation for maximum performance
   */
  private async executeViaJit(
    ruleset: any,
    input: any,
    options: ExecutionOptions
  ): Promise<JITExecutionResult> {
    const endpoint = options.httpEndpoint || 'http://localhost:8080';

    try {
      // Build JIT execution request
      const requestBody: any = {
        ruleset,
        input,
        trace: options.includeTrace ?? true,
        jit_enabled: true,
      };

      // Include schema if provided
      if (options.jitSchema) {
        requestBody.schema = options.jitSchema;
      }

      const response = await fetch(`${endpoint}/api/v1/execute/jit`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(requestBody),
      });

      if (!response.ok) {
        // Fall back to regular HTTP execution if JIT endpoint not available
        if (response.status === 404) {
          console.warn('[JIT] JIT endpoint not available, falling back to regular HTTP');
          const httpResult = await this.executeViaHttp(ruleset, input, options);
          return {
            ...httpResult,
            jit_metrics: {
              jit_enabled: false,
              compiled_expressions: 0,
              compilation_time_us: 0,
              speedup_factor: 1.0,
            },
          };
        }
        throw new Error(
          `JIT execution failed: ${response.status} ${await response.text()}`
        );
      }

      return response.json();
    } catch (error) {
      // Fall back to HTTP if JIT fails
      console.warn('[JIT] JIT execution failed, falling back to HTTP:', error);
      const httpResult = await this.executeViaHttp(ruleset, input, options);
      return {
        ...httpResult,
        jit_metrics: {
          jit_enabled: false,
          compiled_expressions: 0,
          compilation_time_us: 0,
          speedup_factor: 1.0,
        },
      };
    }
  }

  /**
   * Execute via WASM
   */
  private async executeViaWasm(
    ruleset: any,
    input: any,
    options: ExecutionOptions
  ): Promise<ExecutionResult> {
    await this.initWasm();

    console.log('this.wasmModule', this.wasmModule);

    if (!this.wasmModule) {
      throw new Error('WASM module not initialized');
    }

    try {
      const rulesetJson = JSON.stringify(ruleset);
      const inputJson = JSON.stringify(input);
      const includeTrace = options.includeTrace ?? true;

      // Debug: log the JSON being sent
      console.log('[WASM] Ruleset JSON:', rulesetJson);
      console.log('[WASM] Input JSON:', inputJson);

      // WASM functions may be async (when using the loader)
      const resultJson = await Promise.resolve(
        this.wasmModule.execute_ruleset(rulesetJson, inputJson, includeTrace)
      );

      console.log('[WASM] Result JSON:', resultJson);
      return JSON.parse(resultJson);
    } catch (error: any) {
      // Try to extract more detailed error info
      let errorMessage = 'Unknown error';
      if (error instanceof Error) {
        errorMessage = error.message;
      } else if (typeof error === 'string') {
        errorMessage = error;
      } else if (error && typeof error.toString === 'function') {
        errorMessage = error.toString();
      }

      console.error('[WASM] Execution error:', error);
      throw new Error(`WASM execution failed: ${errorMessage}`);
    }
  }

  /**
   * Execute via HTTP API
   */
  private async executeViaHttp(
    ruleset: any,
    input: any,
    options: ExecutionOptions
  ): Promise<ExecutionResult> {
    const endpoint = options.httpEndpoint || 'http://localhost:8080';

    try {
      // 1. Upload ruleset
      const uploadResponse = await fetch(`${endpoint}/api/rulesets`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ ruleset }),
      });

      if (!uploadResponse.ok) {
        throw new Error(
          `Failed to upload ruleset: ${uploadResponse.status} ${await uploadResponse.text()}`
        );
      }

      // 2. Execute
      const executeResponse = await fetch(
        `${endpoint}/api/rulesets/${ruleset.config.name}/execute`,
        {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            input,
            trace: options.includeTrace ?? true,
          }),
        }
      );

      if (!executeResponse.ok) {
        throw new Error(
          `Execution failed: ${executeResponse.status} ${await executeResponse.text()}`
        );
      }

      return executeResponse.json();
    } catch (error) {
      throw new Error(
        `HTTP execution failed: ${error instanceof Error ? error.message : 'Unknown error'}`
      );
    }
  }

  /**
   * Validate a ruleset
   * 验证规则集
   */
  async validate(
    ruleset: RuleSet,
    options: Pick<ExecutionOptions, 'mode' | 'httpEndpoint'> = {}
  ): Promise<ValidationResult> {
    // First check client-side compatibility
    const compatErrors = validateEngineCompatibility(ruleset);
    if (compatErrors.length > 0) {
      return {
        valid: false,
        errors: compatErrors,
      };
    }

    // Convert to engine format
    const engineRuleset = convertToEngineFormat(ruleset);

    // Choose validation mode
    const mode = options.mode || 'wasm';
    if (mode === 'http' || options.httpEndpoint) {
      return this.validateViaHttp(engineRuleset, options);
    } else {
      return this.validateViaWasm(engineRuleset);
    }
  }

  /**
   * Validate via WASM
   */
  private async validateViaWasm(ruleset: any): Promise<ValidationResult> {
    await this.initWasm();

    if (!this.wasmModule) {
      throw new Error('WASM module not initialized');
    }

    try {
      const rulesetJson = JSON.stringify(ruleset);
      const resultJson = await Promise.resolve(this.wasmModule.validate_ruleset(rulesetJson));
      return JSON.parse(resultJson);
    } catch (error) {
      throw new Error(
        `WASM validation failed: ${error instanceof Error ? error.message : 'Unknown error'}`
      );
    }
  }

  /**
   * Validate via HTTP API
   */
  private async validateViaHttp(
    ruleset: any,
    options: Pick<ExecutionOptions, 'httpEndpoint'>
  ): Promise<ValidationResult> {
    const endpoint = options.httpEndpoint || 'http://localhost:8080';

    try {
      const response = await fetch(`${endpoint}/api/rulesets/validate`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ ruleset }),
      });

      if (!response.ok) {
        throw new Error(`Validation failed: ${response.status} ${await response.text()}`);
      }

      return response.json();
    } catch (error) {
      throw new Error(
        `HTTP validation failed: ${error instanceof Error ? error.message : 'Unknown error'}`
      );
    }
  }

  /**
   * Evaluate an expression
   * 计算表达式
   */
  async evalExpression(
    expression: string,
    context: Record<string, any> = {},
    options: EvalOptions = {}
  ): Promise<EvalResult> {
    const mode = options.mode || 'wasm';
    if (mode === 'http' || options.httpEndpoint) {
      return this.evalViaHttp(expression, context, options);
    } else {
      return this.evalViaWasm(expression, context);
    }
  }

  /**
   * Evaluate via WASM
   */
  private async evalViaWasm(expression: string, context: Record<string, any>): Promise<EvalResult> {
    await this.initWasm();

    if (!this.wasmModule) {
      throw new Error('WASM module not initialized');
    }

    try {
      const contextJson = JSON.stringify(context);
      const resultJson = await Promise.resolve(
        this.wasmModule.eval_expression(expression, contextJson)
      );
      return JSON.parse(resultJson);
    } catch (error) {
      throw new Error(
        `WASM eval failed: ${error instanceof Error ? error.message : 'Unknown error'}`
      );
    }
  }

  /**
   * Evaluate via HTTP API
   */
  private async evalViaHttp(
    expression: string,
    context: Record<string, any>,
    options: EvalOptions
  ): Promise<EvalResult> {
    const endpoint = options.httpEndpoint || 'http://localhost:8080';

    try {
      const response = await fetch(`${endpoint}/api/eval`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ expression, context }),
      });

      if (!response.ok) {
        throw new Error(`Eval failed: ${response.status} ${await response.text()}`);
      }

      return response.json();
    } catch (error) {
      throw new Error(
        `HTTP eval failed: ${error instanceof Error ? error.message : 'Unknown error'}`
      );
    }
  }

  /**
   * Analyze JIT compatibility for a ruleset
   * 分析规则集的 JIT 兼容性
   */
  async analyzeJitCompatibility(
    ruleset: RuleSet,
    options: Pick<ExecutionOptions, 'mode' | 'httpEndpoint'> = {}
  ): Promise<JITRulesetAnalysis> {
    const engineRuleset = convertToEngineFormat(ruleset);
    const mode = options.mode || 'wasm';

    if (mode === 'wasm') {
      return this.analyzeJitViaWasm(engineRuleset);
    } else {
      return this.analyzeJitViaHttp(engineRuleset, options);
    }
  }

  /**
   * Analyze JIT compatibility via WASM
   */
  private async analyzeJitViaWasm(ruleset: any): Promise<JITRulesetAnalysis> {
    await this.initWasm();

    if (!this.wasmModule) {
      throw new Error('WASM module not initialized');
    }

    try {
      const rulesetJson = JSON.stringify(ruleset);
      const resultJson = await Promise.resolve(
        this.wasmModule.analyze_ruleset_jit(rulesetJson)
      );
      const result = JSON.parse(resultJson);
      
      // Convert snake_case to camelCase for frontend
      return {
        overallCompatible: result.overall_compatible,
        compatibleCount: result.compatible_count,
        incompatibleCount: result.incompatible_count,
        totalExpressions: result.total_expressions,
        expressions: result.expressions.map((e: any) => ({
          stepId: e.step_id,
          stepName: e.step_name,
          location: e.location,
          expression: e.expression,
          analysis: {
            jitCompatible: e.analysis.jit_compatible,
            reason: e.analysis.reason,
            accessedFields: e.analysis.accessed_fields,
            unsupportedFeatures: e.analysis.unsupported_features,
            supportedFeatures: e.analysis.supported_features,
          },
        })),
        estimatedSpeedup: result.estimated_speedup,
        requiredFields: result.required_fields.map((f: any) => ({
          path: f.path,
          inferredType: f.inferred_type,
          usedInSteps: f.used_in_steps,
        })),
      };
    } catch (error) {
      throw new Error(
        `WASM JIT analysis failed: ${error instanceof Error ? error.message : 'Unknown error'}`
      );
    }
  }

  /**
   * Analyze JIT compatibility via HTTP API
   */
  private async analyzeJitViaHttp(
    ruleset: any,
    options: Pick<ExecutionOptions, 'httpEndpoint'>
  ): Promise<JITRulesetAnalysis> {
    const endpoint = options.httpEndpoint || 'http://localhost:8080';

    try {
      const response = await fetch(`${endpoint}/api/v1/debug/jit/analyze-ruleset`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(ruleset),
      });

      if (!response.ok) {
        throw new Error(`JIT analysis failed: ${response.status} ${await response.text()}`);
      }

      const result = await response.json();
      
      // Convert snake_case to camelCase for frontend
      return {
        overallCompatible: result.overall_compatible,
        compatibleCount: result.compatible_count,
        incompatibleCount: result.incompatible_count,
        totalExpressions: result.total_expressions,
        expressions: result.expressions?.map((e: any) => ({
          stepId: e.step_id,
          stepName: e.step_name,
          location: e.location,
          expression: e.expression,
          analysis: {
            jitCompatible: e.analysis?.jit_compatible,
            reason: e.analysis?.reason,
            accessedFields: e.analysis?.accessed_fields,
            unsupportedFeatures: e.analysis?.unsupported_features,
            supportedFeatures: e.analysis?.supported_features,
          },
        })) || [],
        estimatedSpeedup: result.estimated_speedup || 1.0,
        requiredFields: result.required_fields?.map((f: any) => ({
          path: f.path,
          inferredType: f.inferred_type,
          usedInSteps: f.used_in_steps,
        })) || [],
      };
    } catch (error) {
      throw new Error(
        `HTTP JIT analysis failed: ${error instanceof Error ? error.message : 'Unknown error'}`
      );
    }
  }
}

/**
 * Create a singleton executor instance
 * 创建单例执行器实例
 */
let defaultExecutor: RuleExecutor | null = null;

export function getDefaultExecutor(): RuleExecutor {
  if (!defaultExecutor) {
    defaultExecutor = new RuleExecutor();
  }
  return defaultExecutor;
}
