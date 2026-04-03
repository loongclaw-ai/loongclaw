# Agent Risk Mapping

Date: 2026-04-03
Issue: #851
Status: Active

This document maps common agent-risk categories to LoongClaw's current
repository-visible controls.

It is a control-mapping baseline, not a compliance certification.
Statuses in this document mean:

- `Enforced`: the repository already documents or implements this as an active
  runtime control
- `Partial`: meaningful controls exist, but important gaps remain
- `Planned`: the roadmap or design direction is clear, but current coverage is
  not yet sufficient to rely on the control as active protection

This document should be read together with:

- [Threat Model](THREAT_MODEL.md)
- [Security](SECURITY.md)
- [Roadmap](ROADMAP.md)
- [Architecture](../ARCHITECTURE.md)

## Control Mapping

| Risk area | Current LoongClaw controls | Status | Main evidence | Main gaps |
|-----------|----------------------------|--------|---------------|-----------|
| Goal or instruction hijack leading to unsafe action | kernel-mediated tool execution, policy extension chain, approval gates, explicit runtime binding | Partial | `docs/SECURITY.md`, `ARCHITECTURE.md`, `docs/design-docs/layered-kernel-design.md` | direct-mode compatibility still exists in some outer seams; not every runtime path is fully converged |
| Excessive capability or privilege | capability tokens, pack boundaries, denylist precedence, TTL-based authorization, tool-specific approval | Enforced to partial | `ARCHITECTURE.md`, `docs/SECURITY.md` | `membrane` not enforced; namespace confinement incomplete |
| Identity or provenance abuse | runtime identity separation, connector caller provenance, channel account identity handling, plugin attestation metadata | Partial | `docs/SECURITY.md`, `docs/product-specs/runtime-self-continuity.md` | no first-class trust plane, no explicit trust decay or delegation contract |
| Plugin or package supply-chain compromise | manifest validation, compatibility checks, ownership checks, structured diagnostics, preflight governance, checksum and signature support | Partial | `docs/SECURITY.md`, `docs/design-docs/plugin-package-manifest-contract.md`, `docs/ROADMAP.md` | trust tiers and reproducible verification are not fully closed yet |
| Unexpected code execution or unsafe runtime expansion | WASM guardrails, runtime execution tiers, bridge support matrix, activation attestation, allowlisted process lanes | Partial | `docs/SECURITY.md`, `docs/ROADMAP.md`, `docs/design-docs/layered-kernel-design.md` | process isolation is not fully implemented; runtime resource guarantees are still under Stage 2 |
| Memory or context poisoning | runtime identity boundary, kernel-bound history fail-closed behavior, staged memory architecture, session-profile separation | Partial | `docs/SECURITY.md`, `docs/product-specs/runtime-self-continuity.md`, `docs/QUALITY_SCORE.md` | provenance, scoped retrieval, deletion governance, and poisoning-resistant operator surfaces are incomplete |
| Insecure inter-system or protocol communication | typed protocol routes, route authorization, bounded transports, SSRF-safe web and browser clients, outbound HTTP host restrictions | Partial | `docs/SECURITY.md`, `ARCHITECTURE.md`, `crates/protocol` references in docs | broader trust semantics across channels, connectors, and future multi-agent paths are still incomplete |
| Cascading failures or uncontrolled fanout | retry and backoff controls, rate shaping, connector circuit breakers, adaptive concurrency, scheduler telemetry | Partial | `docs/ROADMAP.md` | reliability primitives are not yet packaged as one explicit operator-facing SRE lane |
| Unsafe human trust or approval misuse | explicit approval surfaces, policy-bound high-risk actions, operator security audit surface | Partial | `docs/SECURITY.md`, `docs/ROADMAP.md` | stronger review guidance, approval evidence packaging, and trust-plane linkage remain future work |
| Rogue or drifted runtime behavior | audit trail, plugin preflight, activation attestation, release-doc governance, architecture and dependency gates | Partial | `docs/SECURITY.md`, `docs/releases/README.md`, `.github/workflows/ci.yml` | continuous drift detection and tamper-evident audit chain are not fully closed |

## Summary

The main conclusion from this first mapping is straightforward:

- LoongClaw already has meaningful governance controls in the core runtime
- the current gaps are mostly about closure, trust modeling, and evidence
  packaging rather than total absence of governance work

The highest-leverage next improvements remain:

1. stronger governance evidence
2. a first-class trust and identity plane
3. runtime isolation completion
4. memory provenance and retention governance
5. plugin supply-chain trust closure

## Non-Claims

This document does not claim:

- complete compliance with any external framework
- uniform enforcement across every future channel or plugin ecosystem
- finished tamper-evident audit guarantees
- complete memory-governance closure

Those areas should move from `Partial` toward `Enforced` only when repository
evidence and tests support the claim.
