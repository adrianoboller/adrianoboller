#!/usr/bin/env python3
import argparse,base64,json
from datetime import datetime,timezone,timedelta
from pathlib import Path
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from v029_common import *
ap=argparse.ArgumentParser();ap.add_argument('--tenant',required=True);ap.add_argument('--snapshot',required=True);ap.add_argument('--capability',required=True);ap.add_argument('--arguments',required=True);ap.add_argument('--secret-handles');ap.add_argument('--risk',choices=['low','medium','high','critical'],required=True);ap.add_argument('--approval-uuid');ap.add_argument('--max-parallel',type=int,required=True);ap.add_argument('--ttl-seconds',type=int,default=300);ap.add_argument('--key-id',required=True);ap.add_argument('--private-key',required=True);ap.add_argument('--policy',required=True);ap.add_argument('--out',required=True);a=ap.parse_args()
pol=load(a.policy); snap=load(a.snapshot); args=load(a.arguments); handles=load(a.secret_handles) if a.secret_handles else []
if a.max_parallel<1 or a.max_parallel>pol['max_parallel']: raise SystemExit('parallelism exceeds policy')
if a.ttl_seconds<1 or a.ttl_seconds>pol['remote_commands']['max_ttl_seconds']: raise SystemExit('ttl exceeds policy')
if contains_raw_secret(args): raise SystemExit('raw secret-like argument rejected; use Secret Broker handles')
if a.capability in pol.get('protected_capabilities',[]) and not a.approval_uuid: raise SystemExit('protected capability requires approval')
if a.risk in ('high','critical') and pol['remote_commands']['high_critical_require_approval'] and not a.approval_uuid: raise SystemExit('high/critical command requires approval')
if a.capability in ('device.system.exec','shell.exec','process.exec') and not pol['remote_commands']['arbitrary_shell']: raise SystemExit('arbitrary shell is disabled')
now=datetime.now(timezone.utc); import uuid
o={'schema_version':'0.29.0','plan_uuid':str(uuid.uuid4()),'tenant_uuid':a.tenant,'snapshot':snap,'capability':a.capability,'arguments':args,'arguments_sha256':sha_obj(args),'secret_handles':handles,'risk':a.risk,'approval_uuid':a.approval_uuid,'max_parallel':a.max_parallel,'created_at':now.isoformat().replace('+00:00','Z'),'expires_at':(now+timedelta(seconds=a.ttl_seconds)).isoformat().replace('+00:00','Z'),'idempotency_seed':sha_obj({'tenant':a.tenant,'snapshot':snap['snapshot_uuid'],'capability':a.capability,'created_at':now.isoformat()}),'policy_sha256':sha_obj(pol),'signer_key_id':a.key_id,'signature_algorithm':'ed25519'}
payload=dict(o); payload.pop('signature_algorithm'); payload.pop('signer_key_id'); priv=Ed25519PrivateKey.from_private_bytes(Path(a.private_key).read_bytes());o['signature_b64']=sign(priv,canon(payload));dump(a.out,o);print(json.dumps({'plan_uuid':o['plan_uuid'],'public_key_b64':pub_b64(priv)}))
