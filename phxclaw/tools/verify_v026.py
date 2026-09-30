#!/usr/bin/env python3
from pathlib import Path
import argparse, json, re, tomllib

def check(cond,name,fail):
    if cond: return 1
    fail.append(name); return 0

def main():
    ap=argparse.ArgumentParser(); ap.add_argument('root',type=Path); args=ap.parse_args(); r=args.root.resolve(); fail=[]; n=0
    must=[
      'requirements-release-candidate.txt','crates/phxclaw-rc-factory/Cargo.toml','crates/phxclaw-rc-factory/src/lib.rs','tools/build_release_candidate.py','tools/verify_release_candidate.py',
      'config/release-candidate.v026.json','config/sprint-gate-map.v026.json','schemas/release-candidate-v026.schema.json',
      'schemas/sprint-promotion-plan-v026.schema.json','docs/RELEASE_CANDIDATE_FACTORY_V026.md','capabilities/V026_CAPABILITIES_DELTA.json']
    for f in must: n+=check((r/f).is_file(),'missing:'+f,fail)
    cargo=tomllib.loads((r/'crates/phxclaw-rc-factory/Cargo.toml').read_text()); n+=check(cargo['package']['version']=='0.26.0','crate_version',fail); n+=check('phxclaw-release-hardening' in cargo.get('dependencies',{}),'direct_hardening_dep',fail)
    lib=(r/'crates/phxclaw-rc-factory/src/lib.rs').read_text();
    for token in ['QualificationNotReady','SourceStateMismatch','MissingNativeArtifact','RollbackRequired','verify_candidate_signature','PublicRelease','release_ready']:
        n+=check(token in lib,'rust:'+token,fail)
    cfg=json.loads((r/'config/release-candidate.v026.json').read_text()); n+=check(cfg['projected_capability_total']==276,'cap_total',fail)
    mp=json.loads((r/'config/sprint-gate-map.v026.json').read_text()); n+=check(len(mp['sprints'])==26,'sprint_count',fail)
    for s,v in mp['sprints'].items():
        n+=check(re.fullmatch(r'F(?:0[0-9]|1[0-9]|2[0-5])',s) is not None,'sprint_id:'+s,fail)
        n+=check('cargo_test' in v['required_gates'],'cargo_test_map:'+s,fail)
    py=(r/'tools/build_release_candidate.py').read_text()
    for token in ['release_ready','source_state_stable','source_bundle_sha256','qualification_run_sha256','artifact_manifest_sha256','canonical_proof_bundle_sha256','signing_payload_sha256','deterministic_zip','SPDX-2.3','https://slsa.dev/provenance/v1','not_qualified','all 26 sprints green','PHXCLAW_RELEASE_SIGN_CMD','verify_ed25519','previous-release','RC.zip','source-state symlink rejected','safe SemVer prerelease']:
        n+=check(token in py,'py:'+token,fail)
    wf=(r/'.github/workflows/phxclaw-release-candidate.yml').read_text() if (r/'.github/workflows/phxclaw-release-candidate.yml').is_file() else ''
    for token in ['previous_release_path','--previous-release','--public-rc','--strict']:
        n+=check(token in wf,'workflow:'+token,fail)
    try:
        import jsonschema
        for schema in ['schemas/release-candidate-v026.schema.json','schemas/sprint-promotion-plan-v026.schema.json']:
            jsonschema.Draft202012Validator.check_schema(json.loads((r/schema).read_text())); n+=1
    except Exception as e: fail.append('schema:'+str(e))
    print(json.dumps({'suite':'PhxClaw v0.26 static verifier','pass':n,'fail':len(fail),'failures':fail},indent=2))
    raise SystemExit(1 if fail else 0)
if __name__=='__main__': main()
