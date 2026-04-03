# Threat Model

Date: 2026-04-03
Issue: #851
Status: Active

This document provides a repository-local threat model for LoongClaw's current
governed runtime posture.

It is intentionally bounded.
It describes current trust boundaries, attack surfaces, mitigations, and
residual risks based on repository evidence today.

It is not a certification document.
It does not claim complete coverage of every agent-risk framework.
It should be read together with:

- [Architecture](../ARCHITECTURE.md)
- [Security](SECURITY.md)
- [Roadmap](ROADMAP.md)
- [Agent Risk Mapping](AGENT_RISK_MAPPING.md)

## Scope

This threat model focuses on the runtime-governance layer that LoongClaw
already exposes through:

- capability-gated kernel execution
- policy and approval enforcement
- audit and observability
- runtime bridge execution
- plugin intake and activation
- protocol and channel integration
- memory and context assembly

It does not attempt to model:

- model-provider safety training
- prompt-content moderation as a standalone product surface
- hardware or robotics deployment
- future trust planes that are not yet implemented

## Trust Boundaries

### 1. Human Or Operator To Runtime

Humans provide prompts, approvals, configuration, policies, and release-facing
artifacts.

Main risks:

- unsafe approvals
- over-broad policy settings
- configuration drift
- release-doc or trace mismatch

Current mitigations:

- explicit approval surfaces
- policy-owned execution path
- structured release-doc governance checks
- operator-facing security posture audit

### 2. Model Or Conversation Loop To Kernel-Governed Actions

This is the core LoongClaw governance boundary.
The runtime may continue conversation orchestration in direct mode, but
kernel-mediated tool execution requires explicit authority.

Main risks:

- hidden kernel bypasses
- accidental direct-mode fallback
- missing approval checks

Current mitigations:

- `ConversationRuntimeBinding`
- `ProviderRuntimeBinding`
- capability and policy checks before governed execution
- binding-first fail-closed behavior for missing authority in kernel-only paths

### 3. Agent To Tool Or Connector

This is the highest-value action boundary because successful abuse can produce:

- shell execution
- file mutation
- network egress
- external side effects

Current mitigations:

- capability tokens
- policy extension chain
- tool-specific approval requirements
- filesystem path confinement
- SSRF-safe web and browser URL validation
- connector caller provenance for programmatic calls

### 4. Agent To Plugin Or Runtime Bridge

Plugins, foreign packages, and runtime bridges create a supply-chain and
execution-expansion boundary.

Main risks:

- malicious or malformed manifests
- incompatible host contracts
- unsafe bridge widening
- runtime expansion without trustworthy provenance

Current mitigations:

- plugin manifest validation
- ownership and compatibility checks
- structured preflight diagnostics
- policy integrity pinning and signature support
- activation attestation for loaded plugin runtime compatibility

### 5. Agent To Memory And Context

Memory and context are long-lived influence surfaces.
If they are poisoned, mis-scoped, or silently de-governed, future turns can
drift without obvious operator visibility.

Main risks:

- poisoned summaries
- provenance loss
- accidental authority bleed between runtime identity and advisory context
- destructive deletion without clear ownership

Current mitigations:

- explicit runtime identity separation
- binding-first history access for kernel-bound flows
- staged memory architecture direction
- fail-closed kernel-bound history loading

### 6. Agent To Channel, Protocol, Or External Endpoint

LoongClaw now spans CLI, browser, providers, channels, and protocol bridges.
That expands the communication boundary.

Main risks:

- untrusted outbound endpoints
- redirect-based trust widening
- protocol mismatch
- ambiguous account identity
- unsafe bridge execution or runtime routing drift

Current mitigations:

- typed protocol routes
- route authorization gates
- bounded transport primitives
- outbound HTTP host restrictions
- no ambient proxy trust for governed web lanes
- explicit channel/runtime account identity handling

### 7. Agent To Repository And Release Control Plane

The repository itself is part of the runtime trust story because release docs,
trace artifacts, CI, and package workflows influence operator trust.

Main risks:

- release metadata drift
- missing traceability
- undocumented contract changes
- supply-chain ambiguity

Current mitigations:

- release-doc conventions
- local release-artifact bootstrap
- doc governance checks
- architecture and dependency-graph gates

## High-Level Data Flow

```text
Human / Operator
    |
    v
Conversation / Provider Runtime
    |
    v
Kernel capability + policy + approval boundary
    |
    +--> Tool plane
    +--> Connector plane
    +--> Runtime bridge plane
    +--> Memory / context path
    +--> Plugin intake / activation path
    |
    v
Audit + observability + release / trace evidence
```

## Primary Attack Surfaces

| Surface | Example threats | Current posture |
|---------|-----------------|-----------------|
| Prompts, approvals, operator input | unsafe approval, social engineering, over-broad settings | Partial |
| Governed tool execution | shell abuse, file mutation, unsafe network access | Enforced to partial, depending on tool and path |
| Plugin manifests and bridge execution | supply-chain drift, host incompatibility, bridge widening | Partial with strong preflight direction |
| Memory and context assembly | poisoned summaries, provenance loss, identity bleed | Partial |
| Channel and protocol edges | SSRF, untrusted endpoints, route drift, account confusion | Partial |
| Release and trace artifacts | missing debug linkage, trace mismatch, documentation drift | Partial |

## STRIDE View

| Category | Example LoongClaw risk | Main mitigations today | Residual gaps |
|----------|------------------------|------------------------|---------------|
| Spoofing | connector or channel caller identity ambiguity | caller provenance, account identity normalization, binding-first execution | no full trust plane yet |
| Tampering | plugin metadata drift, audit evidence manipulation, release trace mismatch | manifest validation, attestation, release-doc checks, durable audit JSONL | no HMAC chain, limited tamper-evidence story |
| Repudiation | operator or runtime cannot prove what happened | structured audit events, approval records, release trace linkage | audit chain integrity still partial |
| Information disclosure | web or browser escape, outbound endpoint misuse, context leakage | SSRF guardrails, host restrictions, tool policy, path confinement | namespace confinement not fully enforced |
| Denial of service | runaway bridge execution, cascading retries, expensive fanout | bounded queues, rate shaping, circuit-breaker and concurrency controls in runtime planning | not yet packaged as one explicit reliability plane |
| Elevation of privilege | direct-mode drift, unsafe capability growth, plugin or bridge expansion | capability tokens, policy chain, approval gates, runtime binding, activation checks | membrane not enforced; process isolation incomplete |

## Residual Risks

The main currently documented residual risks are:

- namespace confinement exists as a concept but is not fully enforced
- the capability `membrane` field is not enforced at runtime
- durable audit exists, but tamper-evidence is still incomplete
- WASM and process isolation are still mid-journey rather than fully closed
- no first-class trust and identity plane exists yet for delegation and trust
  decay
- memory provenance and deletion governance remain incomplete

These are not hidden caveats.
They are active roadmap items and should remain explicit in future security and
adoption discussions.

## Operator Assumptions

LoongClaw's current threat posture assumes:

- operators keep policy scope narrow
- high-risk actions keep approval or allowlist boundaries
- plugin and bridge expansion are treated as governance events, not convenience
  steps
- release and trace artifacts are regenerated before strict docs governance
  checks
- deployment-specific isolation stronger than the current runtime baseline may
  still be needed for higher-trust environments

## See Also

- [Security](SECURITY.md)
- [Agent Risk Mapping](AGENT_RISK_MAPPING.md)
- [Architecture](../ARCHITECTURE.md)
- [Roadmap](ROADMAP.md)
