import type { ConceptDefinition } from '@/api/types';
import type { Expr, RuleSet, SubRuleGraph } from '@ordo-engine/editor-core';

type RulesetStep = RuleSet['steps'][number];
type BinaryOperator = Extract<Expr, { type: 'binary' }>['op'];

const CONCEPT_PRELUDE_ID = '__ordo_concepts_prelude';
const IDENT_RE = /^[A-Za-z_][A-Za-z0-9_.]*$/;

type Token =
  | { type: 'identifier'; value: string }
  | { type: 'number'; value: string }
  | { type: 'string'; value: string }
  | { type: 'operator'; value: string }
  | { type: 'punct'; value: '(' | ')' | ',' }
  | { type: 'eof'; value: '' };

function cloneRuleset(ruleset: RuleSet): RuleSet {
  return JSON.parse(JSON.stringify(ruleset));
}

function normalizePath(path: string): string {
  if (path.startsWith('$.')) return path.slice(2);
  if (path.startsWith('$')) return path.slice(1);
  return path;
}

function tokenize(input: string): Token[] {
  const tokens: Token[] = [];
  let i = 0;

  while (i < input.length) {
    const ch = input[i];
    if (/\s/.test(ch)) {
      i += 1;
      continue;
    }

    const two = input.slice(i, i + 2);
    if (['>=', '<=', '==', '!=', '&&', '||'].includes(two)) {
      tokens.push({ type: 'operator', value: two });
      i += 2;
      continue;
    }

    if (['>', '<', '+', '-', '*', '/', '%', '!'].includes(ch)) {
      tokens.push({ type: 'operator', value: ch });
      i += 1;
      continue;
    }

    if (ch === '(' || ch === ')' || ch === ',') {
      tokens.push({ type: 'punct', value: ch });
      i += 1;
      continue;
    }

    if (ch === '"' || ch === "'") {
      const quote = ch;
      let value = '';
      i += 1;
      while (i < input.length) {
        const current = input[i];
        if (current === '\\' && i + 1 < input.length) {
          value += input[i + 1];
          i += 2;
          continue;
        }
        if (current === quote) break;
        value += current;
        i += 1;
      }
      if (input[i] !== quote) throw new Error('Unterminated string literal');
      i += 1;
      tokens.push({ type: 'string', value });
      continue;
    }

    if (/\d/.test(ch)) {
      let value = ch;
      i += 1;
      while (i < input.length && /[\d.]/.test(input[i])) {
        value += input[i];
        i += 1;
      }
      tokens.push({ type: 'number', value });
      continue;
    }

    if (/[A-Za-z_$]/.test(ch)) {
      let value = ch;
      i += 1;
      while (i < input.length && /[A-Za-z0-9_.$]/.test(input[i])) {
        value += input[i];
        i += 1;
      }
      if (value === 'and') tokens.push({ type: 'operator', value: '&&' });
      else if (value === 'or') tokens.push({ type: 'operator', value: '||' });
      else if (value === 'not') tokens.push({ type: 'operator', value: '!' });
      else if (value === 'contains' || value === 'in') tokens.push({ type: 'operator', value });
      else tokens.push({ type: 'identifier', value: normalizePath(value) });
      continue;
    }

    throw new Error(`Unsupported token "${ch}"`);
  }

  tokens.push({ type: 'eof', value: '' });
  return tokens;
}

class ConceptExpressionParser {
  private readonly tokens: Token[];
  private pos = 0;

  constructor(input: string) {
    this.tokens = tokenize(input);
  }

  parse(): Expr {
    const expr = this.parseOr();
    if (this.peek().type !== 'eof') {
      throw new Error(`Unexpected token "${this.peek().value}"`);
    }
    return expr;
  }

  private peek(): Token {
    return this.tokens[this.pos] ?? { type: 'eof', value: '' };
  }

  private advance(): Token {
    return this.tokens[this.pos++] ?? { type: 'eof', value: '' };
  }

