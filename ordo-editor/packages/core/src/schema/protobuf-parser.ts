/**
 * Protobuf Schema Parser
 * 解析 .proto 文件并生成 JIT Schema
 *
 * This module parses protobuf definitions and converts them to JITSchema
 * format for use with the Schema-Aware JIT compiler.
 */

import type { JITSchema, JITSchemaField, JITFieldType, JITPrimitiveType } from '../model/ruleset';

/**
 * Protobuf field descriptor (intermediate representation)
 */
export interface ProtoFieldDescriptor {
  name: string;
  number: number; // Tag number
  type: string; // Protobuf type name
  repeated: boolean;
  optional: boolean;
  oneofIndex?: number;
  typeName?: string; // For message/enum types
}

/**
 * Protobuf message descriptor
 */
export interface ProtoMessageDescriptor {
  name: string;
  fullName: string;
  fields: ProtoFieldDescriptor[];
  nestedTypes: ProtoMessageDescriptor[];
  enumTypes: ProtoEnumDescriptor[];
}

/**
 * Protobuf enum descriptor
 */
export interface ProtoEnumDescriptor {
  name: string;
  values: Array<{ name: string; number: number }>;
}

/**
 * Parsed proto file result
 */
export interface ParsedProtoFile {
  package: string;
  messages: ProtoMessageDescriptor[];
  enums: ProtoEnumDescriptor[];
}

/**
 * Map protobuf type to JIT field type
 */
function mapProtoTypeToJIT(protoType: string, repeated: boolean, optional: boolean): JITFieldType {
  let baseType: JITFieldType;

  // Map protobuf scalar types to JIT types
  switch (protoType.toLowerCase()) {
    case 'double':
      baseType = 'float64';
      break;
    case 'float':
      baseType = 'float32';
      break;
    case 'int32':
    case 'sint32':
    case 'sfixed32':
      baseType = 'int32';
      break;
    case 'int64':
    case 'sint64':
    case 'sfixed64':
      baseType = 'int64';
      break;
    case 'uint32':
    case 'fixed32':
      baseType = 'uint32';
      break;
    case 'uint64':
    case 'fixed64':
      baseType = 'uint64';
      break;
    case 'bool':
      baseType = 'bool';
      break;
    case 'string':
      baseType = 'string';
      break;
    case 'bytes':
      baseType = 'bytes';
      break;
    default:
      // Assume it's a message or enum type
      if (protoType.startsWith('.')) {
        baseType = { message: protoType.slice(1) };
      } else {
        baseType = { message: protoType };
      }
  }

  // Wrap in repeated or optional if needed
  if (repeated) {
    return { repeated: baseType };
  }
  if (optional) {
    return { optional: baseType };
  }

  return baseType;
}

/**
 * Get the size of a primitive type in bytes
 */
function getPrimitiveSize(type: JITPrimitiveType): number {
  switch (type) {
    case 'bool':
      return 1;
    case 'int32':
    case 'uint32':
    case 'float32':
      return 4;
    case 'int64':
    case 'uint64':
    case 'float64':
      return 8;
    case 'string':
    case 'bytes':
      return 24; // Rust String/Vec size (ptr + len + cap)
    default:
      return 8;
  }
}

/**
 * Calculate C-compatible layout for fields
 * Uses simple alignment rules similar to Rust #[repr(C)]
 */
function calculateLayout(fields: JITSchemaField[]): {
  fields: JITSchemaField[];
  totalSize: number;
} {
  let offset = 0;
  const layoutFields: JITSchemaField[] = [];

  for (const field of fields) {
    let size: number;

    if (typeof field.type === 'string') {
      size = getPrimitiveSize(field.type as JITPrimitiveType);
    } else if ('repeated' in field.type || 'message' in field.type) {
      size = 24; // Vec or struct pointer
    } else if ('optional' in field.type) {
      // Option<T> in Rust has extra byte for discriminant + alignment
      const innerType = field.type.optional;
      if (typeof innerType === 'string') {
        size = getPrimitiveSize(innerType as JITPrimitiveType) + 8; // Rough estimate
      } else {
        size = 32;
      }
    } else {
      size = 8;
    }

    // Align to natural boundary (min of size and 8)
    const alignment = Math.min(size, 8);
    offset = Math.ceil(offset / alignment) * alignment;

    layoutFields.push({
      ...field,
      offset,
      size,
    });

    offset += size;
  }

  // Align total size to 8 bytes
  const totalSize = Math.ceil(offset / 8) * 8;

  return { fields: layoutFields, totalSize };
}

/**
 * Simple proto file parser
 * Parses a subset of proto3 syntax
 */
