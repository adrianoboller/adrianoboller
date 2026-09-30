from __future__ import annotations
from pathlib import Path
from datetime import datetime, timezone
import base64, hashlib, json, os, shlex, subprocess, uuid

def utcnow(): return datetime.now(timezone.utc).isoformat().replace('+00:00','Z')
def sha_bytes(b:bytes)->str: return hashlib.sha256(b).hexdigest()
def sha_file(p:Path)->str:
    h=hashlib.sha256()
    with p.open('rb') as f:
        for c in iter(lambda:f.read(1024*1024),b''): h.update(c)
    return h.hexdigest()
def load_json(p:Path):
    if p.is_symlink() or not p.is_file(): raise SystemExit('JSON input must be regular non-symlink: '+str(p))
    return json.loads(p.read_text(encoding='utf-8'))
def write_json(p:Path,o): p.parent.mkdir(parents=True,exist_ok=True); p.write_text(json.dumps(o,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
def uuid7():
    import time, secrets
    ms=int(time.time()*1000)&((1<<48)-1); rnd=secrets.randbits(74)
    return str(uuid.UUID(int=(ms<<80)|(0x7<<76)|(((rnd>>62)&0xfff)<<64)|(0b10<<62)|(rnd&((1<<62)-1))))
def parse_time(s): return datetime.fromisoformat(s.replace('Z','+00:00'))
def fleet_signers(root:Path): return [x for x in load_json(root/'config/trusted-fleet-signers.v028.json').get('signers',[]) if x.get('enabled',True) is True]
def verify_fleet(root:Path,key_id:str,purpose:str,sig_b64:str,payload:bytes,label='fleet signature'):
    s=next((x for x in fleet_signers(root) if x.get('key_id')==key_id and purpose in x.get('purposes',[])),None)
    if s is None: raise SystemExit(f'{label}: signer not trusted for {purpose}')
    try:
        from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey
        pub=base64.b64decode(s['public_key_b64'],validate=True); sig=base64.b64decode(sig_b64,validate=True)
        if len(pub)!=32 or len(sig)!=64: raise ValueError('invalid Ed25519 length')
        Ed25519PublicKey.from_public_bytes(pub).verify(sig,payload)
    except Exception as e: raise SystemExit(f'{label}: invalid signature: {e}')
def sign_fleet(root:Path,payload:bytes,purpose:str,label:str):
    raw=os.environ.get('PHXCLAW_FLEET_SIGN_CMD','').strip()
    if not raw: raise SystemExit('PHXCLAW_FLEET_SIGN_CMD is required')
    d=root/'var/fleet-signing'; d.mkdir(parents=True,exist_ok=True); p=d/(label+'.payload.bin'); p.write_bytes(payload)
    r=subprocess.run(shlex.split(raw)+[str(p),purpose],cwd=root,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,timeout=300)
    if r.returncode: raise SystemExit(label+' signer failed: '+r.stderr[:800])
    try:o=json.loads(r.stdout)
    except Exception as e: raise SystemExit(label+f' signer output invalid: {e}')
    if o.get('signature_algorithm','ed25519')!='ed25519': raise SystemExit('fleet signature algorithm must be ed25519')
    verify_fleet(root,o['signer_key_id'],purpose,o['signature_b64'],payload,label); return o
def rollout_payload(o):
    stages_hash=sha_bytes(json.dumps(o['stages'],separators=(',',':'),ensure_ascii=False).encode())
    health_hash=sha_bytes(json.dumps(o['health_thresholds'],separators=(',',':'),ensure_ascii=False).encode())
    critical_hash=sha_bytes(json.dumps(o['critical_thresholds'],separators=(',',':'),ensure_ascii=False).encode())
    return (f"phxclaw-fleet-rollout-v028\nrollout_uuid={o['rollout_uuid']}\ntenant_uuid={o['tenant_uuid']}\nchannel={o['channel']}\nversion={o['version']}\nsequence={o['sequence']}\nsource_state_sha256={o['source_state_sha256'].lower()}\nupdate_manifest_sha256={o['update_manifest_sha256'].lower()}\npolicy_sha256={o['policy_sha256'].lower()}\nhealth_thresholds_sha256={health_hash}\ncritical_thresholds_sha256={critical_hash}\nhealth_evidence_max_age_seconds={o['health_evidence_max_age_seconds']}\nstages_sha256={stages_hash}\ncreated_at={o['created_at']}\nexpires_at={o['expires_at']}\n").encode()
def health_payload(o):
    return (f"phxclaw-fleet-health-v028\nevidence_uuid={o['evidence_uuid']}\nrollout_uuid={o['rollout_uuid']}\nstage_index={o['stage_index']}\nsamples={o['samples']}\nsuccess_rate={float(o['success_rate']):.9f}\ncrash_rate={float(o['crash_rate']):.9f}\nrollback_rate={float(o['rollback_rate']):.9f}\nstale_heartbeat_rate={float(o['stale_heartbeat_rate']):.9f}\ninstall_error_rate={float(o['install_error_rate']):.9f}\nwindow_started_at={o['window_started_at']}\ncreated_at={o['created_at']}\n").encode()
def state_payload(o):
    return (f"phxclaw-fleet-state-v028\nrollout_uuid={o['rollout_uuid']}\ngeneration={o['generation']}\nfencing_token={o['fencing_token']}\nstate={o['state']}\nstage_index={o['stage_index']}\nupdated_at={o['updated_at']}\nreason_sha256={sha_bytes(o.get('reason','').encode())}\n").encode()
def cohort_bucket(node_uuid:str,rollout_uuid:str)->int:
    b=uuid.UUID(node_uuid).bytes+uuid.UUID(rollout_uuid).bytes; return int.from_bytes(hashlib.sha256(b).digest()[:2],'big')%10000

def rollback_payload(o):
    return (f"phxclaw-fleet-rollback-v028\nauthorization_uuid={o['authorization_uuid']}\nrollout_uuid={o['rollout_uuid']}\nfrom_version={o['from_version']}\nto_version={o['to_version']}\nfrom_sequence={o['from_sequence']}\nto_sequence={o['to_sequence']}\nmax_nodes={o['max_nodes']}\nreason_sha256={sha_bytes(o['reason'].encode())}\ncreated_at={o['created_at']}\nexpires_at={o['expires_at']}\n").encode()
def db_contract_payload(o):
    return (f"phxclaw-database-contract-approval-v028\napproval_uuid={o['approval_uuid']}\ncomponent_plan_sha256={o['component_plan_sha256'].lower()}\nfleet_state_sha256={o['fleet_state_sha256'].lower()}\ntarget_core_version={o['target_core_version']}\nfleet_compatible_percent={o['fleet_compatible_percent']}\ncreated_at={o['created_at']}\nexpires_at={o['expires_at']}\n").encode()

def component_plan_payload(o):
    components_hash=sha_bytes(json.dumps(o['components'],separators=(',',':'),ensure_ascii=False).encode())
    return (f"phxclaw-component-plan-v028\ntarget_core_version={o['target_core_version']}\npolicy_sha256={o['policy_sha256'].lower()}\ncomponents_sha256={components_hash}\n").encode()
