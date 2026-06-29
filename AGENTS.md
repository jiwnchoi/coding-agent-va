## Commit & PR

- [Build, Test, and Development Commands](docs/agents/build-test-and-development-commands.md)
- [Commit Conventions](docs/agents/commit-conventions.md)
- [PR Conventions](docs/agents/pr-conventions.md)
- [Toolchains](docs/agents/toolchains.md)

## Documentation Maintenance

- When project rules, conventions, commands, or workflows change, update the relevant files in `docs/agents` in the same change.
- Keep this `AGENTS.md` section link list in sync with the files under `docs/agents`.
- Keep `AGENTS.md` focused on document links and Behavioral Guidelines; store additional project-specific instructions in `docs/agents/*` and reference them here as links.
- Even if a user explicitly asks to add rules to `AGENTS.md`, add project-specific conventions to `docs/agents/*` and keep `AGENTS.md` limited to links plus behavioral guidance.

## Behavioral Guidelines

- Don't aim for the smallest patch; aim for a cleaner codebase. When updating features, prefer solutions that shorten the overall codebase and architecture through refactoring, rather than minimizing the size of the individual change.
- Prefer implementing features at the RN (TypeScript) level; when a native module is required due to essential native APIs or heavy workloads, build Swift and Android in parallel, implementing Swift first and then porting to Android.
- For Android Gradle commands, use `just gradlew <task/args...>` instead of invoking `./gradlew` directly.
- After completing a task, always find and run an appropriate check command from the `justfile`.
- Never ignore linter errors. Always refactor the code.