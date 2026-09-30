#!/usr/bin/env python3
from pathlib import Path
import json, re, sys
try: import jsonschema
except Exception: jsonschema=None
root=Path(__file__).resolve().parents[1]; over=root/'overlay' if (root/'overlay').is_dir() else root
checks=[]
def ck(n,c,d=''): checks.append((n,bool(c),d))
cfgp=over/'config/phxclaw.config.json'; schp=over/'schemas/phxclaw-config-v040.schema.json'; html=over/'ui/config.html'; rust=over/'crates/phxclaw-config-runtime/src/lib.rs'; fac=over/'crates/phxclaw-software-factory/src/lib.rs'; sql=over/'migrations/0040_unified_configuration_and_factory.sql'
cfg=json.loads(cfgp.read_text()); sch=json.loads(schp.read_text())
ck('config_version',cfg.get('schema_version')=='0.40.0'); ck('canonical_path',cfg['config_management']['canonical_path']=='config/phxclaw.config.json'); ck('legacy_not_authoritative',cfg['compatibility']['legacy_policy_files_authoritative'] is False); ck('plaintext_forbidden',cfg['security']['secrets_plaintext_forbidden'] is True); ck('deny_default',cfg['security']['deny_by_default'] is True); ck('software_factory',len(cfg['software_factory']['stages'])==12); ck('schema_validate', jsonschema is not None)
if jsonschema:
 try: jsonschema.Draft202012Validator(sch,format_checker=jsonschema.FormatChecker()).validate(cfg); ck('schema_instance',True)
 except Exception as e: ck('schema_instance',False,str(e))
t=html.read_text(); ck('config_html_api',"const API='/v1/config'" in t); ck('config_html_revision','if-match' in t); ck('config_html_import_export','Exportar JSON' in t and 'Importar JSON' in t); ck('config_html_secret_guard','segredo em claro proibido' in t); ck('config_html_raw','JSON bruto' in t)
r=rust.read_text(); ck('rust_atomic','fs::rename' in r and 'sync_all' in r); ck('rust_revision_conflict','RevisionConflict' in r); ck('rust_history','history_dir' in r); ck('rust_secret_scan','scan_secrets' in r and 'plaintext secret-like value forbidden' in r); ck('rust_uuidv7','get_version_num() != 7' in r)
f=fac.read_text(); ck('factory_checkpoint','checkpoint_uuid' in f and 'CheckpointRequired' in f); ck('factory_approval','ApprovalRequired' in f)
s=sql.read_text(); ck('sql_force_rls',s.count('FORCE ROW LEVEL SECURITY')>=2); ck('sql_rls_exact',"NULLIF(current_setting('phoenix.tenant_uuid', true), '')::uuid" in s); ck('sql_append_only','configuration revision history is append-only' in s)
cap=json.loads((over/'capabilities/V040_CAPABILITIES_DELTA.json').read_text()); ck('cap_total',cap['projected_total']==802); ck('cap_new',cap['new_capabilities']==52)
for n,c,d in checks: print(('PASS' if c else 'FAIL'),n,d)
print(f'TOTAL {sum(c for _,c,_ in checks)} PASS / {sum(not c for _,c,_ in checks)} FAIL')
sys.exit(1 if any(not c for _,c,_ in checks) else 0)
