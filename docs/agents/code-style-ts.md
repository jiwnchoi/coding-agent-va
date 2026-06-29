# TypeScript Conventions

Last updated: 2026-06-29

Applies to `src/**` and `modules/*/src/**`.

## Naming and File Rules

- Use TypeScript for new files in scope.
- Use `PascalCase` for React component identifiers.
- Use `camelCase` for variables, functions, and custom hooks.
- Hook filenames must start with `use` (example: `usePhotoScan.ts`).
- Route files must follow Expo Router naming under `src/app/**` (examples: `_layout.tsx`, `index.tsx`, `+not-found.tsx`).

## Export Rules

- Every public feature folder must export from `index.ts` or `index.tsx`.
- Every public domain folder must export from `index.ts` or `index.tsx`.
- Each file may export only one public React component or one public custom hook.
- Wildcard re-exports (`export * from ...`) are prohibited in this repository; use explicit named re-exports.
- Domain modules may be imported through deep paths from non-domain code when that keeps dependencies clearer; feature internals must still not be deep-imported across feature boundaries.

## Boundary Parsing Rule

- Boundary parsing rules are documentation-only conventions.
- Boundary parsing rules are out of scope for static lint enforcement (`dependency-cruiser`, `eslint`, `oxlint`).
- Boundary inputs from network responses, native bridge payloads, and persisted raw storage reads must be validated with `valibot` at the first entry point.
- Parse once at the boundary and pass normalized typed models inward; do not spread repeated boundary parsing across downstream hooks/components.
- When boundary validation fails, return a safe fallback state and keep failure handling localized near the boundary.

## Lint Enforcement Scope

- `dependency-cruiser` architecture rules apply only to `src/**`.
- `eslint` rules apply to `src/**` and `modules/*/src/**`.
- `oxlint` rules apply repository-wide to JavaScript/TypeScript files, excluding paths in `tools/config/.oxlintrc.json` `ignorePatterns`.

## Test File Rules

- Test files must use `.test.ts` only.
- Test files must be colocated with the source file.
- Do not create test files with the `.tsx` extension.
- Do not create TypeScript tests for `.tsx` files or React component files; React components are treated as pure view layers, so test the custom hooks, services, or domain logic used by components instead.
