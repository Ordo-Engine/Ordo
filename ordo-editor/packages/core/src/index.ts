/**
 * @ordo-engine/editor-core
 *
 * Core logic for Ordo Rule Editor (framework-agnostic)
 * 规则编辑器核心逻辑（框架无关）
 */

// Model exports
export * from './model';

// Validator exports
export * from './validator';

// Serializer exports
export * from './serializer';

// Event bus exports
export * from './events';

// Utility exports
export * from './utils';

// Engine integration exports
export * from './engine';

// Schema exports (Protobuf parsing, JIT schema utilities)
export * from './schema';

// Version
export const VERSION = '0.2.0';
