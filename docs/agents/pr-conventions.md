# Pull Request Conventions — Agent Cheatsheet

MUST WRITE PR BODY IN ENGLISH

Purpose: concise guidance so tools/agents create clear, reviewer-friendly PRs.

## Principles

- Derive from commits: PR content summarizes the commit history (titles, bodies, breaking notes).
- Be concise: prioritize signal; avoid repetition and noise.
- Emphasize reviewer needs: call out risky areas, decisions, and what to verify.
- Cover What / Why / How explicitly.
- `gh` CLI can be used to create/update PRs (`gh pr create`, `gh pr edit`) with this convention.
- Always end with `@codex review` to request an automated review pass.
- If the branch name contains a Jira ticket key (e.g., `ABC-123`), prefix the PR title with it: `ABC-123: <title>`.

## Structure (Template)

- Title: mirror the main commit subject, optionally refined for scope.
- What: one sentence describing the change outcome.
- Why: motivation, problem solved, expected impact; link issues.
- How: brief approach and key changes (bullets, 3–6 items max).
- Reviewer Focus: files/paths, edge cases, breaking changes, migration steps.
- Risk/Impact: compatibility, perf, UX, data; note any `BREAKING CHANGE`.
- Test Plan: how to verify; commands, cases, screenshots if applicable.
- Links: `Refs: #issue`, related PRs, docs.
- Footer: `@codex review`

## Example

Title: feat(profile): add tag statistics page

What: introduce tag-level charts/tables and filters.

Why: help users discover trends; requested in #123.

How:

- add `src/pages/tag_statistics.py` with charts and query helpers
- reuse `domains.profile_filter` for filtering logic
- wire route in `src/router.py`; add nav entry
- tests: basic data shape assertions

Reviewer Focus:

- performance on large datasets
- filtering semantics vs existing `profile_filter`
- navigation and deep-link behavior

Risk/Impact:

- no breaking API changes
- increased memory usage for chart data; verified locally

Test Plan:

- `uv run task type && uv run task lint && uv run task format`
- manual: open Tag Statistics, filter by top-10 tags, verify counts

Links: Refs: #123

@codex review