  private matchOperator(...operators: string[]): string | null {
    const token = this.peek();
    if (token.type !== 'operator' || !operators.includes(token.value)) return null;
    this.advance();
    return token.value;
  }

  private matchPunct(value: '(' | ')' | ','): boolean {
    const token = this.peek();
    if (token.type !== 'punct' || token.value !== value) return false;
    this.advance();
    return true;
  }

  private parseOr(): Expr {
    let expr = this.parseAnd();
    while (this.matchOperator('||')) {
      expr = { type: 'binary', op: 'or', left: expr, right: this.parseAnd() };
    }
    return expr;
  }

  private parseAnd(): Expr {
    let expr = this.parseComparison();
    while (this.matchOperator('&&')) {
      expr = { type: 'binary', op: 'and', left: expr, right: this.parseComparison() };
    }
    return expr;
  }

  private parseComparison(): Expr {
    let expr = this.parseAdditive();
    const op = this.matchOperator('==', '!=', '>', '>=', '<', '<=', 'contains', 'in');
    if (!op) return expr;
    const opMap: Record<string, BinaryOperator> = {
      '==': 'eq',
      '!=': 'ne',
      '>': 'gt',
      '>=': 'gte',
      '<': 'lt',
      '<=': 'lte',
      contains: 'contains',
      in: 'in',
    };
    expr = { type: 'binary', op: opMap[op], left: expr, right: this.parseAdditive() };
    return expr;
  }

  private parseAdditive(): Expr {
    let expr = this.parseMultiplicative();
    let op = this.matchOperator('+', '-');
    while (op) {
      expr = {
        type: 'binary',
        op: op === '+' ? 'add' : 'sub',
        left: expr,
        right: this.parseMultiplicative(),
      };
      op = this.matchOperator('+', '-');
    }
    return expr;
  }

  private parseMultiplicative(): Expr {
    let expr = this.parseUnary();
    let op = this.matchOperator('*', '/', '%');
    while (op) {
      const opMap: Record<string, BinaryOperator> = { '*': 'mul', '/': 'div', '%': 'mod' };
      expr = { type: 'binary', op: opMap[op], left: expr, right: this.parseUnary() };
      op = this.matchOperator('*', '/', '%');
    }
    return expr;
  }

  private parseUnary(): Expr {
    if (this.matchOperator('!')) {
      return { type: 'unary', op: 'not', operand: this.parseUnary() };
    }
    if (this.matchOperator('-')) {
      return { type: 'unary', op: 'neg', operand: this.parseUnary() };
    }
    return this.parsePrimary();
  }

  private parsePrimary(): Expr {
    const token = this.advance();
    if (token.type === 'number') {
      return { type: 'literal', value: Number(token.value), valueType: 'number' };
    }
    if (token.type === 'string') {
      return { type: 'literal', value: token.value, valueType: 'string' };
    }
    if (token.type === 'identifier') {
      if (token.value === 'true' || token.value === 'false') {
        return { type: 'literal', value: token.value === 'true', valueType: 'boolean' };
      }
      if (token.value === 'null') {
        return { type: 'literal', value: null, valueType: 'null' };
      }
      if (this.matchPunct('(')) {
        const args: Expr[] = [];
        if (!this.matchPunct(')')) {
          do {
            args.push(this.parseOr());
          } while (this.matchPunct(','));
          if (!this.matchPunct(')')) throw new Error('Expected closing parenthesis');
        }
        return { type: 'function', name: token.value, args };
      }
      return { type: 'variable', path: token.value };
    }
    if (token.type === 'punct' && token.value === '(') {
      const expr = this.parseOr();
      if (!this.matchPunct(')')) throw new Error('Expected closing parenthesis');
      return expr;
    }
    throw new Error(`Unexpected token "${token.value}"`);
  }
}

function parseConceptExpression(expression: string): Expr {
  return new ConceptExpressionParser(expression).parse();
}

