#!/usr/bin/env python3
from pathlib import Path
import json, sys, tomllib
ROOT=Path(__file__).resolve().parents[1]
OVER=ROOT/'overlay'
checks=[]
def ck(name,cond,detail=''): checks.append({'name':name,'pass':bool(cond),'detail':str(detail)})
def text(rel): return (OVER/rel).read_text(encoding='utf-8')
def has_all(s,*xs): return all(x in s for x in xs)
def main():
    required=[
      'crates/phxclaw-ga-release-manager/Cargo.toml','crates/phxclaw-ga-release-manager/src/lib.rs',
      'config/multi-platform-qualification.v027.json','config/updater-policy.v027.json',
      'tools/v027_common.py','tools/verify_v027.py','tools/run_platform_gates.py','tools/create_platform_evidence.py',
      'tools/aggregate_platform_qualifications.py','tools/build_ga_release.py','tools/verify_ga_release.py',
      'tools/verify_update_manifest.py','tools/create_rollback_authorization.py',
      'schemas/platform-qualification-v027.schema.json','schemas/multi-platform-qualification-v027.schema.json',
      'schemas/ga-release-v027.schema.json','schemas/update-manifest-v027.schema.json',
      'schemas/rollback-authorization-v027.schema.json','schemas/compatibility-n-n1-v027.schema.json',
      'capabilities/V027_CAPABILITIES_DELTA.json','docs/PROJECT_STATUS_V027.json','docs/MULTI_PLATFORM_GA_V027.md',
      'requirements-ga.txt','.github/workflows/phxclaw-ga-release.yml','ci/run-platform-qualification.sh','ci/run-platform-qualification.ps1','ci/run-ga-release.sh','ci/run-ga-release.ps1'
    ]
    for r in required: ck('file_'+r,(OVER/r).is_file())
    for p in sorted(OVER.rglob('*.json')):
        try: json.loads(p.read_text(encoding='utf-8')); ck('json_'+str(p.relative_to(OVER)),True)
        except Exception as e: ck('json_'+str(p.relative_to(OVER)),False,e)
    try: tomllib.loads((OVER/'crates/phxclaw-ga-release-manager/Cargo.toml').read_text(encoding='utf-8')); ck('crate_toml',True)
    except Exception as e: ck('crate_toml',False,e)

    lib=text('crates/phxclaw-ga-release-manager/src/lib.rs')
    for token in ['verify_platform_evidence','verify_ed25519','verify_update_transition','SequenceNotMonotonic','InvalidMetadataLifetime','RollbackAuthorizationRequired','InvalidVersionTransition','rc_archive_sha256','first_release','previous_version']:
        ck('rust_'+token,token in lib)

    common=text('tools/v027_common.py')
    for token in ['phxclaw-platform-qualification-v027','phxclaw-multiplatform-qualification-v027','phxclaw-ga-release-v027','phxclaw-update-manifest-v027','phxclaw-rollback-authorization-v027']:
        ck('payload_'+token,token in common)
    ck('common_trust_enabled',"x.get('enabled',True) is True" in common)
    ck('common_safe_zip',has_all(common,'duplicate archive path','unsafe archive path','non-regular file rejected while packaging'))

    cfg=json.loads(text('config/multi-platform-qualification.v027.json'))
    expected=[('linux','x86_64','ubuntu-24.04'),('windows','x86_64','windows-2025'),('macos','aarch64','macos-15')]
    actual=[(x.get('platform'),x.get('architecture'),x.get('github_runner')) for x in cfg.get('required_platforms',[])]
    ck('config_exact_native_platforms',actual==expected,actual)
    ck('config_fail_closed',cfg.get('mode')=='fail_closed')
    ck('config_same_rc_required',cfg.get('requirements',{}).get('same_rc_archive') is True)
    ck('config_native_origin_required',cfg.get('requirements',{}).get('platform_evidence_must_originate_from_native_runner') is True)

    pg=text('tools/run_platform_gates.py')
    ck('runner_rehashes_workspace',has_all(pg,'source_state.py','native checkout/RC identity mismatch'))
    ck('runner_verifies_rc',has_all(pg,'verify_release_candidate.py','rc_archive_sha256'))
    ck('runner_origin_marker',"phxclaw-native-platform-runner-v027" in pg)
    ck('runner_actual_os_arch',has_all(pg,'platform.system()','platform.machine()'))
    ck('windows_authenticode_native',has_all(pg,'Get-AuthenticodeSignature','TimeStamperCertificate','signtool','PHXCLAW_WINDOWS_SIGNER_THUMBPRINTS'))
    ck('macos_native_signing',has_all(pg,'codesign','spctl','stapler','TeamIdentifier','PHXCLAW_MACOS_TEAM_IDS','signed/notarized DMG','context:primary-signature'))
    ck('runner_six_logs',all(x in pg for x in ['native-tests.log','fresh-install.log','upgrade-n-minus-1.log','rollback-or-restore.log','code-signing.log','notarization.log']))

    ce=text('tools/create_platform_evidence.py')
    ck('evidence_requires_native_origin',"status.json was not produced by native platform runner" in ce)
    ck('evidence_rehashes_workspace',"current source-state differs from native runner status" in ce)
    ck('evidence_artifact_bound',"artifact differs from native runner status" in ce)
    ck('evidence_logs_bound',"evidence log differs from runner status" in ce)

    agg=text('tools/aggregate_platform_qualifications.py')
    ck('aggregate_exact_platform_set',has_all(agg,"set(keys)!=required","len(keys)!=len(set(keys))"))
    ck('aggregate_same_identity',has_all(agg,"len(sources)!=1","len(candidates)!=1","len(rcs)!=1","len(prev)!=1"))
    ck('aggregate_signature_verify',has_all(agg,'verify_ed25519','aggregate_payload'))
    ck('aggregate_rc_bound','rc_archive_sha256' in agg)

    build=text('tools/build_ga_release.py')
    ck('ga_verifies_rc',has_all(build,'verify_release_candidate.py','verify_rc'))
    ck('ga_exact_rc_digest',"aggregate is bound to a different RC archive" in build)
    ck('ga_version_transition',has_all(build,'Version(',"first release must not claim previous version/sequence"))
    ck('ga_sequence_monotonic',"GA sequence must be greater than previous-sequence" in build)
    ck('ga_https_only',"--base-url must use HTTPS" in build)
    ck('ga_artifact_bound',"artifact differs from signed platform evidence" in build)
    ck('ga_stable_update',"'channel':'stable'" in build)
    ck('ga_artifact_manifest_bound','artifact_manifest_sha256' in build)

    vg=text('tools/verify_ga_release.py')
    ck('ga_safe_zip',has_all(vg,'unsafe GA zip entry','0o120000','duplicate GA zip entry'))
    ck('ga_embedded_rc_verify',has_all(vg,'verify_release_candidate.py','release-candidate.zip'))
    ck('ga_update_bound',"update manifest not bound to stable GA" in vg)
    ck('ga_identity_bound',"GA identity mismatch" in vg)
    ck('ga_duplicate_artifact_rejected',has_all(vg,'duplicate GA artifact target','seen.add(k)'))

    upd=text('tools/verify_update_manifest.py')
    ck('update_monotonic',"update sequence is not monotonic" in upd)
    ck('update_downgrade_failclosed',"downgrade denied without trusted rollback authorization" in upd)
    ck('update_artifact_digest',"downloaded artifact digest/size mismatch" in upd)
    ck('update_https_only','must use HTTPS' in upd or "startswith('https://')" in upd)
    ck('update_rollback_scope',all(x in upd for x in ['from_version','to_version','from_sequence','to_sequence']))

    wf=text('.github/workflows/phxclaw-ga-release.yml')
    ck('workflow_exact_runners',all(x in wf for x in ['ubuntu-24.04','windows-2025','macos-15']))
    ck('workflow_exact_architectures',all(x in wf for x in ['architecture: x86_64','architecture: aarch64']))
    ck('workflow_macos_dmg','artifact: dist/macos/PhxClaw.dmg' in wf)
    ck('workflow_rc_https',has_all(wf,"--proto '=https'",'rc_sha256'))
    ck('workflow_native_gate','run_platform_gates.py' in wf)
    ck('workflow_independent_ga_verify','verify_ga_release.py' in wf)
    ck('workflow_locked_build','cargo build --locked --release --workspace' in wf)
    ck('legacy_runner_absent',not (OVER/'tools/run_native_platform_qualification.py').exists())
    ck('ci_platform_launcher_strong','run_platform_gates.py' in text('ci/run-platform-qualification.sh') and 'run_platform_gates.py' in text('ci/run-platform-qualification.ps1'))
    ck('ci_ga_previous_sequence','--previous-sequence' in text('ci/run-ga-release.sh') and '--previous-sequence' in text('ci/run-ga-release.ps1'))

    cap=json.loads(text('capabilities/V027_CAPABILITIES_DELTA.json'))
    ck('capability_total',cap.get('base_total')==276 and cap.get('projected_total')==290 and len(cap.get('added',[]))==14,cap)
    ck('capability_unique',len(cap.get('added',[]))==len(set(cap.get('added',[]))))
    ps=json.loads(text('docs/PROJECT_STATUS_V027.json'))
    ck('status_not_fake_release',ps.get('release_ready') is False and ps.get('e2e_verified') is False)

    report={'suite':'PhxClaw v0.27 static verifier','pass':sum(x['pass'] for x in checks),'fail':sum(not x['pass'] for x in checks),'checks':checks}
    out=ROOT/'reports/V027_STATIC_VERIFY_REPORT.json'; out.parent.mkdir(exist_ok=True); out.write_text(json.dumps(report,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
    print(json.dumps({'pass':report['pass'],'fail':report['fail'],'report':str(out)},indent=2))
    if report['fail']:
        for x in checks:
            if not x['pass']: print('FAIL',x['name'],x['detail'],file=sys.stderr)
    return 1 if report['fail'] else 0
if __name__=='__main__': raise SystemExit(main())
