#!/usr/bin/env python3
import argparse,json
from datetime import datetime,timezone
from v029_common import *
ap=argparse.ArgumentParser();ap.add_argument('--plan',required=True);ap.add_argument('--policy',required=True);ap.add_argument('--signers',required=True);a=ap.parse_args();o=load(a.plan);p=load(a.policy);s=load(a.signers)
if o.get('signature_algorithm')!='ed25519': raise SystemExit('signature algorithm rejected')
if o['policy_sha256']!=sha_obj(p): raise SystemExit('control policy changed after plan signing')
if o['snapshot']['selector_sha256']!=sha_obj(o['snapshot']['selector']): raise SystemExit('selector hash mismatch')
if o['arguments_sha256']!=sha_obj(o['arguments']): raise SystemExit('arguments hash mismatch')
if datetime.fromisoformat(o['expires_at'].replace('Z','+00:00'))<=datetime.now(timezone.utc): raise SystemExit('control plan expired')
signer=trusted(s,o['signer_key_id'],'fleet.control.plan'); payload=dict(o); sig=payload.pop('signature_b64');payload.pop('signature_algorithm');payload.pop('signer_key_id')
if not verify(signer['public_key_b64'],canon(payload),sig): raise SystemExit('invalid control-plan signature')
print(json.dumps({'verified':True,'plan_uuid':o['plan_uuid']}))
