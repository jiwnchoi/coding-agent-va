# Build, Test, and Development Commands

## Single Source of Truth

- Local command behavior is defined only in `justfile`, `api/justfile`, `Makefile`, `tools/justfile.*` (including `tools/justfile.python`), `tools/android-logic/**`, `scripts/*.sh`, `scripts/*.py`, `experiments/android_benchmark/**`, and `experiments/ios_benchmark/**`.
- API container image, build context, and runtime entrypoint behavior are defined in `.dockerignore`, `api/Dockerfile`, `Makefile`, and `api/entrypoint.sh`.
- Generated API client source selection, checked-in OpenAPI snapshot path, and output behavior are defined in `orval.config.cjs`.
- Expo prebuild-time native generation behavior is defined in `app.config.js` plugin entries and `plugins/*.js`.
- Expo run build cache provider behavior is defined in `app.json` under `expo.buildCacheProvider`.
- Expo native compatibility fingerprint behavior is defined in `fingerprint.config.js` and `.fingerprintignore`.
- Push-time native check/test change detection is defined in `.github/workflows/check-test.yml` and uses `scripts/resolve_expo_fingerprint_hash.js` for Expo fingerprint comparisons.
- Android CI jobs run on `android-build` runners that already provide Java, Android SDK, and NDK; command-time Android environment validation is defined in `scripts/setup_android_env.sh`.
- Git hook command behavior is defined only in `lefthook.yml`.
- CI/CD command behavior is defined only in `.github/workflows/*.yml`, `.github/actions/**/action.yml`, and `.github/actionlint.yaml`.
- Manual stable mobile release source resolution, distribution, and main-branch metadata persistence behavior is defined in `.github/workflows/stable-release.yml`, `.github/workflows/build-deploy.yml`, `scripts/stable_release_main.py`, and `scripts/mobile_release_main.py`.
- PR preview distribution behavior, including preview-only release metadata overrides and workflow-file-change guardrails, is defined in `.github/workflows/pr-build.yml` and `.github/workflows/build-deploy.yml`.

## Documentation Rules

- Do not document per-command internals in this file.
- Do not duplicate build/test/CI step descriptions here.
- Do not include linting or formatting rules in `docs/agents`.
- Do not duplicate rules already enforced by linting or formatting tools in `docs/agents`; the tool config and rule tests are the source of truth.
- If command behavior changes, update the command source files first.
- Update this file only when source-of-truth file paths change.
