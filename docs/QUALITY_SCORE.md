# Quality Score

Domain grades for LoongClaw. Updated periodically to track gaps, prioritize cleanup, and measure harness maturity.

## Domain Grades

| Domain | Grade | Last Reviewed | Gaps |
|--------|-------|---------------|------|
| Contracts (L0) | A | 2026-03-13 | `#[non_exhaustive]` applied; membrane field not yet enforced at runtime |
| Kernel Security (L1) | B+ | 2026-04-06 | Core tool policy coverage is stronger, but connector/ACP/runtime-only analytics still are not uniformly routed through the same L1 decision surface |
| Execution Planes (L2) | B | 2026-03-13 | Core/extension pattern solid; no WASM fuel metering yet |
| Orchestration (L3) | B | 2026-03-13 | HarnessBroker routes correctly; context-engine selection is pluggable, but richer engine implementations and broader runtime coverage are still limited |
| Observability (L4) | B- | 2026-04-06 | Durable JSONL retention exists, but tamper-evidence, richer query surfaces, and broader SIEM/export adapters remain incomplete |
| Vertical Packs (L5) | B | 2026-03-13 | Pack validation works; namespace struct exists but not enforced |
| Protocol (L5.5) | B+ | 2026-03-13 | Transport contracts and typed routing operational |
| Integration (L6) | B | 2026-03-13 | Plugin scanning works; hotplug lifecycle incomplete |
| Plugin IR (L7) | B- | 2026-03-13 | Bridge inference works; multi-language support limited |
| Self-Awareness (L8) | B- | 2026-03-13 | Snapshots generated but not continuous; no drift detection agent |
| Bootstrap (L9) | B | 2026-03-13 | Activation plans work; no policy-bounded bootstrap validation |
| Context/Memory | C+ | 2026-04-06 | Runtime-self continuity, durable recall, and context-engine seams exist, but governed external memory providers, provenance-ranked recall, and retrieval evaluation still are not there |
| Skills / Capability Registry | B- | 2026-04-06 | Managed and bundled skills exist, but there is still no unified governed registry with progressive disclosure across discovery, install, and assistant-visible invocation |
| Experiment / Evaluator Loop | C- | 2026-04-06 | Snapshot, experiment, and capability records exist, but staged evaluator runs, keep/discard evidence, and learning-ready feedback artifacts are still missing |
| Learning Architecture | C- | 2026-04-06 | The runtime has the right experiment and evidence primitives, but there is still no normalized next-state feedback schema or separated evaluator/trainer pipeline above live serving |
| Documentation | A- | 2026-03-13 | Strong coverage across design docs, security, product sense, and quality tracking |
| CI/Enforcement | A | 2026-03-13 | 8 CI workflows, convention-engineering (14 files, 11 checks), check:harness mirror gate |
| Contributor Experience | A- | 2026-03-13 | Clear tracks and recipes; could add more examples |

## Grading Criteria

- **A**: Full test coverage, no known debt, documentation current, mechanical enforcement
- **B**: Adequate coverage, minor debt tracked, docs mostly current
- **C**: Coverage gaps, significant debt, stale or missing docs
- **D**: Minimal coverage, blocking debt, docs unreliable
- **F**: Untested, untracked, undocumented

## Harness Maturity Assessment

| Criterion | Status |
|-----------|--------|
| Agent entry point (AGENTS.md) | Present, 102 lines, mirrored with CLAUDE.md |
| Architecture defined with enforcement | Present, DAG + boundary checks + CI |
| Progressive disclosure hierarchy | Present, 3-tier structure |
| Mechanical enforcement | 8 CI workflows, convention-engineering (14 files, 11 content checks), check:harness, pre-commit |
| Quality tracked | This file |
| External context captured | Core beliefs principle #8 requires it |
