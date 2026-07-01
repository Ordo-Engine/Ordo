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

export interface ToolActivity {
  id: string;
  /** Tool name (e.g. `write_file`); the UI derives a human verb from it. */
  name: string;
  /** The target (file path / query), shown dimmed + monospace. */
  target: string;
  status: 'running' | 'ok' | 'error';
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
/** An `ask_question` tool call awaiting the user's choice. */
export interface PendingQuestion {
  question: string;
  options: string[];
  resolve: (answer: string) => void;
}

export type AiMode = 'agent' | 'ask';

export type PlanStatus = 'pending' | 'in_progress' | 'completed';
/** One item in the agent's task checklist (set via the `update_plan` tool). */
export interface PlanItem {
  content: string;
  status: PlanStatus;
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
  /** An `ask_question` tool call awaiting the user's answer. */
  const pendingQuestion = ref<PendingQuestion | null>(null);
  /** "agent" = full tools; "ask" = read-only Q&A. */
  const mode = ref<AiMode>('agent');
  /** High-risk tools the user chose to always allow this session. */
  const allowlist = ref<Set<string>>(new Set());
  /** Before-state of each ruleset the AI edited this session (name → ruleset), so
   * individual changes can be reverted (edit review). */
  const fileSnapshots = ref<Map<string, RuleSet>>(new Map());
  /** Files the assistant has touched this session (for the sidebar's review list). */
  const touchedFiles = ref<string[]>([]);
  /** Files the user pinned as @context — their content is added to every request. */
  const contextFiles = ref<string[]>([]);
  /** The agent's live task checklist (set via `update_plan`). */
  const plan = ref<PlanItem[]>([]);

  // AbortController for the in-flight streaming request (the Stop button).
  let abortCtrl: AbortController | null = null;

  const transcript = ref<AiChatMessage[]>([]);
  let ctx: fs.FsCtx = { orgId: '', projectId: '' };

  const ready = computed(() => !!provider.value && !!modelId.value);
  /** Whether a given file path can be reverted (has a before-snapshot). */
  function canRevert(path: string): boolean {
    const m = path.match(RULESET_RE);
    return !!m && fileSnapshots.value.has(m[1]);
  }

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
    fileSnapshots.value.clear();
    contextFiles.value = [];
    plan.value = [];
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
    const pinned: Array<{ path: string; content: string }> = [];
    for (const p of contextFiles.value) {
      try {
        pinned.push({ path: p, content: await fs.readFile(ctx, p) });
      } catch {
        // skip a pinned file that can't be read
      }
    }
    return {
      files,
      openFile: tab ? `rulesets/${tab.name}.json` : null,
      openFileContent: tab?.ruleset ?? null,
      pinnedFiles: pinned,
    };
  }

