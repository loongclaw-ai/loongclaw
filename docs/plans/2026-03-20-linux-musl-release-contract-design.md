# Linux Musl Release Contract Design

## Context

As of 2026-03-20, the latest public GitHub release is `v0.1.0-alpha.2`. The
published Linux asset is still GNU-only, and a Debian 12 reproduction confirms
that the shipped `x86_64-unknown-linux-gnu` binary requires `GLIBC_2.38` and
`GLIBC_2.39` while Debian 12 provides glibc 2.36.

The current product code reflects that same contract:

- `scripts/release_artifact_lib.sh` resolves Linux targets only to GNU triples
- `scripts/install.sh` downloads only the resolved GNU archive for Linux
- `.github/workflows/release.yml` publishes GNU Linux artifacts only
- product docs do not explain a Linux libc compatibility contract

This means the reported failure is not a one-off packaging mistake. It is a
contract gap between the public installer surface and the actual Linux runtime
compatibility envelope.

## Problem

LoongClaw currently treats Linux as if a single GNU release binary were
universally installable.

That assumption is false:

1. Debian 12 and other older-glibc systems can receive an unusable binary
   through the default install path.
2. The installer has no libc-aware selection or fallback path once a Linux arch
   is resolved.
3. The release workflow names Linux artifacts by architecture only, not by libc
   variant, so the public contract is underspecified.
4. The repository has a glibc-floor check mechanism, but it is not surfaced as
   installer selection data for Linux x86_64.

This is a correctness and release-contract issue, not just a documentation
problem.

## Chosen Slice

Take the smallest complete contract fix instead of trying to solve all Linux
distribution packaging at once:

1. Publish dual Linux artifacts for x86_64:
   `x86_64-unknown-linux-gnu` and `x86_64-unknown-linux-musl`.
2. Keep the implementation structure multi-arch ready, but defer `aarch64`
   musl publishing to follow-up work.
3. Teach the Bash installer to choose GNU only when the host glibc is present
   and new enough for the published GNU artifact; otherwise choose musl.
4. Add an explicit user override for Linux libc selection.
5. Make the release helper, workflow, tests, and docs speak the same libc-aware
   contract.

This fixes the Debian 12 class of failures without expanding into package
managers, auto-update, or a broader Linux packaging redesign.

## Approaches Considered

### Approach A: Dual Linux artifacts plus libc-aware installer selection

Publish GNU and musl Linux archives, keep GNU as the preferred path when the
host can run it, and fall back to musl otherwise.

Pros:

- fixes the confirmed Debian 12 failure path
- preserves GNU binaries for hosts where they already work well
- keeps the public contract explicit and testable
- scales naturally to future `aarch64` musl support

Cons:

- adds a second Linux artifact to the release workflow
- requires deterministic glibc detection and override behavior

### Approach B: Musl-first Linux installs

Publish both artifacts but default all Linux installs to musl unless the user
explicitly requests GNU.

Pros:

- simplest installer behavior
- maximizes compatibility by default

Cons:

- changes the primary runtime profile for all Linux users
- hides GNU compatibility drift instead of making it explicit

### Approach C: GNU-only rebuild on an older glibc baseline

Keep one Linux artifact and move the GNU build to an older baseline so Debian 12
works without musl.

Pros:

- no new installer selection logic
- single Linux asset remains simple

Cons:

- still leaves Linux tied to one libc contract
- harder to guarantee across release infrastructure drift
- does not provide a compatibility escape hatch when GNU floor changes again

### Recommendation

Choose Approach A.

The issue is a missing Linux artifact contract, not merely a bad single build.
Dual artifacts plus libc-aware selection closes the compatibility gap while
keeping the release surface explicit.

## Design

### 1. Public Linux artifact contract

The public release surface should distinguish Linux assets by libc, not only by
architecture.

First-pass Linux artifacts:

- `x86_64-unknown-linux-gnu`
- `x86_64-unknown-linux-musl`

Non-Linux targets remain unchanged. Linux `aarch64` stays GNU-only in this
patch, but helper interfaces and workflow matrix entries should be structured so
adding `aarch64-unknown-linux-musl` later is a data extension rather than a
design rewrite.

Archive and checksum naming stay explicit by full target triple:

- `loongclaw-<tag>-x86_64-unknown-linux-gnu.tar.gz`
- `loongclaw-<tag>-x86_64-unknown-linux-musl.tar.gz`
- matching `.sha256` files for each

No generic `linux.tar.gz` alias should be introduced in this slice.

### 2. Shared Linux target and libc metadata

`scripts/release_artifact_lib.sh` should become the single source of truth for
Linux release-asset selection metadata.

It should grow libc-aware helpers rather than forcing the installer to invent
selection rules privately. The exact helper names can be chosen during
implementation, but the shared contract needs to cover:

- the supported Linux libc variants for a given architecture
- the default GNU target triple for a Linux architecture
- the musl fallback triple for a Linux architecture when supported
- the minimum supported glibc version for GNU Linux targets that the installer
  may auto-select

For this first slice:

- Linux x86_64 exposes GNU plus musl variants
- Linux aarch64 exposes GNU only
- Windows and macOS behavior is unchanged

The standalone Bash installer must remain self-contained. If it cannot source
`release_artifact_lib.sh` from a repository checkout, it should continue to
carry a mirrored fallback implementation of the same libc-aware helpers.

