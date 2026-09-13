# Documentation index

## Current guides

- [Project specification](project-spec.md): current goals, defaults, scientific limits and incomplete gates.
- [Local setup](local-setup.md): WSL/Linux CUDA, dedicated environment and Windows viewer.
- [Video production](video-production.md): final V5 deliverables and local reproduction.
- [Artifact retention](artifact-retention.md): what to keep, clean and commit.
- [Neural grooming](neural-grooming.md): hybrid implementation versus biological claims.
- [Sources](../REFERENCES.md) and [licenses](../THIRD_PARTY_NOTICES.md).

## Implementation evidence (dated, not current configuration guides)

- [Windows WebGPU baseline](baseline-rtx5080-windows.md), [CUDA core](cuda-engine-stage-2026-09-12.md),
  [full-CNS parity](cuda-full-cns-stage-2026-09-12.md), [native integration](native-cuda-mujoco-stage-2026-09-12.md).
- [CUDA transfers](cuda-native-transfer-optimization-stage-2026-09-12.md),
  [chunked propagation](cuda-chunked-propagation-stage-2026-09-12.md),
  [Graph experiment](cuda-graph-experiment-2026-09-12.md),
  [physics timebase](native-timebase-stage-a-2026-09-13.md),
  [viewer bottleneck diagnosis](native-five-way-bottleneck-report-2026-09-13.md),
  [independent browser viewer](native-threejs-viewer-stage-1-2026-09-13.md).
- Indoor rebuild: [scene](indoor-v2-stage-1-result.md), [senses](indoor-v2-stage-2-result.md),
  [lifecycle](indoor-v2-stage-3-result.md), [flight/display](indoor-v2-stage-4-result.md),
  [food-search gates](indoor-v2-stage-5-result.md), [needs/grooming](indoor-v2-stage-6-result.md),
  [incomplete long-run gates](indoor-v2-stage-7-result.md).
- [Flat flower and timing changes](indoor-v2-polish-2026-09-13.md),
  [paired foreleg rubbing](forward-rub-2026-09-13.md).

Historical reports remain immutable evidence of their measured versions, including
failures. Local `outputs/` links may require retained artifacts; they are not all
downloaded by cloning Git. A later short demo or user acceptance does not turn an
earlier failed numerical/behavioral gate into a pass.

## Reference paths and superseded plans

[Original browser/WASM guide](browser.md), [browser performance](browser-performance.md),
[appearance](appearance.md), [CNS embodiment](cns-embodiment.md),
[odor guidance](cns-odor-guidance.md), [pathway assay](male-cns-pathway.md) and
[Cloudflare deployment](cloudflare.md) retain their specific runtime/model scope.
The old [six-stage roadmap](autonomous-fly-roadmap-2026-09-13.md),
[simplification plan](realtime-simplification-plan-2026-09-13.md) and
[indoor repair plan](indoor-repair-plan-2026-09-13.md) document decisions and gates;
their future-tense prose is historical, not an instruction to revert current defaults.
Earlier `stage-*` reports concern the superseded first room; `indoor-v2-stage-*`
reports concern the later rebuild. Other dated profiles remain supporting evidence.
