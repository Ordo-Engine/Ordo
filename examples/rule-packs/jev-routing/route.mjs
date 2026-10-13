#!/usr/bin/env node
// Jev reads the message, Ordo decides what happens next.
//
//   node route.mjs samples/double-charge.json
//
// With OPENROUTER_API_KEY set, the message is sent to Jev through OpenRouter.
// Without it, the recorded `jev_response` in the sample file is used, so the
// example runs offline. Needs Node 18+ and the `ordo` CLI on PATH.

import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const samplePath = process.argv[2];
if (!samplePath) {
  console.error("usage: node route.mjs <sample.json>");
  process.exit(2);
}
const sample = JSON.parse(readFileSync(samplePath, "utf8"));
const questions = JSON.parse(readFileSync(join(here, "jev-questions.json"), "utf8"));

async function askJev(message) {
  const key = process.env.OPENROUTER_API_KEY;
  if (!key) return { ...sample.jev_response, recorded: true };
  const res = await fetch("https://openrouter.ai/api/v1/systemone", {
    method: "POST",
    headers: { Authorization: `Bearer ${key}`, "Content-Type": "application/json" },
    body: JSON.stringify({
      model: process.env.JEV_MODEL ?? "typesafe/jev-1.13",
      state: message,
      questions,
    }),
  });
  if (!res.ok) throw new Error(`Jev returned ${res.status}: ${await res.text()}`);
  return res.json();
}

const jev = await askJev(sample.message);
const a = jev.answers;

// Only the fields the rules read. Ordo validates them against the input
// schema, so a malformed Jev answer is rejected instead of guessed at.
const input = {
  jev: {
    intent: { choice: a.intent.choice, confidence: a.intent.confidence },
    urgency: { score: a.urgency?.score ?? 0 },
    chargeback: { noul: a.chargeback?.noul ?? 0 },
  },
  order: sample.order,
  customer: sample.customer,
};

const out = execFileSync(
  "ordo",
  ["trace", "refund-routing", "--input", JSON.stringify(input)],
  { cwd: here, encoding: "utf8" },
);

console.log(`message: ${sample.message}`);
console.log(`jev (${jev.recorded ? "recorded" : jev.model}): ` +
  `intent=${a.intent.choice} ${a.intent.confidence}, ` +
  `urgency=${a.urgency?.score}, chargeback=${a.chargeback?.noul}`);
console.log();
console.log(out.trimEnd());
