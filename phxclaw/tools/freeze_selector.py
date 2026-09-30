#!/usr/bin/env python3
import argparse,uuid
from datetime import datetime,timezone
from v029_common import *
ap=argparse.ArgumentParser();ap.add_argument('--tenant',required=True);ap.add_argument('--selector',required=True);ap.add_argument('--inventory',required=True);ap.add_argument('--out',required=True);a=ap.parse_args()
s=load(a.selector); nodes=load(a.inventory); ids=membership(nodes,s)
o={'snapshot_uuid':str(uuid.uuid7()) if hasattr(uuid,'uuid7') else str(uuid.uuid4()),'tenant_uuid':a.tenant,'selector':s,'selector_sha256':sha_obj(s),'membership_sha256':sha_members(ids),'member_count':len(ids),'created_at':datetime.now(timezone.utc).isoformat().replace('+00:00','Z')}
dump(a.out,o);print(json.dumps({'members':len(ids),'membership_sha256':o['membership_sha256']}))
