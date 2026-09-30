#!/usr/bin/env python3
from pathlib import Path
import argparse, json, re, sys, tomllib
try:
    from jsonschema import Draft202012Validator
except Exception:
    Draft202012Validator=None

CRATE='crates/phxclaw-release-qualification'

def collect(root,mode):
    checks=[]
    def ck(name,ok,detail=''): checks.append({'name':name,'pass':bool(ok),'detail':detail})
    if mode in ('all','workspace'):
        for p in sorted(root.rglob('Cargo.toml')):
            try: tomllib.loads(p.read_text()); ck('toml:'+str(p.relative_to(root)),True)
            except Exception as e: ck('toml:'+str(p.relative_to(root)),False,str(e))
        try:
            cargo=tomllib.loads((root/'Cargo.toml').read_text()); members=cargo.get('workspace',{}).get('members',[]); ck('workspace_member_once',members.count(CRATE)==1,str(members.count(CRATE)))
        except Exception as e: ck('workspace_root_parse',False,str(e))
        for rel in ['tools/source_state.py','tools/qualify_release.py','tools/verify_v025.py','crates/phxclaw-release-qualification/src/lib.rs','config/release-qualification.v025.json','config/trusted-release-signers.v025.json','requirements-release-qualification.txt']:
            ck('required:'+rel,(root/rel).exists())
        lib=(root/'crates/phxclaw-release-qualification/src/lib.rs').read_text() if (root/'crates/phxclaw-release-qualification/src/lib.rs').exists() else ''
        for token in ['QualificationRun','ReleaseAttestationV025','build_attestation','verify_attestation','SignatureRequired','InvalidSignature','verify_attestation_signature','attestation_signing_payload','signing_payload_sha256_hex','TrustedReleaseSigner','REQUIRED_GATES']:
            ck('rust_contract:'+token,token in lib)
        ck('rust_no_unsafe','unsafe {' not in lib and 'unsafe fn' not in lib)
        rcargo=tomllib.loads((root/'crates/phxclaw-release-qualification/Cargo.toml').read_text()); deps=rcargo.get('dependencies',{}); ck('rust_crypto_deps',all(k in deps for k in ['ed25519-dalek','base64','serde_json']))
        q=(root/'tools/qualify_release.py').read_text() if (root/'tools/qualify_release.py').exists() else ''
        for token in ['PHXCLAW_TEST_DATABASE_URL','PHXCLAW_RLS_DATABASE_URL','PGPASSWORD','V025_RLS_ROLE_OK','source state changed during qualification','--locked','def uuid7','source_state_sha256_hex','signing_payload_sha256','verify_trusted_ed25519','trusted-release-signers.v025.json']:
            ck('qualification_contract:'+token,token in q)
        ck('postgres_password_not_argv',"['psql',db" not in q and "['psql', db" not in q and "['psql',admin_url" not in q)
        ck('rls_admin_not_proof','admin connection cannot prove RLS' in q)
        prereq=(root/'tests/postgres/v025_rls_role_prereq.sql').read_text() if (root/'tests/postgres/v025_rls_role_prereq.sql').exists() else ''
        ck('rls_non_superuser_guard','rolsuper' in prereq and 'rolbypassrls' in prereq)
        workflow=(root/'.github/workflows/phxclaw-release-qualification.yml').read_text() if (root/'.github/workflows/phxclaw-release-qualification.yml').exists() else ''
        ck('ci_strict_qualification','--strict' in workflow)
        ck('ci_rls_database','PHXCLAW_RLS_DATABASE_URL' in workflow and 'phxclaw_rls' in workflow)
        ck('ci_supply_chain_tool','cargo install cargo-deny --locked' in workflow)
        ck('ci_upload_evidence','if: always()' in workflow and 'actions/upload-artifact@v4' in workflow)
        ck('ed25519_trust_store','TrustedReleaseSigner' in lib and 'verify_strict' in lib and 'SignerNotTrusted' in lib)
        ck('canonical_proof_hash','CanonicalProof' in lib and 'serde_json::to_vec' in lib and '8026ca26cea13700' in lib)
        cfg=json.loads((root/'config/release-qualification.v025.json').read_text()); ck('required_gate_count',len(cfg['required_gates'])==15,str(len(cfg['required_gates']))); ck('cap_total',cfg['projected_capability_total']==266); ck('config_no_password_argv',cfg['postgres'].get('passwords_in_argv') is False); ck('cargo_locked',all('--locked' in cfg['native_commands'][g] for g in ['cargo_check','cargo_test','cargo_clippy'])); ck('ed25519_config',cfg['attestation'].get('signature_algorithm')=='ed25519' and cfg['attestation'].get('trusted_signers_file')=='config/trusted-release-signers.v025.json'); ck('config_locked_cargo','--locked' in cfg['native_commands']['cargo_check'] and '--locked' in cfg['native_commands']['cargo_test'] and '--locked' in cfg['native_commands']['cargo_clippy'])
    if mode in ('all','schema'):
        schemas=sorted(root.rglob('*.schema.json')); ck('schemas_present',bool(schemas),str(len(schemas)))
        if Draft202012Validator is None: ck('jsonschema_module',False,'jsonschema not installed')
        else:
            for p in schemas:
                try: d=json.loads(p.read_text()); Draft202012Validator.check_schema(d); ck('schema:'+str(p.relative_to(root)),True)
                except Exception as e: ck('schema:'+str(p.relative_to(root)),False,str(e))
    if mode in ('all','migration'):
        migs=sorted(root.glob('migrations/*.sql')); ck('migrations_present',bool(migs),str(len(migs)))
        for p in migs:
            t=p.read_text(); ck('migration_nonempty:'+p.name,bool(t.strip()))
            # Phoenix managed migrations are transaction-wrapped in the F22+ sequence.
            if p.name >= '0021': ck('transaction:'+p.name,'BEGIN;' in t and t.rstrip().endswith('COMMIT;'))
        v24=(root/'migrations/0024_release_hardening.sql')
        if v24.exists():
            t=v24.read_text(); ck('v024_force_rls',t.count(' FORCE ROW LEVEL SECURITY;')>=24,str(t.count(' FORCE ROW LEVEL SECURITY;'))); ck('v024_composite_fk','fk_knowledge_edges_tenant_from' in t and 'fk_device_commands_tenant_node' in t); ck('v024_append_only','trg_release_evidence_immutable' in t and 'trg_release_attestations_immutable' in t)
        ck('v025_rls_test',(root/'tests/postgres/v024_rls_cross_tenant.sql').exists())
        ck('v025_attestation_test',(root/'tests/postgres/v025_release_attestation.sql').exists())
    return checks

def main():
    ap=argparse.ArgumentParser(); ap.add_argument('root',type=Path); ap.add_argument('--mode',choices=['all','workspace','schema','migration'],default='all'); args=ap.parse_args(); root=args.root.resolve()
    checks=collect(root,args.mode); passed=sum(x['pass'] for x in checks); failed=len(checks)-passed; report={'suite':f'PhxClaw v0.25 static qualification ({args.mode})','pass':passed,'fail':failed,'checks':checks}
    print(json.dumps(report,indent=2)); raise SystemExit(1 if failed else 0)
if __name__=='__main__': main()
