#!/usr/bin/env bash
# Версия SoftEcho: источник истины — Cargo.toml [package].version
#
#   ./scripts/version.sh              # напечатать текущую (0.2.0)
#   ./scripts/version.sh tag          # v0.2.0
#   ./scripts/version.sh check-tag    # на CI: GITHUB_REF_NAME / аргумент == Cargo.toml
#   ./scripts/version.sh bump patch|minor|major [--commit] [--tag] [--push]
#
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

cargo_version() {
  sed -n 's/^version *= *"\([^"]*\)".*/\1/p' "$ROOT/Cargo.toml" | head -1
}

set_cargo_version() {
  local new="$1"
  local tmp
  tmp="$(mktemp)"
  awk -v ver="$new" '
    BEGIN { done = 0 }
    /^\[package\]/ { in_pkg = 1 }
    in_pkg && /^version *=/ && !done {
      print "version = \"" ver "\""
      done = 1
      next
    }
    /^\[/ && !/^\[package\]/ { in_pkg = 0 }
    { print }
  ' "$ROOT/Cargo.toml" > "$tmp"
  mv "$tmp" "$ROOT/Cargo.toml"
  # Синхронизировать запись пакета в lock (если есть).
  if [[ -f "$ROOT/Cargo.lock" ]]; then
    awk -v ver="$new" '
      $0 == "name = \"softecho\"" { hit = 1 }
      hit && /^version = / {
        print "version = \"" ver "\""
        hit = 0
        next
      }
      { print }
    ' "$ROOT/Cargo.lock" > "$tmp"
    mv "$tmp" "$ROOT/Cargo.lock"
  fi
}

bump_semver() {
  local kind="$1"
  local cur maj min pat
  cur="$(cargo_version)"
  IFS=. read -r maj min pat <<<"$cur"
  maj="${maj:-0}"
  min="${min:-0}"
  pat="${pat:-0}"
  # отбросить pre-release / build metadata
  pat="${pat%%-*}"
  pat="${pat%%+*}"
  case "$kind" in
    patch) pat=$((pat + 1)) ;;
    minor) min=$((min + 1)); pat=0 ;;
    major) maj=$((maj + 1)); min=0; pat=0 ;;
    *)
      echo "bump: нужен patch|minor|major" >&2
      exit 1
      ;;
  esac
  echo "${maj}.${min}.${pat}"
}

normalize_tag() {
  local t="$1"
  t="${t#refs/tags/}"
  t="${t#v}"
  echo "$t"
}

cmd="${1:-get}"
case "$cmd" in
  -h|--help)
    sed -n '2,12p' "$0"
    exit 0
    ;;
  get|"")
    cargo_version
    ;;
  tag)
    echo "v$(cargo_version)"
    ;;
  check-tag)
    ref="${2:-${GITHUB_REF_NAME:-}}"
    if [[ -z "$ref" ]]; then
      echo "check-tag: нет тега (аргумент или GITHUB_REF_NAME)" >&2
      exit 1
    fi
    expect="$(normalize_tag "$ref")"
    got="$(cargo_version)"
    if [[ "$expect" != "$got" ]]; then
      echo "Версия не совпадает: тег → $expect, Cargo.toml → $got" >&2
      echo "Сначала: ./scripts/version.sh bump … --commit --tag" >&2
      exit 1
    fi
    echo "OK: v$got ↔ Cargo.toml"
    ;;
  bump)
    kind="${2:-}"
    if [[ -z "$kind" ]]; then
      echo "usage: $0 bump patch|minor|major [--commit] [--tag] [--push]" >&2
      exit 1
    fi
    shift 2 || true
    do_commit=0
    do_tag=0
    do_push=0
    for arg in "$@"; do
      case "$arg" in
        --commit) do_commit=1 ;;
        --tag) do_tag=1; do_commit=1 ;;
        --push) do_push=1; do_tag=1; do_commit=1 ;;
        *)
          echo "неизвестный флаг: $arg" >&2
          exit 1
          ;;
      esac
    done
    old="$(cargo_version)"
    new="$(bump_semver "$kind")"
    set_cargo_version "$new"
    echo "$old → $new"
    if [[ "$do_commit" -eq 1 ]]; then
      if ! git diff --quiet -- Cargo.toml Cargo.lock; then
        git add Cargo.toml Cargo.lock
        git commit -m "Release v${new}"
      else
        echo "нет изменений для commit" >&2
        exit 1
      fi
    fi
    if [[ "$do_tag" -eq 1 ]]; then
      if git rev-parse "v${new}" >/dev/null 2>&1; then
        echo "тег v${new} уже есть" >&2
        exit 1
      fi
      git tag -a "v${new}" -m "SoftEcho v${new}"
      echo "тег v${new}"
    fi
    if [[ "$do_push" -eq 1 ]]; then
      git push origin HEAD
      git push origin "v${new}"
      echo "pushed HEAD + v${new}"
    fi
    ;;
  *)
    echo "unknown: $cmd (get|tag|check-tag|bump)" >&2
    exit 1
    ;;
esac
