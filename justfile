set shell := ["bash", "-euo", "pipefail", "-c"]

import 'tools/justfile.node'
import 'tools/justfile.rust'

default:
  @just --list

prepare-hooks:
  if command -v lefthook >/dev/null 2>&1; then \
    lefthook install; \
  else \
    ./node_modules/.bin/lefthook install; \
  fi

prepare:
  @just prepare-hooks
  @just prepare-node
  @just prepare-rust

dev:
  @just dev-rust

[parallel]
format:
  @just format-ts
  @just format-rust

[parallel]
lint:
  @just lint-ts
  @just lint-rust

[parallel]
typecheck:
  @just typecheck-ts
  @just typecheck-rust

[parallel]
compile:
  @just compile-ts
  @just compile-rust

[parallel]
build:
  @just build-ts
  @just build-rust

[parallel]
test:
  @just test-ts
  @just test-rust

check:
  @just format
  @just lint
  @just typecheck
  @just test