export function parseProtoContent(content: string): ParsedProtoFile {
  const lines = content.split('\n');
  let packageName = '';
  const messages: ProtoMessageDescriptor[] = [];
  const enums: ProtoEnumDescriptor[] = [];

  let currentMessage: ProtoMessageDescriptor | null = null;
  let currentEnum: ProtoEnumDescriptor | null = null;
  let braceDepth = 0;
  const messageStack: ProtoMessageDescriptor[] = [];

  for (let line of lines) {
    line = line.trim();

    // Skip comments and empty lines
    if (line.startsWith('//') || line === '') continue;

    // Remove inline comments
    const commentIdx = line.indexOf('//');
    if (commentIdx > 0) {
      line = line.slice(0, commentIdx).trim();
    }

    // Package declaration
    const packageMatch = line.match(/^package\s+([^;]+);/);
    if (packageMatch) {
      packageName = packageMatch[1];
      continue;
    }

    // Message start
    const messageMatch = line.match(/^message\s+(\w+)\s*\{?/);
    if (messageMatch) {
      if (currentMessage) {
        messageStack.push(currentMessage);
      }
      currentMessage = {
        name: messageMatch[1],
        fullName: packageName ? `${packageName}.${messageMatch[1]}` : messageMatch[1],
        fields: [],
        nestedTypes: [],
        enumTypes: [],
      };
      if (line.includes('{')) braceDepth++;
      continue;
    }

    // Enum start
    const enumMatch = line.match(/^enum\s+(\w+)\s*\{?/);
    if (enumMatch) {
      currentEnum = {
        name: enumMatch[1],
        values: [],
      };
      if (line.includes('{')) braceDepth++;
      continue;
    }

    // Brace tracking
    if (line === '{') {
      braceDepth++;
      continue;
    }
    if (line === '}' || line.startsWith('}')) {
      braceDepth--;

      if (currentEnum) {
        if (currentMessage) {
          currentMessage.enumTypes.push(currentEnum);
        } else {
          enums.push(currentEnum);
        }
        currentEnum = null;
      } else if (currentMessage && braceDepth === messageStack.length) {
        if (messageStack.length > 0) {
          const parent = messageStack.pop()!;
          parent.nestedTypes.push(currentMessage);
          currentMessage = parent;
        } else {
          messages.push(currentMessage);
          currentMessage = null;
        }
      }
      continue;
    }

    // Enum value
    if (currentEnum) {
      const valueMatch = line.match(/^(\w+)\s*=\s*(\d+)/);
      if (valueMatch) {
        currentEnum.values.push({
          name: valueMatch[1],
          number: parseInt(valueMatch[2], 10),
        });
      }
      continue;
    }

    // Field definition (within message)
    if (currentMessage) {
      // Handle repeated/optional modifiers
      const fieldMatch = line.match(/^(repeated\s+|optional\s+)?(\w+)\s+(\w+)\s*=\s*(\d+)/);
      if (fieldMatch) {
        const modifier = fieldMatch[1]?.trim() || '';
        const type = fieldMatch[2];
        const name = fieldMatch[3];
        const number = parseInt(fieldMatch[4], 10);

        currentMessage.fields.push({
          name,
          number,
          type,
          repeated: modifier === 'repeated',
          optional: modifier === 'optional',
        });
      }
    }
  }

  return {
    package: packageName,
    messages,
    enums,
  };
}

/**
 * Convert a parsed proto message to JITSchema
 */
export function protoMessageToJITSchema(
  message: ProtoMessageDescriptor,
  packageName: string
): JITSchema {
  const fields: JITSchemaField[] = message.fields.map((field) => ({
    name: field.name,
    type: mapProtoTypeToJIT(field.type, field.repeated, field.optional),
    offset: 0, // Will be calculated
    protoTag: field.number,
    required: !field.optional && !field.repeated,
  }));

  const layout = calculateLayout(fields);

  return {
    name: message.name,
    version: '1.0.0',
    fields: layout.fields,
    totalSize: layout.totalSize,
    source: 'protobuf',
    protoPackage: packageName,
  };
}

/**
 * Parse a .proto file content and return all JIT schemas
 */
export function parseProtoFile(content: string): JITSchema[] {
  const parsed = parseProtoContent(content);
  const schemas: JITSchema[] = [];

  function processMessage(msg: ProtoMessageDescriptor, prefix: string = '') {
    const fullName = prefix ? `${prefix}.${msg.name}` : msg.name;
    const schema = protoMessageToJITSchema(msg, parsed.package);
    schema.name = fullName;
    schemas.push(schema);

    // Process nested messages
    for (const nested of msg.nestedTypes) {
      processMessage(nested, fullName);
    }
  }

  for (const message of parsed.messages) {
    processMessage(message);
  }

  return schemas;
}

/**
 * Parse proto content and return a single schema for the specified message
 */
export function parseProtoForMessage(content: string, messageName: string): JITSchema | null {
  const schemas = parseProtoFile(content);
  return schemas.find((s) => s.name === messageName || s.name.endsWith(`.${messageName}`)) || null;
}

/**
 * Validate a proto file and return any errors
 */
export function validateProtoContent(content: string): { valid: boolean; errors: string[] } {
  const errors: string[] = [];

  try {
    const parsed = parseProtoContent(content);

    if (parsed.messages.length === 0) {
      errors.push('No messages found in proto file');
    }

    // Check for reserved keywords
    for (const msg of parsed.messages) {
      for (const field of msg.fields) {
        if (['type', 'class', 'interface'].includes(field.name)) {
          errors.push(`Field name "${field.name}" in message "${msg.name}" is a reserved keyword`);
        }
      }
    }
  } catch (e) {
    errors.push(`Parse error: ${e instanceof Error ? e.message : 'Unknown error'}`);
  }

  return {
    valid: errors.length === 0,
    errors,
  };
}

/**
 * Generate a sample proto content for testing
 */
export function generateSampleProto(): string {
  return `syntax = "proto3";

package ordo.example;

// Example context for JIT compilation
message LoanContext {
  double credit_score = 1;
  double debt_to_income = 2;
  int64 loan_amount = 3;
  int32 loan_term_months = 4;
  bool is_first_time_buyer = 5;
  string applicant_name = 6;
}

message RiskContext {
  double risk_score = 1;
  int32 risk_level = 2;
  bool is_high_risk = 3;
  double transaction_amount = 4;
  int64 account_age_days = 5;
}
`;
}
