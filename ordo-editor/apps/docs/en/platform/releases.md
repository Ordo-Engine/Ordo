# Drafts and Releases

Edits never go straight to an engine. They are saved as a draft, and a release sends a draft to the engines of an environment after review.

## Drafts

Saving in Studio, or `ordo push` from the CLI, updates the ruleset's draft. Each save carries the sequence number it started from. If someone else saved in between, the save is rejected with `409 Conflict`, and Studio (or `ordo pull`) brings in the newer draft first. Nothing is overwritten by accident.

You can try a draft on sample input before it is released. See [Studio](./studio#try-a-run).

Every release adds an entry to the ruleset's history, with the author and a snapshot. Studio shows the diff between any two entries.

## Release requests

A release request names the rulesets, their versions and the target environment. Studio shows the diff against what that environment runs now.

Release policies decide who must approve. A policy applies to a project or an environment and sets:

- the minimum number of approvals and who may approve,
- whether authors may approve their own request,
- how the release rolls out: all at once, in fixed-size batches, in percentage batches, or in batches on a timer,
- when to pause (error rate, metric breach) and whether to roll back automatically when instances fail.

Approvers see pending requests in their notifications. They approve or reject, with a reason.

## Running a release

After approval, run the release. The platform sends the rules to the engines bound to that environment, batch by batch, and shows progress live. A running release can be paused, resumed or rolled back.

Rolling back returns the environment to the version it ran before that release. The rollback is recorded like any other step, so history keeps every change.

## Tests and releases

Tests do not run on their own when a release is created. Run them in Studio before you open the request, or in CI with `ordo test` on the files pulled with `ordo pull`. See [Testing and Impact](/en/guide/testing).
