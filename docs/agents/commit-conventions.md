# Conventional Commits 1.0.0 — Agent Cheatsheet

Purpose: concise rules so tools/agents can generate consistent, automatable commit messages.

## Basic Format

<type>[optional scope][!]: <short description>

- type: feat | fix | docs | style | refactor | perf | test | build | chore | ci | revert
- scope (optional): area of the codebase in parentheses, e.g., feat(parser): ...
- ! (optional): marks a breaking change
- description: imperative, concise, no trailing period

Optional sections:

- Body: start one blank line after the subject; explain motivation, context, impact.
- Footers: start one blank line after body; use Trailer: value (e.g., BREAKING CHANGE: ..., Refs: #123, Reviewed-by: ...).

## SemVer Mapping

- feat: MINOR release
- fix: PATCH release
- BREAKING CHANGE: MAJOR release (can appear via type! or a footer)
- Other types categorize changes; they do not affect SemVer unless BREAKING CHANGE is present.

## Prompt-Oriented Rules

- Always produce a one-line subject: <type>[scope][!]: <description>
- Add body and footers only when helpful; separate sections with a single blank line.
- Keep subject around 72 characters; body can be multiple paragraphs.
- If breaking behavior exists, include ! in the type or a BREAKING CHANGE: footer with details.
- Reference issues/PRs in footers (e.g., Refs: #123). Avoid noisy metadata in the subject.
- Do not bypass hooks with `--no-verify`; resolve pre-commit/pre-push failures before creating commits.

## Examples

```
feat: allow provided config object to extend other configs

BREAKING CHANGE: `extends` key in config file is now used for extending other config files
```

```
feat!: send an email to the customer when a product is shipped
```

```
feat(api)!: send an email to the customer when a product is shipped
```

```
chore!: drop support for Node 6

BREAKING CHANGE: use JavaScript features not available in Node 6.
```

```
docs: correct spelling of CHANGELOG
```

```
feat(lang): add Polish language
```

```
fix: prevent racing of requests

Introduce a request id and a reference to latest request. Dismiss incoming responses other than from latest request.

Remove timeouts which were used to mitigate the racing issue but are obsolete now.

Reviewed-by: Z
Refs: #123
```

## Notes

- Tokens are case-insensitive, except BREAKING CHANGE which must be uppercase.
- BREAKING-CHANGE is synonymous with BREAKING CHANGE in footers.