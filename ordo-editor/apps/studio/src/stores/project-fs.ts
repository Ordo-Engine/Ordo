/**
 * Virtual project filesystem.
 *
 * Presents a decision project as a small tree of JSON files and maps reads/writes
 * onto the existing platform entities (ruleset drafts, facts, concepts, tests,
 * contracts). The AI assistant edits the project through these file ops, just like a
 * coding agent over a repo — no new storage, the DB stays the source of truth and the
 * visual editor stays a live view over the same data.
 *
 *   ordo.yaml                  project config + environments (read-only)
 *   rulesets/<name>.json       a ruleset (studio format)
 *   facts.json                 array of fact definitions
 *   concepts.json              array of concept definitions
 *   tests/<ruleset>.json       array of test cases for a ruleset
 *   contracts/<ruleset>.json   the decision contract for a ruleset
 */
import type { RuleSet } from '@ordo-engine/editor-core';
import { rulesetDraftApi, testApi } from '@/api/platform-client';
import { catalogApi } from '@/api/catalog-client';
import type {
  ConceptDefinition,
  DecisionContract,
  DraftConflictResponse,
  FactDefinition,
  TestCaseInput,
} from '@/api/types';
import { useAuthStore } from './auth';
import { useProjectStore } from './project';
import { useEnvironmentStore } from './environment';

export interface FsCtx {
  orgId: string;
  projectId: string;
}

const RULESET_RE = /^rulesets\/(.+)\.json$/;
const TESTS_RE = /^tests\/(.+)\.json$/;
const CONTRACT_RE = /^contracts\/(.+)\.json$/;

function token(): string {
  return useAuthStore().token ?? '';
}
const pretty = (v: unknown) => JSON.stringify(v, null, 2);

/** The project's full file tree. */
export async function listFiles(ctx: FsCtx): Promise<string[]> {
  const paths = ['ordo.yaml', 'facts.json', 'concepts.json'];
  const [rulesets, contracts] = await Promise.all([
    rulesetDraftApi.list(token(), ctx.orgId, ctx.projectId),
    catalogApi.listContracts(token(), ctx.projectId).catch(() => [] as DecisionContract[]),
  ]);
  const withContract = new Set(contracts.map((c) => c.ruleset_name));
  for (const r of rulesets) {
    paths.push(`rulesets/${r.name}.json`);
    paths.push(`tests/${r.name}.json`);
    if (withContract.has(r.name)) paths.push(`contracts/${r.name}.json`);
  }
  return paths;
}

/** Read a file's full contents as pretty JSON text. */
export async function readFile(ctx: FsCtx, path: string): Promise<string> {
  if (path === 'ordo.yaml') {
    const env = useEnvironmentStore();
    if (!env.environments.length) {
      await env.fetchEnvironments(ctx.orgId, ctx.projectId).catch(() => undefined);
    }
    return pretty({
      project: ctx.projectId,
      environments: env.environments.map((e) => ({ id: e.id, name: e.name })),
    });
  }
  if (path === 'facts.json') return pretty(await catalogApi.listFacts(token(), ctx.projectId));
  if (path === 'concepts.json')
    return pretty(await catalogApi.listConcepts(token(), ctx.projectId));

  const ruleset = path.match(RULESET_RE);
  if (ruleset) {
    const name = ruleset[1];
    const open = useProjectStore().openTabs.find((t) => t.name === name);
    if (open) return pretty(open.ruleset);
    const got = await rulesetDraftApi.get(token(), ctx.orgId, ctx.projectId, name);
    return pretty(got.draft);
  }
  const tests = path.match(TESTS_RE);
  if (tests) return pretty(await testApi.list(token(), ctx.projectId, tests[1]));
  const contract = path.match(CONTRACT_RE);
  if (contract) {
    const all = await catalogApi.listContracts(token(), ctx.projectId);
    return pretty(all.find((c) => c.ruleset_name === contract[1]) ?? null);
  }
  throw new Error(`No such file: ${path}`);
}

/** Create or overwrite a file (content is the FULL new content). */
export async function writeFile(ctx: FsCtx, path: string, content: string): Promise<string> {
  const parsed = JSON.parse(content) as unknown;

  if (path === 'ordo.yaml') throw new Error('ordo.yaml is read-only.');
  if (path === 'facts.json') return writeFacts(ctx, parsed as FactDefinition[]);
  if (path === 'concepts.json') return writeConcepts(ctx, parsed as ConceptDefinition[]);

  const ruleset = path.match(RULESET_RE);
  if (ruleset) return writeRuleset(ctx, ruleset[1], parsed as RuleSet);
  const tests = path.match(TESTS_RE);
  if (tests) return writeTests(ctx, tests[1], parsed as TestFileEntry[]);
  const contract = path.match(CONTRACT_RE);
  if (contract) {
    await catalogApi.upsertContract(
      token(),
      ctx.projectId,
      contract[1],
      parsed as Omit<DecisionContract, 'ruleset_name' | 'updated_at'>
    );
    return `wrote ${path}`;
  }
  throw new Error(`Cannot write unknown file: ${path}`);
}

