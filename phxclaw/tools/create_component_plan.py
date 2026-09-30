#!/usr/bin/env python3
from pathlib import Path
import argparse,json,sys
sys.path.insert(0,str(Path(__file__).resolve().parent)); from v028_common import *
def main():
 ap=argparse.ArgumentParser(); ap.add_argument('root',type=Path); ap.add_argument('--input',type=Path,required=True); ap.add_argument('--out',type=Path,required=True); a=ap.parse_args(); root=a.root.resolve(); src=load_json(a.input)
 if set(src)-{'target_core_version','components'}: raise SystemExit('unsigned component input has unsupported fields')
 o={'schema_version':'0.28.0','target_core_version':src['target_core_version'],'policy_sha256':sha_file(root/'config/component-update-policy.v028.json'),'components':src['components'],'signer_key_id':'','signature_algorithm':'ed25519','signature_b64':''}; sig=sign_fleet(root,component_plan_payload(o),'fleet.component_plan','component-plan'); o.update(sig); write_json(a.out,o); print(json.dumps({'status':'signed','components':len(o['components']),'out':str(a.out)},indent=2))
if __name__=='__main__': main()
