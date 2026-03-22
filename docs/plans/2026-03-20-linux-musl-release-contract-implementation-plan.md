# Linux Musl Release Contract Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a Linux x86_64 GNU plus musl release contract so Debian 12 class hosts install a usable binary by default.

**Architecture:** Extend the shared shell release helper with libc-aware Linux metadata, drive Bash installer selection from that helper plus host glibc detection, then publish the extra musl artifact in the release workflow and sync docs to the new contract.

**Tech Stack:** Bash, GitHub Actions YAML, Markdown docs, existing shell regression tests.

---

## Chunk 1: Shared Release Metadata

### Task 1: Add failing helper coverage for Linux libc variants

**Files:**
- Modify: `scripts/test_release_artifact_lib.sh`
- Modify: `scripts/test_install_sh.sh`
- Modify: `tasks/todo.md`

- [ ] **Step 1: Add failing helper assertions for musl-aware Linux metadata**

Add checks for:
- `x86_64-unknown-linux-musl` archive and checksum naming
- Linux x86_64 supported libc variants
- Linux x86_64 GNU floor metadata
- Linux aarch64 remaining GNU-only in the first slice

- [ ] **Step 2: Run helper test to verify it fails**

Run: `bash scripts/test_release_artifact_lib.sh`
Expected: FAIL because the helper only knows GNU Linux targets today.

- [ ] **Step 3: Update `tasks/todo.md` implementation checklist**

Record the helper-first execution order and the failing-test evidence.

### Task 2: Implement libc-aware release helper metadata

**Files:**
- Modify: `scripts/release_artifact_lib.sh`
- Modify: `scripts/test_release_artifact_lib.sh`

- [ ] **Step 1: Add shared Linux libc helper functions**

Implement narrow helpers for:
- supported libc variants per Linux architecture
- target triple resolution by Linux arch + libc
- default GNU floor lookup for supported GNU Linux targets

- [ ] **Step 2: Keep existing naming helpers compatible**

Preserve current archive/checksum/binary naming semantics and make musl targets round-trip through the same helpers.

- [ ] **Step 3: Re-run helper test to verify it passes**

Run: `bash scripts/test_release_artifact_lib.sh`
Expected: PASS

## Chunk 2: Installer Selection and Safety

### Task 3: Add failing installer coverage for libc auto-selection

**Files:**
- Modify: `scripts/test_install_sh.sh`
- Modify: `tasks/todo.md`

- [ ] **Step 1: Add failing installer scenarios**

Cover:
- GNU selected when host glibc satisfies the configured floor
- musl selected when host glibc is too old
- musl selected when glibc detection is unavailable
- `--target-libc gnu|musl` and `LOONGCLAW_INSTALL_TARGET_LIBC`
- forced GNU on an unsupported glibc host fails before download

- [ ] **Step 2: Run installer test to verify it fails**

Run: `bash scripts/test_install_sh.sh`
Expected: FAIL because the installer currently resolves only one GNU Linux target and has no libc override or detection logic.

### Task 4: Implement Bash installer libc selection

**Files:**
- Modify: `scripts/install.sh`
- Modify: `scripts/release_artifact_lib.sh`
- Modify: `scripts/test_install_sh.sh`

- [ ] **Step 1: Add Linux libc override parsing**

Implement:
- `--target-libc gnu|musl`
- `LOONGCLAW_INSTALL_TARGET_LIBC`
- precise validation errors for unsupported combinations

- [ ] **Step 2: Add glibc detection helpers**

Prefer:
- `getconf GNU_LIBC_VERSION`
- fallback to `ldd --version`
- unresolved or untrustworthy detection defaults to musl

- [ ] **Step 3: Resolve Linux target through the shared helper**

Keep non-Linux behavior unchanged and make standalone-installer fallback helpers mirror the repository helper contract.

- [ ] **Step 4: Fail early for explicit incompatible GNU override**

If the user forces GNU and the detected glibc floor is too old, return a compatibility error before download.

- [ ] **Step 5: Re-run installer regression test**

Run: `bash scripts/test_install_sh.sh`
Expected: PASS

## Chunk 3: Release Workflow and Docs

### Task 5: Add the musl release artifact to the publish workflow

**Files:**
- Modify: `.github/workflows/release.yml`
- Modify: `.github/workflows/ci.yml`
- Modify: `scripts/test_release_artifact_lib.sh`

- [ ] **Step 1: Extend the release matrix**

Add:
- `x86_64-unknown-linux-musl`
- explicit x86_64 GNU glibc floor env alongside existing Linux ARM64 floor

- [ ] **Step 2: Keep release verification aligned**

Ensure:
- GNU floor checks apply to GNU Linux artifacts only
- musl artifact packaging uploads archive + checksum
- CI shell syntax/regression coverage stays green for the touched scripts

- [ ] **Step 3: Run targeted shell regression checks**

Run: `bash scripts/test_release_artifact_lib.sh`
Expected: PASS

Run: `bash scripts/test_install_sh.sh`
Expected: PASS

### Task 6: Sync public docs to the libc-aware Linux contract

**Files:**
- Modify: `README.md`
- Modify: `docs/product-specs/installation.md`
- Modify: `tasks/todo.md`

- [ ] **Step 1: Update README install wording**

Document that Linux installs:
- publish GNU and musl artifacts where available
- prefer GNU on compatible glibc hosts
- fall back to musl otherwise
- allow explicit override

- [ ] **Step 2: Update product-spec acceptance wording**

Reflect the shipped libc-aware Linux behavior without changing macOS or Windows contract language.

- [ ] **Step 3: Update task tracker with implementation results**

Record the executed checks and any residual follow-up such as future `aarch64` musl work.

## Chunk 4: Verification and Delivery

### Task 7: Run targeted verification gates

**Files:**
- No file changes required

- [ ] **Step 1: Run shell regression suite**

Run: `bash scripts/test_release_artifact_lib.sh`
Expected: PASS

Run: `bash scripts/test_install_sh.sh`
Expected: PASS

Run: `bash scripts/test_check_glibc_floor.sh`
Expected: PASS

- [ ] **Step 2: Run diff hygiene**

Run: `git diff --check`
Expected: PASS

### Task 8: Run repo verification and document known unrelated failures

**Files:**
- Modify: `tasks/todo.md`

- [ ] **Step 1: Run canonical verification**

Run: `task verify`
Expected: PASS except for pre-existing unrelated repo gate failures, if any.

- [ ] **Step 2: If `task verify` fails for an unrelated baseline issue, capture it explicitly**

Current known candidate:
- `cargo deny` advisory `RUSTSEC-2026-0049` on `rustls-webpki 0.103.9`

- [ ] **Step 3: Commit implementation in logical slices**

Suggested commit order:
- helper + tests
- installer + tests
- workflow + docs

- [ ] **Step 4: Final review summary**

Capture:
- what changed
- what was verified
- what remains intentionally out of scope (`aarch64` musl)
