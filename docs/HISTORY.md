# History of this repository

Two audit documents once sat in this folder. Both described a design Cerulea
has retired. Both are removed. This page names what they were and where the
record that replaced them lives.

## What the two documents were

1. **AUDIT_CHAIN.md.** A deep audit of the chain tree dated 11 July 2026. The
   audit listed defects in the consensus tree of the earlier design, named the
   scoring model that has since been replaced, and used the sub-mechanism names
   Cerulea has retired.

2. **AUDIT_STUDIO.md.** A deep audit of the Studio workspace when Studio lived
   inside this repository. Cerulea Studio now lives in its own repository and
   the layout the audit describes is gone.

## Where the record lives now

1. **Defect closure.** `docs/DEFECT_CLOSURE.md` carries every defect the two
   audits found, the state of each one and the task that closes it. That
   document replaced the earlier defect report and the earlier list of proposed
   solutions.

2. **The consensus design.** `docs/CONSENSUS_ARCHITECTURE.md` describes the
   Dynamic Consensus Framework and its eight engines in Cerulea terms.

3. **The reading surface.** `docs/NODE_INTERFACE.md` describes every method the
   node publishes and the test that covers each one.

4. **The network bounds.** `docs/NETWORK_BOUNDS.md` lists each decode path a
   stranger reaches and the bound on each one.

5. **The build baseline and the test baseline.** `docs/BUILD_BASELINE.md` and
   `docs/TEST_BASELINE.md` record the measured state of the tree.

## What replaced the earlier deployment material

The deployment layer now lives outside this repository. `scripts/deploy/` in
the scripts repository carries the image descriptions, the deployment
definition, the self-hosting package and the operator commands. The setup
guide sits at `scripts/deploy/docs/SELF_HOSTING.md`.
