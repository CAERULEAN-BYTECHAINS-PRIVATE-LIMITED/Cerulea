# Security evidence

Every protocol control of the Cerulea chain carries evidence collected as the
control lands rather than assembled before an audit. One directory per control
sits under `evidence/`. Each directory carries a README naming the implementing
commit, the test that proves the control and the command that runs it, beside a
`last-run.txt` holding the output of the last run.

Controls implemented, certification not held.

## The controls

1. **equivocation-detection.** One validator signing two conflicting blocks for
   the same slot is detected and penalised within one epoch.
2. **checkpoint-anchoring.** Committed checkpoints settle history, so a chain
   rewritten from far below the head is refused.
3. **peer-diversity.** The peer set holds a minimum number of distinct address
   prefixes, so a node behind attacker-controlled peers halts authoring.
4. **message-bounds.** Every network-reachable decode path carries a hard size
   bound and a cost proportional to size, enforced before any work.
