# v0.53 → v0.55 compatibility repair

The surviving v0.53 package installs `phxclaw-adaptive-portfolio-execution-controller`, while the v0.55 apply script checks `phxclaw-adaptive-portfolio-controller`.

The consolidated checkout keeps the v0.53 implementation unchanged and adds a facade crate under the shorter historical name.

Status: `CONSOLIDATION_REPAIR`; not claimed as a byte-identical historical artifact.
