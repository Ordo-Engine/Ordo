/**
 * @ordo/editor-vue
 *
 * Vue components for Ordo Rule Editor
 * Vue 规则编辑器组件库
 */

// Import styles
import './styles';

// Base components
export * from './components/base';

// Step editor components
export * from './components/step';

// Form mode components
export * from './components/form';

// Flow mode components
export * from './components/flow';

// Execution components
export * from './components/execution';

// Debug components
export * from './components/debug';

// Icons
export { default as OrdoIcon } from './components/icons/OrdoIcon.vue';

// I18n
export { createI18n, useI18n, LOCALE_KEY, type Lang } from './locale';

// Re-export core types
export type {
  RuleSet,
  RuleSetConfig,
  SchemaField,
  FieldType,
  StepUnion,
  StepType,
  DecisionStep,
  ActionStep,
  TerminalStep,
  Branch,
  VariableAssignment,
  OutputField,
  ExternalCall,
  ConditionUnion,
  SimpleCondition,
  LogicalCondition,
  NotCondition,
  ExpressionCondition,
  ConstantCondition,
  ExprUnion,
  LiteralExpr,
  VariableExpr,
  BinaryExpr,
  UnaryExpr,
  FunctionExpr,
  ConditionalExpr,
  ArrayExpr,
  ObjectExpr,
  MemberExpr,
  BinaryOp,
  UnaryOp,
  ValueType,
  ValidationResult,
  ValidationError,
} from '@ordo/editor-core';

// Re-export type aliases that have same-name values
// These need explicit type/value separation for isolatedModules
export type { Step, Condition, Expr } from '@ordo/editor-core';

// Re-export values (factory objects and functions)
import {
  Step as StepFactory,
  Condition as ConditionFactory,
  Expr as ExprFactory,
  validateRuleSet,
  serializeRuleSet,
  deserializeRuleSet,
  generateId,
  isDecisionStep,
  isActionStep,
  isTerminalStep,
  getNextStepIds,
  exprToString,
  conditionToString,
  convertToEngineFormat,
} from '@ordo/editor-core';

// Export factory objects with their original names
export const Step = StepFactory;
export const Condition = ConditionFactory;
export const Expr = ExprFactory;

// Export functions
export {
  validateRuleSet,
  serializeRuleSet,
  deserializeRuleSet,
  generateId,
  isDecisionStep,
  isActionStep,
  isTerminalStep,
  getNextStepIds,
  exprToString,
  conditionToString,
  convertToEngineFormat,
};

// Version
export const VERSION = '0.1.0';
