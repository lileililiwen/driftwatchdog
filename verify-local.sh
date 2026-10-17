#!/bin/sh
# verify-local.sh — local rehearsal of CI gates, per project.
#
# This is the "local == remote CI" guard. Run it BEFORE you push so an agent
# (or a human) catches the failure locally instead of discovering it late in
# GitHub Actions. It detects the project's language(s) by the files next to it
# and runs the same kind of checks CI runs.
#
# After you fix and this goes green, push and WAIT for remote CI:
#     git push -u origin <branch>
#     gh run watch            # blocks until the GitHub Actions verdict
#     # or for a PR:  gh pr create && gh pr checks
#
# NOTE: database-dependent projects (e.g. those with a Postgres service in CI)
# need a local database first. Start one (see the project's compose.test.yml or
# CI `services:` block) and export DATABASE_URL before running this.
#
# Add a project-specific verify-local.sh next to this one to override/extend
# the generic checks below.

set -u

ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT" || exit 1

FAILS=0
pass() { printf '\033[32m✔\033[0m %s\n' "$1"; }
fail() { printf '\033[31m✘\033[0m %s\n' "$1"; FAILS=$((FAILS+1)); }

has_rust=0; has_node=0; has_dotnet=0; has_py=0
[ -f Cargo.toml ] && has_rust=1
[ -f package.json ] && has_node=1
( ls *.csproj >/dev/null 2>&1 || ls *.sln >/dev/null 2>&1 || [ -f global.json ] ) && has_dotnet=1
( [ -f pyproject.toml ] || [ -f setup.py ] || [ -f setup.cfg ] || [ -f requirements.txt ] ) && has_py=1

is_workspace=0
[ -f Cargo.toml ] && grep -q '^\[workspace\]' Cargo.toml && is_workspace=1

if [ "$has_rust" -eq 1 ]; then
  echo "=== Rust ==="
  # Only touch rustup where it exists. Non-Rust projects (and Rust projects
  # checked out on a machine without a Rust toolchain) must not fail here.
  command -v rustup >/dev/null 2>&1 && rustup component add rustfmt clippy >/dev/null 2>&1 || true
  if cargo fmt --all -- --check 2>rust_fmt.log; then pass "cargo fmt"; else fail "cargo fmt"; cat rust_fmt.log; fi
  rm -f rust_fmt.log
  if [ "$is_workspace" -eq 1 ]; then
    cargo clippy --workspace --all-targets --all-features -- -D warnings 2>rust_clippy.log && pass "cargo clippy (workspace)" || { fail "cargo clippy (workspace)"; cat rust_clippy.log; }
  else
    cargo clippy --all-targets --all-features -- -D warnings 2>rust_clippy.log && pass "cargo clippy" || { fail "cargo clippy"; cat rust_clippy.log; }
  fi
  rm -f rust_clippy.log
  if [ "$is_workspace" -eq 1 ]; then
    cargo test --workspace --all-targets 2>rust_test.log && pass "cargo test (workspace)" || { fail "cargo test (workspace)"; cat rust_test.log; }
  else
    cargo test --all-targets 2>rust_test.log && pass "cargo test" || { fail "cargo test"; cat rust_test.log; }
  fi
  rm -f rust_test.log
fi

if [ "$has_node" -eq 1 ]; then
  echo "=== Node/TypeScript ==="
  if [ -f package-lock.json ]; then npm ci >/dev/null 2>&1 && pass "npm ci" || fail "npm ci"; else npm install >/dev/null 2>&1 && pass "npm install" || fail "npm install"; fi
  has_script() { node -e "process.exit((require('./package.json').scripts&&require('./package.json').scripts['$1'])?0:1)" 2>/dev/null; }
  has_script check    && ( npm run check    >/dev/null 2>&1 && pass "npm run check"    || fail "npm run check" )
  has_script typecheck && ( npm run typecheck >/dev/null 2>&1 && pass "npm run typecheck" || fail "npm run typecheck" )
  has_script test     && ( npm test >/dev/null 2>&1 && pass "npm test" || fail "npm test" )
fi

if [ "$has_dotnet" -eq 1 ]; then
  echo "=== .NET ==="
  dotnet build 2>dotnet_build.log && pass "dotnet build" || { fail "dotnet build"; cat dotnet_build.log; }
  rm -f dotnet_build.log
  dotnet test --no-build 2>dotnet_test.log && pass "dotnet test" || { fail "dotnet test"; cat dotnet_test.log; }
  rm -f dotnet_test.log
fi

if [ "$has_py" -eq 1 ]; then
  echo "=== Python ==="
  [ -f pyproject.toml ] && grep -q 'pytest' pyproject.toml && ( python -m pytest 2>py_test.log && pass "pytest" || { fail "pytest"; cat py_test.log; } )
  rm -f py_test.log
  command -v ruff >/dev/null 2>&1 && ( ruff check . 2>py_ruff.log && pass "ruff" || { fail "ruff"; cat py_ruff.log; } )
  rm -f py_ruff.log
  [ -f pyproject.toml ] && grep -q '\[tool.mypy\]' pyproject.toml && ( mypy . 2>py_mypy.log && pass "mypy" || { fail "mypy"; cat py_mypy.log; } )
  rm -f py_mypy.log
fi

if [ "$has_rust$has_node$has_dotnet$has_py" = "0000" ]; then
  echo "No recognized project type here (Cargo.toml/package.json/*.csproj/pyproject.toml)."
fi

if [ "$FAILS" -eq 0 ]; then
  printf '\n\033[32mAll local checks passed.\033[0m Push and wait: gh run watch\n'
else
  printf '\n\033[31m%d check(s) failed locally.\033[0m Fix before pushing.\n' "$FAILS"
fi
exit "$FAILS"