/** Delete a file. Rulesets/contracts are removable; catalog files are not. */
export async function deleteFile(ctx: FsCtx, path: string): Promise<string> {
  const ruleset = path.match(RULESET_RE);
  if (ruleset) {
    await rulesetDraftApi.delete(token(), ctx.orgId, ctx.projectId, ruleset[1]);
    return `deleted ${path}`;
  }
  const contract = path.match(CONTRACT_RE);
  if (contract) {
    await catalogApi.deleteContract(token(), ctx.projectId, contract[1]);
    return `deleted ${path}`;
  }
  throw new Error(`Cannot delete ${path} (only rulesets/* and contracts/* are deletable files).`);
}

/** Substring search across all files. */
export async function grepFiles(ctx: FsCtx, query: string): Promise<string> {
  const files = await listFiles(ctx);
  const hits: string[] = [];
  for (const path of files) {
    try {
      const content = await readFile(ctx, path);
      content.split('\n').forEach((line, i) => {
        if (line.includes(query)) hits.push(`${path}:${i + 1}: ${line.trim()}`);
      });
    } catch {
      // unreadable/empty file — skip
    }
    if (hits.length > 200) break;
  }
  return hits.length ? hits.join('\n') : `No matches for "${query}".`;
}

// ── write-through helpers ────────────────────────────────────────────────────

async function writeRuleset(ctx: FsCtx, name: string, ruleset: RuleSet): Promise<string> {
  const proj = useProjectStore();
  // If the ruleset is the active editor tab, route through the editor's save path so
  // the canvas updates live and the optimistic-lock sequence stays consistent.
  if (proj.activeTab?.name === name) {
    proj.setTabRuleset(name, ruleset, true);
    const conflict = await proj.saveRuleset(name);
    if (conflict) throw new Error('Save conflict — the draft changed; re-read the file and retry.');
    return `wrote rulesets/${name}.json`;
  }
  // Otherwise persist straight to the draft store (creates it if new).
  let seq = 0;
  try {
    const existing = await rulesetDraftApi.get(token(), ctx.orgId, ctx.projectId, name);
    seq = existing.draft_seq;
  } catch {
    seq = 0; // new ruleset
  }
  const res = await rulesetDraftApi.save(token(), ctx.orgId, ctx.projectId, name, {
    ruleset,
    expected_seq: seq,
  });
  if ((res as DraftConflictResponse).conflict) {
    throw new Error('Save conflict — re-read the file and retry.');
  }
  const open = proj.openTabs.find((t) => t.name === name);
  if (open) proj.setTabRuleset(name, ruleset, false);
  return `wrote rulesets/${name}.json`;
}

async function writeFacts(ctx: FsCtx, next: FactDefinition[]): Promise<string> {
  const cur = await catalogApi.listFacts(token(), ctx.projectId);
  const keep = new Set(next.map((f) => f.name));
  for (const f of next) await catalogApi.upsertFact(token(), ctx.projectId, f);
  for (const f of cur) {
    if (!keep.has(f.name)) await catalogApi.deleteFact(token(), ctx.projectId, f.name);
  }
  return `wrote facts.json (${next.length} fact(s))`;
}

async function writeConcepts(ctx: FsCtx, next: ConceptDefinition[]): Promise<string> {
  const cur = await catalogApi.listConcepts(token(), ctx.projectId);
  const keep = new Set(next.map((c) => c.name));
  for (const c of next) await catalogApi.upsertConcept(token(), ctx.projectId, c);
  for (const c of cur) {
    if (!keep.has(c.name)) await catalogApi.deleteConcept(token(), ctx.projectId, c.name);
  }
  return `wrote concepts.json (${next.length} concept(s))`;
}

interface TestFileEntry {
  name: string;
  description?: string;
  input?: Record<string, unknown>;
  expect?: TestCaseInput['expect'];
  tags?: string[];
}

async function writeTests(ctx: FsCtx, ruleset: string, next: TestFileEntry[]): Promise<string> {
  const cur = await testApi.list(token(), ctx.projectId, ruleset);
  const byName = new Map(cur.map((t) => [t.name, t]));
  const keptIds = new Set<string>();
  for (const tc of next) {
    const input: TestCaseInput = {
      name: tc.name,
      description: tc.description,
      input: tc.input ?? {},
      expect: tc.expect ?? {},
      tags: tc.tags,
    };
    const existing = byName.get(tc.name);
    if (existing) {
      await testApi.update(token(), ctx.projectId, ruleset, existing.id, input);
      keptIds.add(existing.id);
    } else {
      const created = await testApi.create(token(), ctx.projectId, ruleset, input);
      keptIds.add(created.id);
    }
  }
  for (const t of cur) {
    if (!keptIds.has(t.id)) await testApi.delete(token(), ctx.projectId, ruleset, t.id);
  }
  return `wrote tests/${ruleset}.json (${next.length} test(s))`;
}
