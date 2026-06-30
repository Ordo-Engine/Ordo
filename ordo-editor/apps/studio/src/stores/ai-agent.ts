/**
 * AI rule assistant — the agentic loop.
 *
 * The browser drives the loop: it sends the transcript + live editor context to
 * the server-side `/ai/chat` proxy, receives one assistant turn (text + tool
 * calls), executes the tool calls locally (reads project state, edits the open
 * ruleset on the canvas), feeds the results back, and repeats until the assistant
 * stops. Edits land live and are reversible (a per-turn snapshot powers "undo this
 * AI change"). High-risk tools (publish / delete) pause the loop for explicit user
 * approval.
 */
import { defineStore } from 'pinia';
import { ref, computed, toRaw } from 'vue';
import type { RuleSet } from '@ordo-engine/editor-core';
import { aiApi, rulesetDraftApi } from '@/api/platform-client';
import { catalogApi } from '@/api/catalog-client';
import type { AiChatMessage, AiProviderOption, AiToolCall } from '@/api/ai-types';
import { useAuthStore } from './auth';
import { useProjectStore } from './project';

/** A high-risk tool whose names require explicit user approval before running. */
const HIGH_RISK = new Set(['publish_ruleset', 'delete_ruleset']);
/** Safety cap on tool-call rounds per user message, then we checkpoint and pause. */
const MAX_ROUNDS = 12;

interface ToolActivity {
  name: string;
  ok: boolean;
  detail?: string;
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

  // The normalized transcript sent to the model (separate from the display list).
  const transcript = ref<AiChatMessage[]>([]);