  async function listProjectFiles(): Promise<string[]> {
    try {
      return await fs.listFiles(ctx);
    } catch {
      return [];
    }
  }
  function pinFile(path: string) {
    if (path && !contextFiles.value.includes(path)) contextFiles.value.push(path);
  }
  function unpinFile(path: string) {
    contextFiles.value = contextFiles.value.filter((f) => f !== path);
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

  /** Stop the running agent (aborts the in-flight stream). */
  function stop() {
    abortCtrl?.abort();
    if (pending.value) rejectPending();
    if (pendingQuestion.value) {
      pendingQuestion.value.resolve('(cancelled)');
      pendingQuestion.value = null;
    }
  }

  async function runLoop() {
    running.value = true;
    abortCtrl = new AbortController();
    try {
      for (let round = 0; round < MAX_ROUNDS; round++) {
        const more = await streamTurn();
        if (!more) return;
      }
      messages.value.push({
        role: 'assistant',
        text: '(Reached the tool-call limit for this message. Ask me to continue if needed.)',
        tools: [],
      });
    } catch (e: unknown) {
      if (abortCtrl?.signal.aborted) {
        messages.value.push({ role: 'assistant', text: '(Stopped.)', tools: [] });
      } else {
        error.value = msg(e);
      }
    } finally {
      running.value = false;
      abortCtrl = null;
    }
  }

  /** Stream one assistant turn; returns true if tools ran and we should loop again. */
  async function streamTurn(): Promise<boolean> {
    const display: DisplayMessage = { role: 'assistant', text: '', tools: [] };
    messages.value.push(display);
    const toolCalls: AiToolCall[] = [];
    let stopReason = 'end_turn';

    await aiApi.chatStream(
      auth.token!,
      {
        provider: provider.value,
        model: modelId.value,
        messages: transcript.value,
        context: await buildContext(),
        mode: mode.value,
      },
      (ev) => {
        if (ev.type === 'text') {
          display.text += ev.text;
        } else if (ev.type === 'tool_start') {
          display.tools.push({ id: ev.id, name: ev.name, target: '', status: 'running' });
        } else if (ev.type === 'tool') {
          const call: AiToolCall = { id: ev.id, name: ev.name, input: ev.input };
          toolCalls.push(call);
          const badge = display.tools.find((t) => t.id === ev.id);
          if (badge) badge.target = toolTarget(call);
          else
            display.tools.push({
              id: ev.id,
              name: ev.name,
              target: toolTarget(call),
              status: 'running',
            });
        } else if (ev.type === 'done') {
          stopReason = ev.stop_reason;
        } else if (ev.type === 'error') {
          error.value = ev.message;
        }
      },
      abortCtrl?.signal
    );

    transcript.value.push({ role: 'assistant', content: display.text, tool_calls: toolCalls });
    if (stopReason !== 'tool_use' || toolCalls.length === 0) return false;

    const results = [];
    for (const call of toolCalls) {
      const r = await executeTool(call);
      const badge = display.tools.find((t) => t.id === call.id);
      if (badge) badge.status = r.is_error ? 'error' : 'ok';
      results.push({ tool_call_id: call.id, content: r.content, is_error: r.is_error });
    }
    transcript.value.push({ role: 'tool', tool_results: results });
    return true;
  }

  function isHighRisk(call: AiToolCall): boolean {
    if (call.name === 'publish') return true;
    if (call.name === 'delete_file') return RULESET_RE.test(String(call.input?.path ?? ''));
    return false;
  }

  async function executeTool(call: AiToolCall): Promise<{ content: string; is_error: boolean }> {
    try {
      // Human-in-the-loop question — pause for the user's choice.
      if (call.name === 'ask_question') {
        const input = call.input ?? {};
        const answer = await new Promise<string>((resolve) => {
          pendingQuestion.value = {
            question: String(input.question ?? ''),
            options: Array.isArray(input.options) ? (input.options as unknown[]).map(String) : [],
            resolve,
          };
        });
        return { content: `The user chose: ${answer}`, is_error: false };
      }
      // High-risk — confirm unless the user chose "always allow" this session.
      if (isHighRisk(call) && !allowlist.value.has(call.name)) {
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
        await snapshotBeforeWrite(path);
        const out = await fs.writeFile(ctx, path, String(input.content));
        markTouched(path);
        return out;
      }
      case 'delete_file': {
        const out = await fs.deleteFile(ctx, String(input.path));
        markTouched(String(input.path));
        return out;
      }
      case 'update_plan': {
        const todos = Array.isArray(input.todos) ? input.todos : [];
        plan.value = todos.map((t) => {
          const item = (t ?? {}) as Record<string, unknown>;
          const status = String(item.status ?? 'pending');
          return {
            content: String(item.content ?? ''),
            status: (['pending', 'in_progress', 'completed'].includes(status)
              ? status
              : 'pending') as PlanStatus,
          };
        });
        return 'ok';
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
  /** Approve + remember: don't ask again for this tool this session. */
  function approvePendingAlways() {
    if (pending.value) allowlist.value.add(pending.value.call.name);
    approvePending();
  }

  // ── ask_question (human-in-the-loop) ──
  function answerQuestion(option: string) {
    const q = pendingQuestion.value;
    if (!q) return;
    pendingQuestion.value = null;
    q.resolve(option);
  }

  function setMode(m: AiMode) {
    mode.value = m;
  }

  // ── changed-file tracking + per-turn undo ──
  function markTouched(path: string) {
    if (path && !touchedFiles.value.includes(path)) touchedFiles.value.push(path);
  }
  /** Capture a ruleset's before-state (once) so an AI edit can be reverted. */
  async function snapshotBeforeWrite(path: string) {
    const m = path.match(RULESET_RE);
    if (!m) return;
    const name = m[1];
    if (fileSnapshots.value.has(name)) return; // keep the earliest before-state
    try {
      fileSnapshots.value.set(name, JSON.parse(await fs.readFile(ctx, path)) as RuleSet);
    } catch {
      // new file — no before-state to snapshot
    }
  }
  /** Revert one file to its before-state (reject the AI's change). */
  async function revertFile(path: string) {
    const m = path.match(RULESET_RE);
    if (!m) return;
    const snap = fileSnapshots.value.get(m[1]);
    if (!snap) return;
    try {
      await fs.writeFile(ctx, path, JSON.stringify(snap));
      fileSnapshots.value.delete(m[1]);
      touchedFiles.value = touchedFiles.value.filter((f) => f !== path);
    } catch (e: unknown) {
      error.value = msg(e);
    }
  }
  /** Keep one file's change (accept): drop its revert snapshot. */
  function keepFile(path: string) {
    const m = path.match(RULESET_RE);
    if (m) fileSnapshots.value.delete(m[1]);
    touchedFiles.value = touchedFiles.value.filter((f) => f !== path);
  }

  return {
    providers,
    provider,
    modelId,
    messages,
    running,
    error,
    pending,
    pendingQuestion,
    mode,
    touchedFiles,
    contextFiles,
    plan,
    ready,
    canRevert,
    init,
    listProjectFiles,
    pinFile,
    unpinFile,
    selectModel,
    reset,
    send,
    stop,
    setMode,
    approvePending,
    approvePendingAlways,
    rejectPending,
    answerQuestion,
    revertFile,
    keepFile,
  };
});

/** The target of a tool call (file path / ruleset / query), for the dimmed detail. */
function toolTarget(call: AiToolCall): string {
  const i = call.input ?? {};
  if (typeof i.path === 'string') return i.path;
  if (typeof i.ruleset === 'string') return i.ruleset;
  if (typeof i.query === 'string') return `"${i.query}"`;
  return '';
}
