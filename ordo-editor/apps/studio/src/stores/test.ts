import { defineStore } from 'pinia';
import { ref } from 'vue';
import type { RuleSet } from '@ordo-engine/editor-core';
import { rulesetDraftApi, testApi } from '@/api/platform-client';
import { normalizeRuleset } from '@/utils/ruleset';
import { useAuthStore } from './auth';
import type { ProjectTestRunResult, TestCase, TestCaseInput, TestRunResult } from '@/api/types';

export const useTestStore = defineStore('test', () => {
  const auth = useAuthStore();

  // Ruleset-level state (keyed by rulesetName)
  const testsByRuleset = ref<Map<string, TestCase[]>>(new Map());
  const runResults = ref<Map<string, TestRunResult[]>>(new Map());
  const loadingRuleset = ref<Map<string, boolean>>(new Map());
  const running = ref(false);

  // Project-level state
  const projectRunResult = ref<ProjectTestRunResult | null>(null);
  const projectRunning = ref(false);

  async function buildPlatformRuleset(
    orgId: string,
    projectId: string,
    rulesetName: string
  ): Promise<Record<string, unknown>> {
    if (!auth.token) throw new Error('Not authenticated');
    // Platform owns studio -> engine conversion. Fetch the draft in studio
    // format, normalize it on the client, then ask platform to convert it
    // before invoking the test execution endpoints.
    const draft = await rulesetDraftApi.get(auth.token, orgId, projectId, rulesetName);
    const studioRuleset: RuleSet = normalizeRuleset(draft.draft, rulesetName);
    return rulesetDraftApi.convert(auth.token, orgId, projectId, rulesetName, studioRuleset);
  }

  // ── Ruleset-level operations ──────────────────────────────────────────────

  async function fetchTests(projectId: string, rulesetName: string): Promise<void> {
    if (!auth.token) return;
    loadingRuleset.value.set(rulesetName, true);
    try {
      const tests = await testApi.list(auth.token, projectId, rulesetName);
      testsByRuleset.value.set(rulesetName, tests);
    } finally {
      loadingRuleset.value.set(rulesetName, false);
    }
  }

  async function createTest(
    projectId: string,
    rulesetName: string,
    input: TestCaseInput
  ): Promise<TestCase | undefined> {
    if (!auth.token) return;
    const tc = await testApi.create(auth.token, projectId, rulesetName, input);
    const list = testsByRuleset.value.get(rulesetName) ?? [];
    testsByRuleset.value.set(rulesetName, [...list, tc]);
    return tc;
  }

  async function updateTest(
    projectId: string,
    rulesetName: string,
    id: string,
    input: TestCaseInput
  ): Promise<TestCase | undefined> {
    if (!auth.token) return;
    const tc = await testApi.update(auth.token, projectId, rulesetName, id, input);
    const list = testsByRuleset.value.get(rulesetName) ?? [];
    testsByRuleset.value.set(
      rulesetName,
      list.map((t) => (t.id === id ? tc : t))
    );
    return tc;
  }

  async function deleteTest(projectId: string, rulesetName: string, id: string): Promise<void> {
    if (!auth.token) return;
    await testApi.delete(auth.token, projectId, rulesetName, id);
    const list = testsByRuleset.value.get(rulesetName) ?? [];
    testsByRuleset.value.set(
      rulesetName,
      list.filter((t) => t.id !== id)
    );
    // Clear stale run result for this test
    const results = runResults.value.get(rulesetName) ?? [];
    runResults.value.set(
      rulesetName,
      results.filter((r) => r.test_id !== id)
    );
  }

  async function runTests(orgId: string, projectId: string, rulesetName: string): Promise<void> {
    if (!auth.token) return;
    running.value = true;
    try {
      const ruleset = await buildPlatformRuleset(orgId, projectId, rulesetName);
      const results = await testApi.runAll(auth.token, projectId, rulesetName, {
        ruleset,
        include_trace: true,
      });
      runResults.value.set(rulesetName, results);
    } finally {
      running.value = false;
    }
  }

  const runningOne = ref<Set<string>>(new Set());

  async function runOneTest(
    orgId: string,
    projectId: string,
    rulesetName: string,
    testId: string
  ): Promise<TestRunResult | undefined> {
    if (!auth.token) return;
    runningOne.value = new Set([...runningOne.value, testId]);
    try {
      const ruleset = await buildPlatformRuleset(orgId, projectId, rulesetName);
      const result = await testApi.runOne(auth.token, projectId, rulesetName, testId, {
        ruleset,
        include_trace: true,
      });
      // Merge into existing results list
      const list = runResults.value.get(rulesetName) ?? [];
      const idx = list.findIndex((r) => r.test_id === testId);
      if (idx >= 0) {
        const updated = [...list];
        updated[idx] = result;
        runResults.value.set(rulesetName, updated);
      } else {
        runResults.value.set(rulesetName, [...list, result]);
      }
      return result;
    } finally {
      const next = new Set(runningOne.value);
      next.delete(testId);
      runningOne.value = next;
    }
  }

  // ── Project-level operations ──────────────────────────────────────────────

  async function runProjectTests(
    orgId: string,
    projectId: string,
    rulesetNames: string[]
  ): Promise<void> {
    if (!auth.token) return;
    projectRunning.value = true;
    try {
      const rulesetEntries = await Promise.all(
        rulesetNames.map(async (rulesetName) => {
          const ruleset = await buildPlatformRuleset(orgId, projectId, rulesetName);
          return [rulesetName, ruleset] as const;
        })
      );

      projectRunResult.value = await testApi.runProject(auth.token, projectId, {
        rulesets: Object.fromEntries(rulesetEntries),
        include_trace: true,
      });
      // Sync ruleset-level results from project run
      for (const rs of projectRunResult.value.rulesets) {
        runResults.value.set(rs.ruleset_name, rs.results);
      }
    } finally {
      projectRunning.value = false;
    }
  }

  return {
    testsByRuleset,
    runResults,
    loadingRuleset,
    running,
    runningOne,
    projectRunResult,
    projectRunning,
    fetchTests,
    createTest,
    updateTest,
    deleteTest,
    runTests,
    runOneTest,
    runProjectTests,
  };
});