  let ctx: { orgId: string; projectId: string } = { orgId: '', projectId: '' };

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
      error.value = e instanceof Error ? e.message : String(e);
    }
  }

  function selectModel(p: string, m: string) {
    provider.value = p;
    modelId.value = m;
  }

  function reset() {
    messages.value = [];
    transcript.value = [];
    error.value = null;
    pending.value = null;
  }

  /** Build the live context the server folds into the system prompt. */
  function buildContext(): Record<string, unknown> {
    const tab = project.activeTab;
    return {
      ruleset: tab?.ruleset ?? null,
      activeRulesetName: tab?.name ?? null,
      otherRulesets: project.openTabs.map((t) => t.name).filter((n) => n !== tab?.name),
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
          context: buildContext(),
        });

        // Record the assistant turn (display + transcript).
        const display: DisplayMessage = { role: 'assistant', text: resp.content, tools: [] };
        messages.value.push(display);
        transcript.value.push({
          role: 'assistant',
          content: resp.content,
          tool_calls: resp.tool_calls,
        });

        if (resp.stop_reason !== 'tool_use' || resp.tool_calls.length === 0) {
          return; // assistant is done
        }

        // Execute every tool call, collect results, feed back as one tool message.
        const results = [];
        for (const call of resp.tool_calls) {
          const r = await executeTool(call);
          display.tools.push({ name: call.name, ok: !r.is_error, detail: r.detail });
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
      error.value = e instanceof Error ? e.message : String(e);
    } finally {
      running.value = false;
    }
  }

  /** Execute one tool call locally. Returns the result string for the model. */
  async function executeTool(
    call: AiToolCall
  ): Promise<{ content: string; is_error: boolean; detail?: string }> {
    try {
      if (HIGH_RISK.has(call.name)) {
        // Pause for explicit user approval (resolved by approve/reject).
        const content = await new Promise<string>((resolve) => {
          pending.value = { call, resolve };
        });
        return { content, is_error: false, detail: 'awaited confirmation' };
      }

      const out = await runTool(call);
      return { content: out, is_error: false };
    } catch (e: unknown) {
      return { content: e instanceof Error ? e.message : String(e), is_error: true };
    }
  }

  /** The actual read/edit dispatch (non-high-risk, or post-approval). */
  async function runTool(call: AiToolCall): Promise<string> {
    const input = call.input ?? {};
    const tab = project.activeTab;
    const token = auth.token!;

    switch (call.name) {
      // ── reads ──
      case 'get_ruleset':
        return JSON.stringify(tab?.ruleset ?? null);
      case 'list_rulesets': {
        const metas = await rulesetDraftApi.list(token, ctx.orgId, ctx.projectId);
        return JSON.stringify(metas.map((m) => m.name));
      }
      case 'get_other_ruleset': {
        const got = await rulesetDraftApi.get(token, ctx.orgId, ctx.projectId, String(input.name));
        return JSON.stringify(got.draft);
      }
      case 'list_facts':
        return JSON.stringify(await catalogApi.listFacts(token, ctx.projectId));
      case 'list_concepts':
        return JSON.stringify(await catalogApi.listConcepts(token, ctx.projectId));
      case 'validate_ruleset': {
        if (!tab) return 'No ruleset is open.';
        try {
          await rulesetDraftApi.convert(token, ctx.orgId, ctx.projectId, tab.name, tab.ruleset);
          return 'valid';
        } catch (e: unknown) {
          return `invalid: ${e instanceof Error ? e.message : String(e)}`;
        }
      }

      // ── edits (current ruleset) ──
      case 'update_ruleset_config':
      case 'set_start_step':
      case 'add_step':
      case 'update_step':
      case 'remove_step':
      case 'add_branch':
      case 'update_branch':
      case 'remove_branch':
      case 'replace_ruleset': {
        if (!tab) return 'No ruleset is open to edit.';
        snapshotForUndo(tab.name, tab.ruleset);
        const next = applyEdit(tab.ruleset, call.name, input);
        project.setTabRuleset(tab.name, next, true);
        return 'ok';
      }

      // ── catalog edits ──
      case 'upsert_fact':
        await catalogApi.upsertFact(token, ctx.projectId, input.fact as never);
        return 'ok';
      case 'upsert_concept':
        await catalogApi.upsertConcept(token, ctx.projectId, input.concept as never);
        return 'ok';

      // ── high-risk (only reached post-approval) ──
      case 'publish_ruleset': {
        if (!tab) return 'No ruleset is open to publish.';
        const dep = await rulesetDraftApi.publish(token, ctx.orgId, ctx.projectId, tab.name, {
          environment_id: String(input.environmentId),
          release_note: input.releaseNote ? String(input.releaseNote) : undefined,
        });
        return `published (deployment ${dep.id}, status ${dep.status})`;
      }
      case 'delete_ruleset':
        await rulesetDraftApi.delete(token, ctx.orgId, ctx.projectId, String(input.name));
        return 'deleted';

      default:
        return `Unknown tool: ${call.name}`;
    }
  }

  // ── high-risk approval ──
  function approvePending() {
    const p = pending.value;
    if (!p) return;
    pending.value = null;
    runTool(p.call)
      .then((out) => p.resolve(out))
      .catch((e) => p.resolve(`error: ${e instanceof Error ? e.message : String(e)}`));
  }
  function rejectPending() {
    const p = pending.value;
    if (!p) return;
    pending.value = null;
    p.resolve('The user declined this action.');
  }

  // ── per-turn undo ──
  function snapshotForUndo(name: string, ruleset: RuleSet) {
    if (undoSnapshot.value && undoSnapshot.value.name === name) return; // keep earliest in a turn
    undoSnapshot.value = { name, ruleset: structuredClone(toRaw(ruleset)) };
  }
  function undoLastChange() {
    const snap = undoSnapshot.value;
    if (!snap) return;
    project.setTabRuleset(snap.name, snap.ruleset, true);
    undoSnapshot.value = null;
  }

  return {
    providers,
    provider,
    modelId,
    messages,
    running,
    error,
    pending,
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

/** Apply a structured edit to a studio RuleSet and return a new ruleset. */
function applyEdit(current: RuleSet, tool: string, input: Record<string, unknown>): RuleSet {
  const rs = structuredClone(toRaw(current)) as unknown as {
    config: Record<string, unknown>;
    startStepId?: string;
    steps: Array<Record<string, unknown>>;
  };
  rs.steps = rs.steps ?? [];

  const findStep = (id: string) => rs.steps.find((s) => s.id === id);

  switch (tool) {
    case 'replace_ruleset':
      return structuredClone(input.ruleset) as RuleSet;

    case 'update_ruleset_config':
      rs.config = { ...rs.config, ...input };
      return rs as unknown as RuleSet;

    case 'set_start_step':
      rs.startStepId = String(input.stepId);
      return rs as unknown as RuleSet;

    case 'add_step': {
      const step = input.step as Record<string, unknown>;
      rs.steps.push(step);
      if (input.setAsStart) rs.startStepId = String(step.id);
      return rs as unknown as RuleSet;
    }

    case 'update_step': {
      const s = findStep(String(input.stepId));
      if (s) Object.assign(s, input.updates as object, { id: s.id, type: s.type });
      return rs as unknown as RuleSet;
    }

    case 'remove_step': {
      const id = String(input.stepId);
      rs.steps = rs.steps.filter((s) => s.id !== id);
      // Clean dangling references.
      for (const s of rs.steps) {
        if (Array.isArray(s.branches)) {
          s.branches = (s.branches as Array<Record<string, unknown>>).filter(
            (b) => b.nextStepId !== id
          );
        }
        if (s.defaultNextStepId === id) s.defaultNextStepId = undefined;
        if (s.nextStepId === id) s.nextStepId = undefined;
      }
      if (rs.startStepId === id) rs.startStepId = rs.steps[0]?.id as string | undefined;
      return rs as unknown as RuleSet;
    }

    case 'add_branch': {
      const s = findStep(String(input.stepId));
      if (s) {
        const branches = (s.branches as Array<unknown>) ?? [];
        branches.push(input.branch);
        s.branches = branches;
      }
      return rs as unknown as RuleSet;
    }

    case 'update_branch': {
      const s = findStep(String(input.stepId));
      const branches = (s?.branches as Array<Record<string, unknown>>) ?? [];
      const b = branches.find((x) => x.id === input.branchId);
      if (b) Object.assign(b, input.updates as object, { id: b.id });
      return rs as unknown as RuleSet;
    }

    case 'remove_branch': {
      const s = findStep(String(input.stepId));
      if (s && Array.isArray(s.branches)) {
        s.branches = (s.branches as Array<Record<string, unknown>>).filter(
          (b) => b.id !== input.branchId
        );
      }
      return rs as unknown as RuleSet;
    }

    default:
      return rs as unknown as RuleSet;
  }
}
