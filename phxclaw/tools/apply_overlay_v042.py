#!/usr/bin/env python3
import json, shutil, sys, tomllib
from pathlib import Path

def merge(dst, src):
    if isinstance(dst, dict) and isinstance(src, dict):
        out=dict(dst)
        for k,v in src.items(): out[k]=merge(out[k],v) if k in out else v
        return out
    return src

def main():
    if len(sys.argv)!=2: raise SystemExit('usage: apply_overlay.py <phxclaw-root>')
    root=Path(sys.argv[1]).resolve(); pkg=Path(__file__).resolve().parents[1]; ov=pkg/'overlay'
    cfg=root/'config/phxclaw.config.json'
    if not cfg.exists():
        old=root/'config/phoenixclaw.config.json'
        if old.exists(): raise SystemExit('v0.41 rename must be applied first: legacy phoenixclaw.config.json still present')
        raise SystemExit('missing canonical config/phxclaw.config.json')
    obj=json.loads(cfg.read_text(encoding='utf-8')); patch=json.loads((ov/'config/patches/v042-project-management.patch.json').read_text())
    for k,v in patch['set'].items(): obj[k]=merge(obj.get(k,{}),v)
    cat=obj.setdefault('skills',{}).setdefault('catalog',[])
    for x in patch['append_unique']['skills.catalog']:
        if x not in cat: cat.append(x)
    obj['product']['version']='0.42.0'
    obj['config_management']['canonical_path']='config/phxclaw.config.json'
    cfg.write_text(json.dumps(obj,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
    # copy overlay except patch file itself
    for p in ov.rglob('*'):
        if not p.is_file() or 'config/patches/' in p.as_posix(): continue
        rel=p.relative_to(ov); dst=root/rel; dst.parent.mkdir(parents=True,exist_ok=True)
        if dst.exists() and dst.read_bytes()!=p.read_bytes():
            # known managed files can be overwritten only for v042-owned paths; canonical config already merged
            if not (str(rel).startswith(('crates/phxclaw-project-management','plugins/project-management','schemas/skills','schemas/integrations')) or rel.name in ['project-management-skill-catalog.v042.json','project-management-hybrid-flow.v042.json','project-management-v042.schema.json','V042_CAPABILITIES_DELTA.json','0042_project_management_suite.sql','project-management.html','PROJECT_MANAGEMENT_UI_CATALOG_V042.md','PROJECT_MANAGEMENT_SUITE_V042.md']):
                raise SystemExit(f'conflict: {rel}')
        shutil.copy2(p,dst)
    cargo=root/'Cargo.toml'
    if cargo.exists():
        txt=cargo.read_text(encoding='utf-8')
        if '"crates/phxclaw-project-management"' not in txt:
            m='members = ['
            if m in txt: txt=txt.replace(m,m+'\n  "crates/phxclaw-project-management",',1)
            cargo.write_text(txt,encoding='utf-8')
            tomllib.loads(txt)
    print('PhxClaw v0.42 Project Management Suite applied')
if __name__=='__main__': main()
