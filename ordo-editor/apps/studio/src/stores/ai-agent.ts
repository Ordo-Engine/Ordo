/**
 * AI rule assistant — the agentic loop over the virtual project filesystem.
 *
 * The browser drives the loop: it sends the transcript + a snapshot of the project
 * (file tree + open file) to the server-side `/ai/chat` proxy, receives one assistant
 * turn (text + tool calls), executes the file tools locally against `project-fs`
 * (read/write/delete files, grep, validate, run tests), feeds the results back, and
 * repeats until the assistant stops. Ruleset writes land live on the canvas and are
 * reversible (a per-turn snapshot powers "undo this AI change"). High-risk tools
 * (publish, deleting a ruleset file) pause the loop for explicit user approval.
 */
import { defineStore } from 'pinia';
import { ref, computed, toRaw } from 'vue';
import type { RuleSet } from '@ordo-engine/editor-core';
import { aiApi, rulesetDraftApi, testApi } from '@/api/platform-client';
import type { AiChatMessage, AiProviderOption, AiToolCall } from '@/api/ai-types';
import { useAuthStore } from './auth';
import { useProjectStore } from './project';
import * as fs from './project-fs';

/** Safety cap on tool-call rounds per user message, then we checkpoint and pause. */
const MAX_ROUNDS = 12;
const RULESET_RE = /^rulesets\/(.+)\.json$/;

interface ToolActivity {
  name: string;
  ok: boolean;
}
/** A rendered chat turn (what the sidebar shows). */
export interface DisplayMessage {
  role: 'user' | 'assistant';
  text: string;
  tools: ToolActivity[];
}

interface PendingConfirm {
  call: AiToolCall;
  resolve: (result: string) => void;
}

