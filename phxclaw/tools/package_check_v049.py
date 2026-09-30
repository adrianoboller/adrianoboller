#!/usr/bin/env python3
from pathlib import Path
import json,sys,re,subprocess
ROOT=Path(__file__).resolve().parents[1];checks=[]
def ck(n,c,d=''):checks.append({'name':n,'pass':bool(c),'detail':d})
for p in ROOT.rglob('*.json'):
    try:json.loads(p.read_text());ck('json_'+str(p.relative_to(ROOT)),True)
    except Exception as e:ck('json_'+str(p.relative_to(ROOT)),False,str(e))
for p in ROOT.rglob('*'):
    if p.is_symlink():ck('no_symlink_'+str(p.relative_to(ROOT)),False)
html=ROOT/'overlay/ui/executive-decision-center.html';ck('ui_html',html.exists())
if html.exists():
    m=re.search(r'<script>(.*?)</script>',html.read_text(),re.S)
    if m:
        js=ROOT/'reports/_v049_ui.js';js.write_text(m.group(1));res=subprocess.run(['node','--check',str(js)],capture_output=True,text=True);ck('ui_js_syntax',res.returncode==0,res.stderr);js.unlink(missing_ok=True)
scan_roots=[ROOT/'overlay',ROOT/'docs',ROOT/'repairs',ROOT/'README.md',ROOT/'THIRD_PARTY_NOTICES_DELTA.md']; files=[]
for x in scan_roots:
    if x.is_file(): files.append(x)
    elif x.exists(): files.extend(p for p in x.rglob('*') if p.is_file())
text='\n'.join(p.read_text(errors='ignore') for p in files)
ck('no_plaintext_secret',not re.search(r'(?i)(api[_-]?key|password|private[_-]?key)\s*[=:]\s*["\']?[A-Za-z0-9_\-]{16,}',text))
ck('no_legacy_brand',not re.search(r'PhoenixClaw|phoenixclaw|PHOENIXCLAW',text))
ck('no_direct_mutation','direct_operational_mutation() -> Result<(),DecisionError> { Err(DecisionError::DelegateRequired) }' in (ROOT/'overlay/crates/phxclaw-executive-decision-center/src/lib.rs').read_text())
passed=sum(x['pass'] for x in checks);failed=len(checks)-passed;rep={'suite':'v0.49 package','pass':passed,'fail':failed,'checks':checks};(ROOT/'reports/V049_PACKAGE_REPORT.json').write_text(json.dumps(rep,indent=2)+'\n');print(json.dumps({'pass':passed,'fail':failed},indent=2));sys.exit(1 if failed else 0)