function rewriteExpressionStringConceptRefs(expression: string, conceptNames: Set<string>): string {
  let result = '';
  let i = 0;
  let quote: '"' | "'" | null = null;
  const reserved = new Set(['true', 'false', 'null', 'undefined', 'and', 'or', 'not', 'in']);

  while (i < expression.length) {
    const ch = expression[i];
    if (quote) {
      result += ch;
      if (ch === '\\' && i + 1 < expression.length) {
        result += expression[i + 1];
        i += 2;
        continue;
      }
      if (ch === quote) quote = null;
      i += 1;
      continue;
    }

    if (ch === '"' || ch === "'") {
      quote = ch;
      result += ch;
      i += 1;
      continue;
    }

    if (/[A-Za-z_$]/.test(ch)) {
      let value = ch;
      i += 1;
      while (i < expression.length && /[A-Za-z0-9_.$]/.test(expression[i])) {
        value += expression[i];
        i += 1;
      }
      const normalized = normalizePath(value);
      const nextToken = expression.slice(i).trimStart();
      if (
        conceptNames.has(normalized) &&
        value !== `$${normalized}` &&
        !reserved.has(normalized) &&
        !nextToken.startsWith('(')
      ) {
        result += `$${normalized}`;
      } else {
        result += value;
      }
      continue;
    }

    result += ch;
    i += 1;
  }

  return result;
}

function rewriteExprConceptRefs(expr: Expr, conceptNames: Set<string>): Expr {
  if (expr.type === 'variable') {
    const normalized = normalizePath(expr.path);
    if (conceptNames.has(normalized) && expr.path !== `$${normalized}`) {
      return { ...expr, path: `$${normalized}` };
    }
    return expr;
  }
  if (expr.type === 'binary') {
    return {
      ...expr,
      left: rewriteExprConceptRefs(expr.left, conceptNames),
      right: rewriteExprConceptRefs(expr.right, conceptNames),
    };
  }
  if (expr.type === 'unary') {
    return { ...expr, operand: rewriteExprConceptRefs(expr.operand, conceptNames) };
  }
  if (expr.type === 'function') {
    return { ...expr, args: expr.args.map((arg) => rewriteExprConceptRefs(arg, conceptNames)) };
  }
  if (expr.type === 'conditional') {
    return {
      ...expr,
      condition: rewriteExprConceptRefs(expr.condition, conceptNames),
      thenExpr: rewriteExprConceptRefs(expr.thenExpr, conceptNames),
      elseExpr: rewriteExprConceptRefs(expr.elseExpr, conceptNames),
    };
  }
  if (expr.type === 'array') {
    return {
      ...expr,
      elements: expr.elements.map((item) => rewriteExprConceptRefs(item, conceptNames)),
    };
  }
  if (expr.type === 'object') {
    return {
      ...expr,
      properties: Object.fromEntries(
        Object.entries(expr.properties).map(([name, value]) => [
          name,
          rewriteExprConceptRefs(value, conceptNames),
        ])
      ),
    };
  }
  if (expr.type === 'member') {
    return { ...expr, object: rewriteExprConceptRefs(expr.object, conceptNames) };
  }
  return expr;
}

function rewriteUnknownExprConceptRefs(expr: unknown, conceptNames: Set<string>): unknown {
  if (!expr || typeof expr !== 'object') return expr;
  return rewriteExprConceptRefs(expr as Expr, conceptNames);
}

function rewriteConditionConceptRefs(condition: unknown, conceptNames: Set<string>): unknown {
  if (!condition || typeof condition !== 'object') return condition;
  const node = condition as any;
  if (node.type === 'simple') {
    return {
      ...node,
      left: rewriteUnknownExprConceptRefs(node.left, conceptNames),
      right: rewriteUnknownExprConceptRefs(node.right, conceptNames),
    };
  }
  if (node.type === 'logical') {
    return {
      ...node,
      conditions: (node.conditions ?? []).map((child: unknown) =>
        rewriteConditionConceptRefs(child, conceptNames)
      ),
    };
  }
  if (node.type === 'not') {
    return { ...node, condition: rewriteConditionConceptRefs(node.condition, conceptNames) };
  }
  if (node.type === 'expression' && node.expression) {
    return {
      ...node,
      expression: rewriteExpressionStringConceptRefs(String(node.expression), conceptNames),
    };
  }
  return condition;
}

