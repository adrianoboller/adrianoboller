#!/usr/bin/env python3
import base64,hashlib,json
from pathlib import Path
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey,Ed25519PublicKey

def canon(o): return json.dumps(o,sort_keys=True,separators=(',',':'),ensure_ascii=False).encode()
def sha_obj(o): return hashlib.sha256(canon(o)).hexdigest()
def sha_members(ids):
 h=hashlib.sha256()
 for x in sorted(set(ids)): h.update(__import__('uuid').UUID(x).bytes)
 return h.hexdigest()
def load(p): return json.loads(Path(p).read_text())
def dump(p,o): Path(p).write_text(json.dumps(o,indent=2,sort_keys=True)+'\n')
def pub_b64(priv): return base64.b64encode(priv.public_key().public_bytes_raw()).decode()
def sign(priv,payload): return base64.b64encode(priv.sign(payload)).decode()
def verify(pub_b64_text,payload,sig):
 try: Ed25519PublicKey.from_public_bytes(base64.b64decode(pub_b64_text)).verify(base64.b64decode(sig),payload); return True
 except Exception:return False

def trusted(signers,key_id,purpose):
 for s in signers.get('signers',[]):
  if s.get('enabled',True) and s.get('key_id')==key_id and purpose in s.get('purposes',[]): return s
 raise SystemExit('signer not trusted for purpose: '+purpose)
def selector_matches(s,n):
 if n.get('device_state') in ('quarantined','revoked') or n.get('control_state') in ('quarantined','revoked'): return False
 regions=set(s.get('regions',[])); ga=set(s.get('groups_any',[])); ta=set(s.get('tags_all',[])); ex=set(s.get('exclude_tags',[]))
 return (not regions or n['region'] in regions) and (not ga or bool(ga & set(n.get('groups',[])))) and ta <= set(n.get('tags',[])) and not (ex & set(n.get('tags',[])))
def membership(nodes,selector): return sorted({n['node_uuid'] for n in nodes if selector_matches(selector,n)})
def bucket(node_uuid,rollout_uuid):
 import uuid
 d=hashlib.sha256(uuid.UUID(node_uuid).bytes+uuid.UUID(rollout_uuid).bytes).digest(); return int.from_bytes(d[:2],'big')%10000

SECRET_KEYS={'password','passwd','token','api_key','apikey','secret','private_key'}
def contains_raw_secret(v):
 if isinstance(v,dict):
  for k,x in v.items():
   nk=k.lower()
   if not (nk.endswith('_secret_uuid') or nk.endswith('_lease_uuid')) and nk in SECRET_KEYS: return True
   if contains_raw_secret(x): return True
  return False
 if isinstance(v,list): return any(contains_raw_secret(x) for x in v)
 return False
