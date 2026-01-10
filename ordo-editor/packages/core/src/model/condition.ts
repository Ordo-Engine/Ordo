/**
 * Condition types for decision steps
 * 条件类型定义
 */

import { Expr, exprToString } from './expr';

/** Simple comparison condition */
export interface SimpleCondition {
  type: 'simple';
  /** Left-hand side expression */
  left: Expr;
  /** Comparison operator */
  operator: 'eq' | 'ne' | 'gt' | 'gte' | 'lt' | 'lte' | 'in' | 'contains' | 'startsWith' | 'endsWith';
  /** Right-hand side expression */
  right: Expr;
}

/** Logical combination of conditions */
export interface LogicalCondition {
  type: 'logical';
  /** Logical operator */
  operator: 'and' | 'or';
  /** Child conditions */
  conditions: Condition[];
}

/** Negation of a condition */
export interface NotCondition {
  type: 'not';
  /** Condition to negate */
  condition: Condition;
}

/** Expression-based condition (for advanced users) */
export interface ExpressionCondition {
  type: 'expression';
  /** Raw expression string (will be parsed) */
  expression: string;
  /** Parsed expression (optional, for validation) */
  parsed?: Expr;
}

/** Always true/false condition */
export interface ConstantCondition {
  type: 'constant';
  value: boolean;
}

/** Condition union type */
export type ConditionUnion =
  | SimpleCondition
  | LogicalCondition
  | NotCondition
  | ExpressionCondition
  | ConstantCondition;

/** Alias for Condition type */
export type Condition = ConditionUnion;

// ============================================================================
// Condition builder helpers
// ============================================================================

export const Condition = {
  /** Create a simple comparison condition */
  simple(
    left: Expr,
    operator: SimpleCondition['operator'],
    right: Expr
  ): SimpleCondition {
    return { type: 'simple', left, operator, right };
  },

  /** Create an AND condition */
  and(...conditions: Condition[]): LogicalCondition {
    return { type: 'logical', operator: 'and', conditions };
  },

  /** Create an OR condition */
  or(...conditions: Condition[]): LogicalCondition {
    return { type: 'logical', operator: 'or', conditions };
  },

  /** Create a NOT condition */
  not(condition: Condition): NotCondition {
    return { type: 'not', condition };
  },

  /** Create an expression condition */
  expression(expression: string): ExpressionCondition {
    return { type: 'expression', expression };
  },

  /** Create an always-true condition */
  alwaysTrue(): ConstantCondition {
    return { type: 'constant', value: true };
  },

  /** Create an always-false condition */
  alwaysFalse(): ConstantCondition {
    return { type: 'constant', value: false };
  },
};

/** Convert condition to human-readable string */
export function conditionToString(condition: Condition): string {
  switch (condition.type) {
    case 'simple': {
      const opMap: Record<SimpleCondition['operator'], string> = {
        eq: '==',
        ne: '!=',
        gt: '>',
        gte: '>=',
        lt: '<',
        lte: '<=',
        in: 'in',
        contains: 'contains',
        startsWith: 'startsWith',
        endsWith: 'endsWith',
      };
      return `${exprToString(condition.left)} ${opMap[condition.operator]} ${exprToString(condition.right)}`;
    }

    case 'logical': {
      const op = condition.operator === 'and' ? ' && ' : ' || ';
      const parts = condition.conditions.map((c) => {
        const str = conditionToString(c);
        // Add parentheses for nested logical conditions
        if (c.type === 'logical' && c.operator !== condition.operator) {
          return `(${str})`;
        }
        return str;
      });
      return parts.join(op);
    }

    case 'not':
      return `!(${conditionToString(condition.condition)})`;

    case 'expression':
      return condition.expression;

    case 'constant':
      return condition.value ? 'true' : 'false';

    default:
      return '<?>';
  }
}

/** Check if a condition is empty or trivial */
export function isEmptyCondition(condition: Condition): boolean {
  if (condition.type === 'constant') return true;
  if (condition.type === 'logical' && condition.conditions.length === 0) return true;
  return false;
}