function rewriteStepConceptRefs(step: RulesetStep, conceptNames: Set<string>): RulesetStep {
  const cloned = JSON.parse(JSON.stringify(step)) as any;
  if (cloned.type === 'decision') {
    cloned.branches = (cloned.branches ?? []).map((branch: any) => ({
      ...branch,
      condition: rewriteConditionConceptRefs(branch.condition, conceptNames),
    }));
  } else if (cloned.type === 'action') {
    cloned.assignments = (cloned.assignments ?? []).map((assignment: any) => ({
      ...assignment,
      value: rewriteUnknownExprConceptRefs(assignment.value, conceptNames),
    }));
    cloned.externalCalls = (cloned.externalCalls ?? []).map((call: any) => ({
      ...call,
      params: Object.fromEntries(
        Object.entries(call.params ?? {}).map(([name, value]) => [
          name,
          rewriteUnknownExprConceptRefs(value, conceptNames),
        ])
      ),
      fallbackValue: rewriteUnknownExprConceptRefs(call.fallbackValue, conceptNames),
    }));
  } else if (cloned.type === 'terminal') {
    cloned.message = rewriteUnknownExprConceptRefs(cloned.message, conceptNames);
    cloned.output = (cloned.output ?? []).map((output: any) => ({
      ...output,
      value: rewriteUnknownExprConceptRefs(output.value, conceptNames),
    }));
  } else if (cloned.type === 'sub_rule') {
    cloned.bindings = (cloned.bindings ?? []).map((binding: any) => ({
      ...binding,
      expr: rewriteUnknownExprConceptRefs(binding.expr, conceptNames),
    }));
  }
  return cloned as RulesetStep;
}

function addExprRefs(expr: unknown, refs: Set<string>) {
  if (!expr || typeof expr !== 'object') return;
  const node = expr as any;
  if (node.type === 'variable') {
    const path = normalizePath(String(node.path ?? ''));
    if (path && IDENT_RE.test(path)) refs.add(path);
    return;
  }
  if (node.type === 'literal') return;
  for (const value of Object.values(node)) {
    if (Array.isArray(value)) value.forEach((item) => addExprRefs(item, refs));
    else addExprRefs(value, refs);
  }
}

function addExpressionStringRefs(expression: string, refs: Set<string>) {
  const stripped = expression.replace(/"[^"]*"|'[^']*'/g, ' ');
  const reserved = new Set(['true', 'false', 'null', 'undefined', 'and', 'or', 'not', 'in']);
  for (const match of stripped.matchAll(/\$?([A-Za-z_][A-Za-z0-9_.]*)/g)) {
    const name = normalizePath(match[1]);
    const nextToken = stripped.slice((match.index ?? 0) + match[0].length).trimStart();
    if (!IDENT_RE.test(name) || reserved.has(name) || nextToken.startsWith('(')) continue;
    refs.add(name);
  }
}

function addConditionRefs(condition: unknown, refs: Set<string>) {
  if (!condition || typeof condition !== 'object') return;
  const node = condition as any;
  if (node.type === 'simple') {
    addExprRefs(node.left, refs);
    addExprRefs(node.right, refs);
    return;
  }
  if (node.type === 'logical') {
    (node.conditions ?? []).forEach((child: unknown) => addConditionRefs(child, refs));
    return;
  }
  if (node.type === 'not') {
    addConditionRefs(node.condition, refs);
    return;
  }
  if (node.type === 'expression') {
    if (node.parsed) addExprRefs(node.parsed, refs);
    else if (node.expression) addExpressionStringRefs(String(node.expression), refs);
  }
}