function msg(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

export const useAiStore = defineStore('ai', () => {
  const auth = useAuthStore();
  const project = useProjectStore();

  const providers = ref<AiProviderOption[]>([]);
  const provider = ref('');
  const modelId = ref('');
  const messages = ref<DisplayMessage[]>([]);
  const running = ref(false);
  const error = ref<string | null>(null);
  const pending = ref<PendingConfirm | null>(null);
  /** Ruleset snapshot taken before the latest AI edits, for one-click undo. */
  const undoSnapshot = ref<{ name: string; ruleset: RuleSet } | null>(null);
  /** Files the assistant has touched this session (for the sidebar's changed list). */
  const touchedFiles = ref<string[]>([]);

  const transcript = ref<AiChatMessage[]>([]);
  let ctx: fs.FsCtx = { orgId: '', projectId: '' };

  const ready = computed(() => !!provider.value && !!modelId.value);
  const canUndo = computed(() => undoSnapshot.value !== null);

  async function init(orgId: string, projectId: string) {
    ctx = { orgId, projectId };
    if (providers.value.length || !auth.token) return;
    try {
      providers.value = await aiApi.listModels(auth.token);
      if (providers.value.length) {
        provider.value = providers.value[0].id;
        modelId.value = providers.value[0].models[0]?.id ?? '';
      }
    } catch (e: unknown) {
      error.value = msg(e);
    }
  }

  function selectModel(p: string, m: string) {
    provider.value = p;
    modelId.value = m;
  }

  function reset() {
    messages.value = [];
    transcript.value = [];
    touchedFiles.value = [];
    error.value = null;
    pending.value = null;
  }

  /** A snapshot of the project the server folds into the system prompt. */
  async function buildContext(): Promise<Record<string, unknown>> {
    const tab = project.activeTab;
    let files: string[] = [];
    try {
      files = await fs.listFiles(ctx);
    } catch {
      // best-effort; the AI can also call list_files
    }
    return {
      files,
      openFile: tab ? `rulesets/${tab.name}.json` : null,
      openFileContent: tab?.ruleset ?? null,
    };
  }

  async function send(text: string) {
    if (!ready.value || running.value || !auth.token) return;
    const trimmed = text.trim();
    if (!trimmed) return;
    error.value = null;
    messages.value.push({ role: 'user', text: trimmed, tools: [] });
    transcript.value.push({ role: 'user', content: trimmed });
    await runLoop();
  }

  async function runLoop() {
    running.value = true;
    try {
      for (let round = 0; round < MAX_ROUNDS; round++) {
        const resp = await aiApi.chat(auth.token!, {
          provider: provider.value,
          model: modelId.value,
          messages: transcript.value,
          context: await buildContext(),
        });

        const display: DisplayMessage = { role: 'assistant', text: resp.content, tools: [] };
        messages.value.push(display);
        transcript.value.push({
          role: 'assistant',
          content: resp.content,
          tool_calls: resp.tool_calls,
        });

        if (resp.stop_reason !== 'tool_use' || resp.tool_calls.length === 0) return;

        const results = [];
        for (const call of resp.tool_calls) {
          const r = await executeTool(call);
          display.tools.push({ name: toolLabel(call), ok: !r.is_error });
          results.push({ tool_call_id: call.id, content: r.content, is_error: r.is_error });
        }
        transcript.value.push({ role: 'tool', tool_results: results });
      }
      messages.value.push({
        role: 'assistant',
        text: '(Reached the tool-call limit for this message. Ask me to continue if needed.)',
        tools: [],
      });
    } catch (e: unknown) {
      error.value = msg(e);
    } finally {
      running.value = false;
    }
  }

  function isHighRisk(call: AiToolCall): boolean {
    if (call.name === 'publish') return true;
    if (call.name === 'delete_file') return RULESET_RE.test(String(call.input?.path ?? ''));
    return false;
  }

  async function executeTool(call: AiToolCall): Promise<{ content: string; is_error: boolean }> {
    try {
      if (isHighRisk(call)) {
        const content = await new Promise<string>((resolve) => {
          pending.value = { call, resolve };
        });
        return { content, is_error: false };
      }
      return { content: await runTool(call), is_error: false };
    } catch (e: unknown) {
      return { content: msg(e), is_error: true };
    }
  }

  /** Dispatch a (non-high-risk, or approved) tool call to the project filesystem. */
  async function runTool(call: AiToolCall): Promise<string> {
    const input = call.input ?? {};
    switch (call.name) {
      case 'list_files':
        return JSON.stringify(await fs.listFiles(ctx));
      case 'read_file':
        return fs.readFile(ctx, String(input.path));
      case 'write_file': {
        const path = String(input.path);
        snapshotForUndo(path);
        const out = await fs.writeFile(ctx, path, String(input.content));
        markTouched(path);
        return out;
      }
      case 'delete_file': {
        const out = await fs.deleteFile(ctx, String(input.path));
        markTouched(String(input.path));
        return out;
      }
      case 'grep':
        return fs.grepFiles(ctx, String(input.query));
      case 'validate':
        return validateRuleset(String(input.path));
      case 'run_tests':
        return runRulesetTests(String(input.ruleset));
      case 'publish':
        return publishRuleset(
          String(input.ruleset),
          String(input.environmentId),
          input.releaseNote
        );
      default:
        return `Unknown tool: ${call.name}`;
    }
  }

  async function validateRuleset(path: string): Promise<string> {
    const m = path.match(RULESET_RE);
    if (!m) return 'validate expects a rulesets/<name>.json path.';
    const ruleset = JSON.parse(await fs.readFile(ctx, path)) as RuleSet;
    try {
      await rulesetDraftApi.convert(auth.token!, ctx.orgId, ctx.projectId, m[1], ruleset);
      return 'valid';
    } catch (e: unknown) {
      return `invalid: ${msg(e)}`;
    }
  }

  async function runRulesetTests(ruleset: string): Promise<string> {
    const results = await testApi.runAll(auth.token!, ctx.projectId, ruleset);
    const passed = results.filter((r) => r.passed).length;
    return (
      `${passed}/${results.length} passed\n` +
      JSON.stringify(
        results.map((r) => ({ test: r.test_name, passed: r.passed, failures: r.failures }))
      )
    );
  }

  async function publishRuleset(
    ruleset: string,
    environmentId: string,
    note: unknown
  ): Promise<string> {
    const dep = await rulesetDraftApi.publish(auth.token!, ctx.orgId, ctx.projectId, ruleset, {
      environment_id: environmentId,
      release_note: note ? String(note) : undefined,
    });
    return `published ${ruleset} (deployment ${dep.id}, status ${dep.status})`;
  }

  // ── high-risk approval ──
  function approvePending() {
    const p = pending.value;
    if (!p) return;
    pending.value = null;
    runTool(p.call)
      .then((out) => {
        if (p.call.name === 'delete_file' || p.call.name === 'write_file') {
          markTouched(String(p.call.input?.path ?? ''));
        }
        p.resolve(out);
      })
      .catch((e) => p.resolve(`error: ${msg(e)}`));
  }
  function rejectPending() {
    const p = pending.value;
    if (!p) return;
    pending.value = null;
    p.resolve('The user declined this action.');
  }

  // ── changed-file tracking + per-turn undo ──
  function markTouched(path: string) {
    if (path && !touchedFiles.value.includes(path)) touchedFiles.value.push(path);
  }
  function snapshotForUndo(path: string) {
    const m = path.match(RULESET_RE);
    if (!m) return;
    const name = m[1];
    if (undoSnapshot.value && undoSnapshot.value.name === name) return; // keep earliest in a turn
    const tab = project.openTabs.find((t) => t.name === name);
    if (tab) undoSnapshot.value = { name, ruleset: structuredClone(toRaw(tab.ruleset)) };
  }
  async function undoLastChange() {
    const snap = undoSnapshot.value;
    if (!snap) return;
    undoSnapshot.value = null;
    try {
      await fs.writeFile(ctx, `rulesets/${snap.name}.json`, JSON.stringify(snap.ruleset));
    } catch (e: unknown) {
      error.value = msg(e);
    }
  }

  return {
    providers,
    provider,
    modelId,
    messages,
    running,
    error,
    pending,
    touchedFiles,
    ready,
    canUndo,
    init,
    selectModel,
    reset,
    send,
    approvePending,
    rejectPending,
    undoLastChange,
  };
});

/** Short label for a tool-call badge (e.g. "write rulesets/x.json"). */
function toolLabel(call: AiToolCall): string {
  const path = call.input?.path;
  if (typeof path === 'string') return `${call.name} ${path}`;
  const ruleset = call.input?.ruleset;
  if (typeof ruleset === 'string') return `${call.name} ${ruleset}`;
  return call.name;
}