### 3. Installer selection model

The Bash installer remains the Linux bootstrap path. PowerShell continues to
cover Windows only and should not grow Linux behavior in this patch.

Linux selection order:

1. If the user provides an explicit libc override, honor it.
2. Otherwise, detect whether glibc is present on the host and determine its
   version.
3. If glibc is present and meets the minimum supported version for the GNU
   target, download GNU.
4. If glibc is absent, unreadable, unparsable, or too old, download musl.

User override:

- add a public Bash flag `--target-libc gnu|musl`
- add matching environment variable `LOONGCLAW_INSTALL_TARGET_LIBC`
- reject unsupported combinations with a precise error
- if the override requests GNU on a host whose detected glibc is too old, fail
  before download with a precise compatibility message instead of knowingly
  installing an unusable binary

Detection behavior must fail closed toward musl. The installer should not guess
GNU when host libc state is ambiguous.

This model intentionally avoids downloading and probing both archives at
runtime. The decision is made from host detection plus shared release metadata.
For host detection, prefer a direct glibc version probe such as
`getconf GNU_LIBC_VERSION` when available, then fall back to parsing
`ldd --version`; if neither produces a trustworthy glibc version, treat the
host as musl/unknown and select musl.

### 4. GNU glibc floor contract

The installer can only prefer GNU safely if the GNU artifact has an explicit
maximum required glibc version.

The release workflow already has a floor-check mechanism through
`scripts/check_glibc_floor.sh`. This slice should extend that existing pattern
instead of introducing a new release-time ABI check path.

Concretely:

- define an explicit GNU glibc floor for Linux x86_64 in the release workflow
- keep the existing GNU glibc floor contract for Linux aarch64
- verify GNU Linux artifacts against those declared floors during release builds
- expose the same floor values to installer selection through the shared helper
  library

That keeps release-time verification and install-time selection aligned. If a
GNU artifact exceeds its declared floor, the release should fail before publish.

### 5. Release workflow changes

`.github/workflows/release.yml` should publish both Linux x86_64 variants.

Required workflow changes:

- add `x86_64-unknown-linux-musl` to the build matrix
- keep `x86_64-unknown-linux-gnu` in the matrix
- continue packaging archives and checksums by full target triple
- ensure the publish step uploads both Linux artifacts
- keep completions generation deterministic by using a known release binary; the
  existing GNU x86_64 path can stay the source as long as that artifact remains
  in the matrix

The workflow should also fail clearly when a declared Linux variant is missing a
packaged archive or checksum.

## Testing Strategy

Follow TDD for the implementation, but the contract requires these verification
layers:

1. shared helper tests:
   - Linux target-to-libc resolution
   - archive/checksum naming for musl targets
   - explicit GNU glibc floor metadata for supported GNU Linux targets
2. Bash installer tests:
   - GNU chosen when host glibc satisfies the configured floor
   - musl chosen when host glibc is too old
   - musl chosen when glibc detection is unavailable or unparsable
   - explicit override forcing GNU or musl
   - precise failure when an overridden or auto-selected asset does not exist
3. release workflow checks:
   - GNU floor checks for GNU Linux artifacts
   - musl artifact packaging and checksum publication
4. reproducible compatibility evidence:
   - a Debian 12 default install path should resolve to musl instead of
     producing a runtime `GLIBC_* not found` failure

## Product Docs

Linux install documentation should explicitly describe the shipped contract:

- LoongClaw publishes GNU and musl Linux artifacts where available
- the Bash installer prefers GNU only on hosts that satisfy the declared glibc
  floor
- otherwise the installer falls back to musl
- users can override the selection when they need a specific libc variant

Docs should also avoid implying that all Linux artifacts are interchangeable.

## Non-Goals

- shipping Linux musl for `aarch64` in this first patch
- changing macOS or Windows install behavior
- adding package-manager distribution
- adding auto-update / self-update
- introducing release manifests or indirection layers beyond the existing asset
  naming model
- redesigning PowerShell install behavior for non-Windows targets

## Risks

- Musl builds may expose Rust or native dependency issues not present in current
  GNU builds. Mitigation: fail the release workflow hard on missing or broken
  musl artifacts.
- Glibc detection can become brittle across Linux environments. Mitigation: keep
  detection minimal and deterministic, and default uncertain cases to musl.
- Shared helper duplication between repository-backed and standalone installer
  paths can drift. Mitigation: keep fallback helper functions narrow and cover
  them in shell tests.
- GNU floor metadata can drift from actual build output. Mitigation: reuse the
  existing `check_glibc_floor.sh` enforcement path in release CI.

## Acceptance Criteria

- Public releases publish both `x86_64-unknown-linux-gnu` and
  `x86_64-unknown-linux-musl` archives with matching checksums.
- The Bash installer auto-selects GNU only when the host glibc satisfies the
  declared GNU floor; otherwise it installs musl.
- Linux users can explicitly override the libc variant through a supported
  installer surface.
- A Debian 12 default install path no longer ends in a runtime
  `GLIBC_2.38` / `GLIBC_2.39` failure from the chosen artifact.
- Release helpers, release workflow, installer behavior, and docs describe the
  same libc-aware Linux contract.
