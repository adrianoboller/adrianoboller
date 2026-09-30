#!/usr/bin/env python3
from pathlib import Path
import json,sys,tomllib
ROOT=Path(__file__).resolve().parents[1];OVER=(ROOT/'overlay' if (ROOT/'overlay/crates/phxclaw-fleet-control-plane').exists() else ROOT);checks=[]
def ck(n,c,d=''): checks.append({'name':n,'pass':bool(c),'detail':str(d)})
def tx(r): return (OVER/r).read_text(encoding='utf-8')
def main():
 req=['crates/phxclaw-fleet-control-plane/Cargo.toml','crates/phxclaw-fleet-control-plane/src/lib.rs','config/fleet-control-policy.v029.json','config/trusted-control-signers.v029.json','migrations/0029_fleet_control_plane.sql','tools/v029_common.py','tools/freeze_selector.py','tools/select_control_nodes.py','tools/evaluate_heartbeat.py','tools/assess_recovery.py','tools/scope_rollout.py','tools/create_control_plan.py','tools/verify_control_plan.py','tools/verify_v029.py','schemas/fleet-selector-v029.schema.json','schemas/fleet-inventory-node-v029.schema.json','schemas/fleet-selector-snapshot-v029.schema.json','schemas/fleet-command-plan-v029.schema.json','schemas/fleet-maintenance-v029.schema.json','schemas/fleet-recovery-observation-v029.schema.json','schemas/fleet-control-state-v029.schema.json','capabilities/V029_CAPABILITIES_DELTA.json','docs/PROJECT_STATUS_V029.json','docs/FLEET_CONTROL_PLANE_V029.md','.github/workflows/phxclaw-fleet-control-plane.yml','ci/run-fleet-control-plane.sh','ci/run-fleet-control-plane.ps1']
 for r in req: ck('file_'+r,(OVER/r).is_file())
 for p in OVER.rglob('*.json'):
  try:json.loads(p.read_text());ck('json_'+str(p.relative_to(OVER)),True)
  except Exception as e:ck('json_'+str(p.relative_to(OVER)),False,e)
 try:tomllib.loads(tx('crates/phxclaw-fleet-control-plane/Cargo.toml'));ck('crate_toml',True)
 except Exception as e:ck('crate_toml',False,e)
 lib=tx('crates/phxclaw-fleet-control-plane/src/lib.rs')
 for t in ['InventoryNode','FleetSelector','SelectorSnapshot','freeze_selector','verify_snapshot','MaintenanceWindow','SignedControlPlanDocument','verify_signed_control_plan','signed_control_plan_payload','plan_fanout','authorize_command','selected_for_percent','scoped_rollout_members','RecoveryObservation','assess_recovery','SequenceRegression','MembershipDrift','FencingMismatch','session_fencing_token']:ck('rust_'+t,t in lib)
 ck('depends_f22','phxclaw-device-nodes' in tx('crates/phxclaw-fleet-control-plane/Cargo.toml'))
 ck('depends_v028','phxclaw-fleet-updater' in tx('crates/phxclaw-fleet-control-plane/Cargo.toml'))
 pol=json.loads(tx('config/fleet-control-policy.v029.json'));ck('no_shell',pol['remote_commands']['arbitrary_shell'] is False);ck('f22_boundary',pol['remote_commands']['use_f22_device_command_contract'] is True);ck('snapshot_required',pol['rollout_scope']['membership_snapshot_required'] is True);ck('pg_fencing',pol['control_lease']['postgresql_cas_fencing'] is True)
 sql=tx('migrations/0029_fleet_control_plane.sql');ck('force_rls','FORCE ROW LEVEL SECURITY' in sql and sql.count("'fleet_")>=11);ck('composite_fks',sql.count('FOREIGN KEY(tenant_uuid,')+sql.count('FOREIGN KEY (tenant_uuid,')>=10);ck('device_fk','REFERENCES phxclaw.device_nodes(tenant_uuid,node_uuid)' in sql);ck('fleet_fk','REFERENCES phoenix_fleet_nodes(tenant_uuid,node_uuid)' in sql);ck('lease_cas',all(x in sql for x in ['phoenix_fleet_control_claim_lease','p_expected_fencing_token','stale or busy fleet-control fencing token','SECURITY INVOKER']));ck('plan_cas',all(x in sql for x in ['phoenix_fleet_remote_plan_transition','stale remote-plan fencing token']));ck('append_only',all(x in sql for x in ['fleet_selector_snapshot_append_only','fleet_recovery_evidence_append_only','fleet_control_events_append_only']) and sql.count('EXECUTE FUNCTION phoenix_append_only_guard()')>=3)
 common=tx('tools/v029_common.py');ck('selector_logic',all(x in common for x in ['groups_any','tags_all','exclude_tags','membership','bucket']));ck('selector_excludes_blocked',all(x in common for x in ["'quarantined'","'revoked'"]) and 'DeviceState::Quarantined | DeviceState::Revoked' in lib)
 ck('drift_guard','membership drift detected' in tx('tools/select_control_nodes.py') and 'membership drift detected' in tx('tools/scope_rollout.py'))
 ck('recovery_guards',all(x in tx('tools/assess_recovery.py') for x in ['sequence regression','fencing mismatch','recovery evidence too old']))
 cp=tx('tools/create_control_plan.py');vp=tx('tools/verify_control_plan.py');ck('signed_control_plan','Ed25519PrivateKey' in cp and 'fleet.control.plan' in vp);ck('policy_bound','policy_sha256' in cp and 'control policy changed after plan signing' in vp);ck('arguments_bound','arguments_sha256' in cp and 'arguments hash mismatch' in vp);ck('shell_reject','arbitrary shell is disabled' in cp);ck('raw_secret_reject','raw secret-like argument rejected' in cp and 'contains_raw_secret' in tx('tools/v029_common.py'));ck('secret_handles_bound',"'secret_handles':handles" in cp);ck('fencing_separated','fencing_token:inv.session_fencing_token' in lib);ck('rust_requires_signed_plan','plan:&SignedControlPlanDocument' in lib and 'verify_signed_control_plan(plan,expected_policy_sha256,signers,now)?' in lib and 'fleet.control.plan' in lib)
 cap=json.loads(tx('capabilities/V029_CAPABILITIES_DELTA.json'));ck('cap_total',cap['base_total']==308 and cap['projected_total']==330 and len(cap['added'])==22);ck('cap_unique',len(cap['added'])==len(set(cap['added'])))
 st=json.loads(tx('docs/PROJECT_STATUS_V029.json'));ck('sprint_colors',st['sprints']=={'green':0,'yellow':26,'red':0});ck('no_fake_release',st['release_ready'] is False and st['e2e_verified'] is False)
 wf=tx('.github/workflows/phxclaw-fleet-control-plane.yml');ck('manual_workflow','workflow_dispatch' in wf and 'push:' not in wf and 'schedule:' not in wf);ck('production_env','environment: fleet-production' in wf);ck('no_auto_side_effect','if: ${{ false }}' in wf)
 rep={'suite':'PhxClaw v0.29 static verifier','pass':sum(x['pass'] for x in checks),'fail':sum(not x['pass'] for x in checks),'checks':checks};(ROOT/'reports').mkdir(exist_ok=True);(ROOT/'reports/V029_STATIC_VERIFY_REPORT.json').write_text(json.dumps(rep,indent=2)+'\n');print(json.dumps({'pass':rep['pass'],'fail':rep['fail'],'report':str(ROOT/'reports/V029_STATIC_VERIFY_REPORT.json')},indent=2));
 if rep['fail']:
  for x in checks:
   if not x['pass']:print('FAIL',x['name'],x['detail'],file=sys.stderr)
 return 1 if rep['fail'] else 0
if __name__=='__main__':raise SystemExit(main())