function collectStepRefs(steps: RulesetStep[]): Set<string> {
  const refs = new Set<string>();
  for (const step of steps as any[]) {
    if (step.systemGenerated) continue;
    if (step.type === 'decision') {
      for (const branch of step.branches ?? []) addConditionRefs(branch.condition, refs);
    } else if (step.type === 'action') {
      for (const assignment of step.assignments ?? []) addExprRefs(assignment.value, refs);
      for (const call of step.externalCalls ?? []) {
        Object.values(call.params ?? {}).forEach((expr) => addExprRefs(expr, refs));
        addExprRefs(call.fallbackValue, refs);
      }
    } else if (step.type === 'terminal') {
      addExprRefs(step.message, refs);
      for (const output of step.output ?? []) addExprRefs(output.value, refs);
    } else if (step.type === 'sub_rule') {
      for (const binding of step.bindings ?? []) addExprRefs(binding.expr, refs);
    }
  }
  return refs;
}

function conceptExpressionRefs(concept: ConceptDefinition): Set<string> {
  const refs = new Set<string>(concept.dependencies ?? []);
  addExpressionStringRefs(concept.expression, refs);
  return refs;
}

function resolveConceptOrder(
  roots: Set<string>,
  concepts: ConceptDefinition[]
): ConceptDefinition[] {
  const byName = new Map(concepts.map((concept) => [concept.name, concept]));
  const order: ConceptDefinition[] = [];
  const visiting = new Set<string>();
  const visited = new Set<string>();

  function visit(name: string) {
    const concept = byName.get(name);
    if (!concept || visited.has(name)) return;
    if (visiting.has(name)) {
      throw new Error(`Concept dependency cycle detected at "${name}"`);
    }
    visiting.add(name);
    for (const dep of conceptExpressionRefs(concept)) {
      if (byName.has(dep)) visit(dep);
    }
    visiting.delete(name);
    visited.add(name);
    order.push(concept);
  }

  for (const ref of roots) visit(ref);
  return order;
}

function materializeGraphConcepts(
  entryStep: string,
  steps: RulesetStep[],
  concepts: ConceptDefinition[]
): { entryStep: string; steps: RulesetStep[] } {
  const cleanSteps = steps.filter((step) => step.id !== CONCEPT_PRELUDE_ID);
  const conceptNames = new Set(concepts.map((concept) => concept.name));
  const referencedConcepts = new Set(
    [...collectStepRefs(cleanSteps)].filter((name) => conceptNames.has(name))
  );
  const order = resolveConceptOrder(referencedConcepts, concepts);
  const rewrittenSteps = cleanSteps.map((step) => rewriteStepConceptRefs(step, conceptNames));
  if (order.length === 0) return { entryStep, steps: rewrittenSteps };

  const actionStep: RulesetStep = {
    id: CONCEPT_PRELUDE_ID,
    name: 'Compute Concepts',
    type: 'action',
    assignments: order.map((concept) => ({
      name: concept.name,
      value: rewriteExprConceptRefs(parseConceptExpression(concept.expression), conceptNames),
    })),
    nextStepId: entryStep,
    systemGenerated: 'concept_runtime',
  } as RulesetStep;

  return {
    entryStep: CONCEPT_PRELUDE_ID,
    steps: [actionStep, ...rewrittenSteps],
  };
}

export function materializeConceptsForExecution(
  ruleset: RuleSet,
  concepts: ConceptDefinition[]
): RuleSet {
  if (concepts.length === 0) return cloneRuleset(ruleset);

  const next = cloneRuleset(ruleset);
  const main = materializeGraphConcepts(next.startStepId, next.steps, concepts);
  next.startStepId = main.entryStep;
  next.steps = main.steps;

  if (next.subRules) {
    const materializedSubRules: Record<string, SubRuleGraph> = {};
    for (const [name, graph] of Object.entries(next.subRules)) {
      const materialized = materializeGraphConcepts(graph.entryStep, graph.steps, concepts);
      materializedSubRules[name] = {
        ...graph,
        entryStep: materialized.entryStep,
        steps: materialized.steps,
      };
    }
    next.subRules = materializedSubRules;
  }

  return next;
}
