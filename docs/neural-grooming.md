# Neural grooming: hybrid implementation and scientific boundary

The indoor runtime now implements autonomous, CNS-gated combined foreleg rubbing
and head/eye cleaning. An engineered grooming urge, satiety and support checks
request the action; neural evidence and motor output gate execution. A four-second
engineered trajectory includes about 1.5 seconds of extended, paired foreleg rubbing.
It is not a fixed after-meal animation or a recovered complete biological circuit.
Manual `H` remains a separate pose diagnostic, not evidence of neural triggering.
See the [current specification](project-spec.md), [causal stage results](indoor-v2-stage-6-result.md)
and [measured foreleg geometry](forward-rub-2026-09-13.md).

## Original candidate rationale

The following candidate inventory and scientific limitations predate the hybrid
implementation. Candidate presence is not proof that a natural grooming circuit
has been recovered.

The exact MaleCNS v1.0 annotation file at
`work/upstream/male-cns/v1.0/body-annotations-male-cns-v1.0-minconf-0.5.feather`
contains these candidate descending neurons. All are present in the 166,700-ID
pack at `outputs/packs/male_cns_v1/neuron_ids.npy`:

| Annotation | MaleCNS body IDs | Candidate role |
|---|---|---|
| `DNg12_e`, MANC type `DNg12` | 20129, 22190, 22474, 22564, 22596, 55021 | Literature-supported anterior-grooming candidates |
| `DNg62`, synonym aDN1 | 13624, 15148 | Antennal grooming candidates |
| `DNge078`, MANC type `DNfl023`, synonym aDN2 | 14537, 36541 | Antennal grooming candidates |

These are MaleCNS body IDs, not IDs copied from the separate FlyWire grooming
dataset. MANC names are annotation columns in the MaleCNS source.

The implementation gap is not simply a missing trigger. A defensible direct
connection still requires directed, signed route verification through premotor
and T1 motor populations, a calibrated sensory input, and an actuator mapping.
The physical model has head, mouthpart, and antennal joints; their existence
does not supply that mapping. The current hybrid implementation still supplies
engineered trajectories on the leg-control channels rather than deriving each
joint target from a validated premotor-to-muscle model.

Validation must compare intact and targeted-silencing/stimulation conditions
and measure grooming-specific leg/head contact and kinematics. Neither higher
descending-neuron activity nor visible leg motion alone establishes grooming.
Reusing walking outputs or firing a timed post-meal animation would not meet
the direct-brain requirement.

## Experimental grounding

[Sapkal et al., Nature (2024)](https://www.nature.com/articles/s41586-024-07854-7)
report DNg12 input to front-leg brake neurons and distinguish grooming-related
stabilization from feeding-related walking suppression. This supports checking
both the grooming command route and stance stabilization, not treating either
alone as a complete grooming controller.

[Inhibitory circuits control leg movements during Drosophila grooming](https://elifesciences.org/articles/106446)
describes aDN/DNg12 inputs to inhibitory 13A circuits, antagonistic motor-neuron
targets, and proprioceptive feedback. These are candidate mechanisms for an
experimental neural embodiment; they do not validate a direct transfer of IDs,
weights, muscle gains, or kinematics to this MaleCNS model.
