#!/usr/bin/env bash
set -euo pipefail

# Doc governance checks — validates mirror consistency and dead links.
# Referenced by: task check:docs (Taskfile.yml)

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ERRORS=0

# --- 1. CLAUDE.md / AGENTS.md mirror check ---
if ! diff -q "$REPO_ROOT/CLAUDE.md" "$REPO_ROOT/AGENTS.md" > /dev/null 2>&1; then
    echo "FAIL: CLAUDE.md and AGENTS.md are not mirrored"
    ERRORS=$((ERRORS + 1))
else
    echo "OK: CLAUDE.md == AGENTS.md"
fi

# --- 2. Dead internal links in docs/ ---
DEAD_LINK_FILE=$(mktemp)
trap 'rm -f "$DEAD_LINK_FILE"' EXIT

find "$REPO_ROOT/docs" "$REPO_ROOT/CLAUDE.md" "$REPO_ROOT/AGENTS.md" -name '*.md' 2>/dev/null | while IFS= read -r md_file; do
    dir="$(dirname "$md_file")"
    # Extract markdown links: [text](path) — skip http/https/mailto/anchor-only links
    grep -oE '\]\([^)]+\)' "$md_file" 2>/dev/null | \
        sed 's/^\]//' | sed 's/)$//' | sed 's/^(//' | \
        grep -v '^http' | grep -v '^mailto' | grep -v '^#' | \
        sed 's/#.*//' | \
    while IFS= read -r link; do
        [ -z "$link" ] && continue
        target="$dir/$link"
        if [ ! -e "$target" ]; then
            echo "DEAD LINK: $md_file -> $link"
            echo "1" >> "$DEAD_LINK_FILE"
        fi
    done || true
done || true

DEAD_LINKS=$(wc -l < "$DEAD_LINK_FILE" 2>/dev/null | tr -d ' ')
if [ "$DEAD_LINKS" -gt 0 ]; then
    ERRORS=$((ERRORS + DEAD_LINKS))
else
    echo "OK: No dead internal links"
fi

# --- 3. Release docs map to released versions ---
RELEASE_VERSIONS="$(grep -oE '^## \[[0-9]+\.[0-9]+\.[0-9]+\]' "$REPO_ROOT/CHANGELOG.md" | sed -E 's/^## \[([0-9]+\.[0-9]+\.[0-9]+)\]$/\1/' || true)"
if [ -z "$RELEASE_VERSIONS" ]; then
    echo "OK: No released versions found in CHANGELOG.md"
else
    while IFS= read -r version; do
        [ -z "$version" ] && continue
        tag="v${version}"
        doc_path="$REPO_ROOT/docs/releases/${tag}.md"

        if [ ! -f "$doc_path" ]; then
            echo "FAIL: missing release doc for ${tag}: docs/releases/${tag}.md"
            ERRORS=$((ERRORS + 1))
            continue
        fi
        if ! grep -Fxq "# Release ${tag}" "$doc_path"; then
            echo "FAIL: ${doc_path} missing heading '# Release ${tag}'"
            ERRORS=$((ERRORS + 1))
        fi
        required_sections=(
            "## Summary"
            "## Process"
            "## Artifacts"
            "## Verification"
            "## Known Issues"
            "## Rollback"
            "## Detail Links"
        )
        for section in "${required_sections[@]}"; do
            if ! grep -Fxq "$section" "$doc_path"; then
                echo "FAIL: ${doc_path} missing section '$section'"
                ERRORS=$((ERRORS + 1))
            fi
        done

        if ! grep -Fq "| Asset |" "$doc_path"; then
            echo "FAIL: ${doc_path} missing artifacts table header '| Asset |'"
            ERRORS=$((ERRORS + 1))
        fi

        DETAIL_LINKS_CONTENT="$(awk '/^## Detail Links$/{flag=1; next} /^## /{flag=0} flag {print}' "$doc_path")"
        DETAIL_LINK_COUNT="$(printf '%s\n' "$DETAIL_LINKS_CONTENT" | grep -Eo '\[[^]]+\]\([^)]+\)' | wc -l | tr -d ' ')"
        if [ "$DETAIL_LINK_COUNT" -lt 3 ]; then
            echo "FAIL: ${doc_path} needs at least three markdown links under '## Detail Links'"
            ERRORS=$((ERRORS + 1))
        fi
    done <<< "$RELEASE_VERSIONS"
fi

# --- Summary ---
if [ "$ERRORS" -gt 0 ]; then
    echo ""
    echo "FAILED: $ERRORS doc governance error(s)"
    exit 1
fi

echo ""
echo "All doc governance checks passed."
