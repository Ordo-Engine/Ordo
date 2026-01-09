/**
 * Expression types for the Ordo rule engine
 * 表达式类型定义
 */

/** Value types */
export type ValueType = 'string' | 'number' | 'boolean' | 'null' | 'array' | 'object';

/** Literal value */
export interface LiteralExpr {
  type: 'literal';
  value: string | number | boolean | null;
  valueType: ValueType;
}

/** Variable reference (e.g., $.user.age, $result.code) */
export interface VariableExpr {
  type: 'variable';
  path: string;
}

/** Binary operation */
export type BinaryOp =
  | 'eq' // ==
  | 'ne' // !=
  | 'gt' // >
  | 'gte' // >=
  | 'lt' // <
  | 'lte' // <=
  | 'and' // &&
  | 'or' // ||
  | 'add' // +
  | 'sub' // -
  | 'mul' // *
  | 'div' // /
  | 'mod' // %
  | 'in' // in
  | 'contains'; // contains

export interface BinaryExpr {
  type: 'binary';
  op: BinaryOp;
  left: Expr;
  right: Expr;
}

/** Unary operation */
export type UnaryOp = 'not' | 'neg';

export interface UnaryExpr {
  type: 'unary';
  op: UnaryOp;
  operand: Expr;
}

/** Function call */
export interface FunctionExpr {
  type: 'function';
  name: string;
  args: Expr[];
}

/** Conditional expression (ternary) */
export interface ConditionalExpr {
  type: 'conditional';
  condition: Expr;
  thenExpr: Expr;
  elseExpr: Expr;
}

/** Array literal */
export interface ArrayExpr {
  type: 'array';
  elements: Expr[];
}

/** Object literal */
export interface ObjectExpr {
  type: 'object';
  properties: Record<string, Expr>;
}

/** Member access (e.g., obj.field or obj["field"]) */
export interface MemberExpr {
  type: 'member';
  object: Expr;
  property: string | Expr;
  computed: boolean; // true for obj[expr], false for obj.field
}

/** Expression union type */
export type ExprUnion =
  | LiteralExpr
  | VariableExpr
  | BinaryExpr
  | UnaryExpr
  | FunctionExpr
  | ConditionalExpr
  | ArrayExpr
  | ObjectExpr
  | MemberExpr;

/** Alias for Expr type */
export type Expr = ExprUnion;

// ============================================================================
// Expression builder helpers
// ============================================================================

export const Expr = {
  /** Create a literal expression */
  literal(value: string | number | boolean | null): LiteralExpr {
    let valueType: ValueType = 'null';
    if (typeof value === 'string') valueType = 'string';
    else if (typeof value === 'number') valueType = 'number';
    else if (typeof value === 'boolean') valueType = 'boolean';

    return { type: 'literal', value, valueType };
  },

  /** Create a string literal */
  string(value: string): LiteralExpr {
    return { type: 'literal', value, valueType: 'string' };
  },

  /** Create a number literal */
  number(value: number): LiteralExpr {
    return { type: 'literal', value, valueType: 'number' };
  },

  /** Create a boolean literal */
  boolean(value: boolean): LiteralExpr {
    return { type: 'literal', value, valueType: 'boolean' };
  },

  /** Create a null literal */
  null(): LiteralExpr {
    return { type: 'literal', value: null, valueType: 'null' };
  },

  /** Create a variable reference */
  variable(path: string): VariableExpr {
    return { type: 'variable', path };
  },

  /** Create a binary expression */
  binary(op: BinaryOp, left: Expr, right: Expr): BinaryExpr {
    return { type: 'binary', op, left, right };
  },

  /** Create a unary expression */
  unary(op: UnaryOp, operand: Expr): UnaryExpr {
    return { type: 'unary', op, operand };
  },

  /** Create a function call */
  call(name: string, ...args: Expr[]): FunctionExpr {
    return { type: 'function', name, args };
  },

  /** Create a conditional expression */
  conditional(condition: Expr, thenExpr: Expr, elseExpr: Expr): ConditionalExpr {
    return { type: 'conditional', condition, thenExpr, elseExpr };
  },

  /** Create an array expression */
  array(...elements: Expr[]): ArrayExpr {
    return { type: 'array', elements };
  },

  /** Create an object expression */
  object(properties: Record<string, Expr>): ObjectExpr {
    return { type: 'object', properties };
  },

  /** Create a member access expression */
  member(object: Expr, property: string | Expr, computed = false): MemberExpr {
    return { type: 'member', object, property, computed };
  },

  // Shorthand operators
  eq: (left: Expr, right: Expr) => Expr.binary('eq', left, right),
  ne: (left: Expr, right: Expr) => Expr.binary('ne', left, right),
  gt: (left: Expr, right: Expr) => Expr.binary('gt', left, right),
  gte: (left: Expr, right: Expr) => Expr.binary('gte', left, right),
  lt: (left: Expr, right: Expr) => Expr.binary('lt', left, right),
  lte: (left: Expr, right: Expr) => Expr.binary('lte', left, right),
  and: (left: Expr, right: Expr) => Expr.binary('and', left, right),
  or: (left: Expr, right: Expr) => Expr.binary('or', left, right),
  not: (operand: Expr) => Expr.unary('not', operand),
  neg: (operand: Expr) => Expr.unary('neg', operand),
};

/** Convert expression to human-readable string */
export function exprToString(expr: Expr): string {
  switch (expr.type) {
    case 'literal':
      if (expr.valueType === 'string') return `"${expr.value}"`;
      if (expr.value === null) return 'null';
      return String(expr.value);

    case 'variable':
      return expr.path;

    case 'binary': {
      const opMap: Record<BinaryOp, string> = {
        eq: '==',
        ne: '!=',
        gt: '>',
        gte: '>=',
        lt: '<',
        lte: '<=',
        and: '&&',
        or: '||',
        add: '+',
        sub: '-',
        mul: '*',
        div: '/',
        mod: '%',
        in: 'in',
        contains: 'contains',
      };
      return `(${exprToString(expr.left)} ${opMap[expr.op]} ${exprToString(expr.right)})`;
    }

    case 'unary': {
      const opMap: Record<UnaryOp, string> = { not: '!', neg: '-' };
      return `${opMap[expr.op]}${exprToString(expr.operand)}`;
    }

    case 'function':
      return `${expr.name}(${expr.args.map(exprToString).join(', ')})`;

    case 'conditional':
      return `(${exprToString(expr.condition)} ? ${exprToString(expr.thenExpr)} : ${exprToString(expr.elseExpr)})`;

    case 'array':
      return `[${expr.elements.map(exprToString).join(', ')}]`;

    case 'object': {
      const props = Object.entries(expr.properties)
        .map(([k, v]) => `${k}: ${exprToString(v)}`)
        .join(', ');
      return `{${props}}`;
    }

    case 'member':
      if (expr.computed) {
        return `${exprToString(expr.object)}[${typeof expr.property === 'string' ? `"${expr.property}"` : exprToString(expr.property)}]`;
      }
      return `${exprToString(expr.object)}.${expr.property}`;

    default:
      return '<?>';
  }
}

