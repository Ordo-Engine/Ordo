/**
 * @ordo-engine/editor-vue
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

// Schema components (JIT Schema Editor, Protobuf import)
export * from './components/schema';

// Table components (Decision Table editor)
export * from './components/table';

// Icons
export { default as OrdoIcon } from './components/icons/OrdoIcon.vue';

// Composables
export { useEditorStore } from './composables/useEditorStore';
export type { UseEditorStoreReturn } from './composables/useEditorStore';

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
} from '@ordo-engine/editor-core';

// Re-export type aliases that have same-name values
// These need explicit type/value separation for isolatedModules
export type { Step, Condition, Expr } from '@ordo-engine/editor-core';

// Re-export store types
export type {
  EditorState,
  EditorListener,
  Command,
  SchemaContext,
  ResolvedField,
  OperatorInfo,
  ValueHint,
} from '@ordo-engine/editor-core';

// Re-export decision table types
export type {
  DecisionTable,
  DecisionTableRow,
  InputColumn,
  OutputColumn,
  CellValue,
  SchemaFieldType,
  HitPolicy,
} from '@ordo-engine/editor-core';

// Re-export document types
export type {
  RuleDocument,
  FlowDocument,
  DecisionTableDocument,
  DocumentType,
} from '@ordo-engine/editor-core';

export {
  EditorStore,
  CommandBus,
  HistoryManager,
  createSchemaContext,
  AddStepCommand,
  RemoveStepCommand,
  UpdateStepCommand,
  MoveStepCommand,
  AddBranchCommand,
  RemoveBranchCommand,
  UpdateBranchCommand,
  ReorderBranchCommand,
  ConnectStepsCommand,
  DisconnectStepsCommand,
  SetStartStepCommand,
  UpdateConfigCommand,
  SetSchemaCommand,
  BatchCommand,
  PasteStepCommand,
  SelectStepCommand,
  ImportDecisionTableCommand,
} from '@ordo-engine/editor-core';

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
  // File operations
  detectFileFormat,
  importRuleSetFromFile,
  exportRuleSetToFile,
  downloadFile,
  downloadBinaryFile,
  readFileAsText,
  readFileAsArrayBuffer,
  // Decision table
  DecisionTable as DecisionTableFactory,
  cellValueToString,
  compileTableToSteps,
  decompileStepsToTable,
  // Document model
  isFlowDocument,
  isDecisionTableDocument,
  documentToRuleSet,
  flowDocumentToRuleSet,
  tableDocumentToRuleSet,
  ruleSetToFlowDocument,
  createEmptyFlowDocument,
  createEmptyTableDocument,
  detectDocumentType,
} from '@ordo-engine/editor-core';

// Export factory objects with their original names
export const Step = StepFactory;
export const Condition = ConditionFactory;
export const Expr = ExprFactory;

// Export factory objects for decision tables
export { DecisionTableFactory };
export const DecisionTableHelper = DecisionTableFactory;

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
  // File operations
  detectFileFormat,
  importRuleSetFromFile,
  exportRuleSetToFile,
  downloadFile,
  downloadBinaryFile,
  readFileAsText,
  readFileAsArrayBuffer,
  // Decision table
  cellValueToString,
  compileTableToSteps,
  decompileStepsToTable,
  // Document model
  isFlowDocument,
  isDecisionTableDocument,
  documentToRuleSet,
  flowDocumentToRuleSet,
  tableDocumentToRuleSet,
  ruleSetToFlowDocument,
  createEmptyFlowDocument,
  createEmptyTableDocument,
  detectDocumentType,
};

// Version
export const VERSION = '0.3.0';
