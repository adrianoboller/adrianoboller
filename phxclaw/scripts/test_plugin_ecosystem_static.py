#!/usr/bin/env python3
from __future__ import annotations
import json, re, tomllib, uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
checks=[]

def check(name, cond, detail=''):
    checks.append((name, bool(cond), detail))

def read(path):
    return (ROOT/path).read_text(encoding='utf-8')

workspace=tomllib.loads(read('Cargo.toml'))
members=set(workspace['workspace']['members'])
for member in [
    'crates/phxclaw-plugin-sdk',
    'crates/phxclaw-community-registry',
    'crates/phxclaw-extension-host',
    'crates/phxclaw-octopus-bridge',
    'apps/phxclaw-octopus-plugin',
]:
    check(f'workspace:{member}', member in members)

for file in [
    'schemas/community-registry.schema.json',
    'schemas/extension-invocation.schema.json',
    'schemas/plugin-package.schema.json',
    'schemas/extension-point-catalog.schema.json',
    'schemas/release-policy.schema.json',
    'migrations/0013_community_plugins.sql',
    'config/community/registry.json',
    'config/extension-points.json',
    'config/release-policy.json',
    'community/examples/octopus-bridge/phxclaw.plugin.template.json',
    'community/examples/olmocr/phxclaw.plugin.template.json',
    'community/examples/olmocr/phxclaw-olmocr-plugin',
    'community/examples/olmocr/README.md',
    'community/examples/olmocr/SBOM.spdx.json',
    'docs/OLMOCR_PLUGIN_V010.md',
    'docs/COMMUNITY_PLUGIN_ARCHITECTURE_V09.md',
    'docs/OCTOPUS_REUSE_AUDIT_V09.md',
    'docs/VERIFICATION_LEVELS_V09.md',
    'community/CONTRIBUTING_PLUGINS.md',
    'CONTRIBUTING.md',
    'GOVERNANCE.md',
    'SECURITY.md',
    'LICENSE',
]:
    check(f'file:{file}', (ROOT/file).is_file())


ext_points=json.loads(read('config/extension-points.json'))
point_names=[p['name'] for p in ext_points.get('extension_points',[])]
check('extension-points:unique', len(point_names)==len(set(point_names)), point_names)
for point in ['capability.provider','model.provider','compiler.frontend','channel.provider','device.node','knowledge.source','skill.provider']:
    check(f'extension-point:{point}', point in point_names)
check('extension-points:capability-stable', next((p.get('stability') for p in ext_points['extension_points'] if p['name']=='capability.provider'),None)=='stable')

release_policy=json.loads(read('config/release-policy.json'))
check('release:no-mocks', release_policy['release']['allow_mock_providers'] is False)
check('release:no-test-doubles', release_policy['release']['allow_test_doubles'] is False)
check('release:no-skipped-gates', release_policy['release']['allow_skipped_required_gates'] is False)
check('release:e2e-required', 'mission.end_to_end' in release_policy['release']['required_gates'])

catalog=json.loads(read('config/capability-catalog.json'))
capnames={c['name'] for c in catalog['capabilities']}
for cap in [
    'plugin.extension.invoke', 'plugin.extension.route.pin',
    'plugin.community.registry.read', 'plugin.community.package.verify',
    'octopus.repo.map', 'octopus.code.analyze', 'octopus.shadow.check',
    'ocr.olmocr.health', 'ocr.olmocr.extract', 'ocr.olmocr.batch', 'knowledge.olmocr.read'
]:
    check(f'capability:{cap}', cap in capnames)

bridge=read('crates/phxclaw-octopus-bridge/src/lib.rs')
check('octopus:no-shell', 'Command::new(&executable)' in bridge and 'sh -c' not in bridge and 'cmd /c' not in bridge)
check('octopus:allowlist', 'supported_capabilities()' in bridge and 'UnsupportedCapability' in bridge)
check('octopus:path-confinement', 'PathOutsideWorkspace' in bridge and 'starts_with(&root)' in bridge)

host=read('crates/phxclaw-extension-host/src/lib.rs')
check('extension:ambiguity-fail-closed', 'Ambiguous' in host and 'InvalidPin' in host)
check('extension:evidence', 'EvidenceLedger' in host and 'plugin.capability.invoke' in host)
check('extension:event-bus', 'LiveEventHub' in host and 'plugin.capability' in host)

manifest=json.loads(read('community/examples/octopus-bridge/phxclaw.plugin.template.json'))
check('template:uuidv7', uuid.UUID(manifest['uuid']).version == 7)
check('template:not-loadable', '<generated-by-phxclaw-plugin-pack>' in manifest['integrity']['digest'])
check('template:network-deny', manifest['sandbox']['network'] == 'deny')


olmocr_manifest=json.loads(read('community/examples/olmocr/phxclaw.plugin.template.json'))
check('olmocr-template:uuidv7', uuid.UUID(olmocr_manifest['uuid']).version == 7)
check('olmocr-template:not-loadable', '<generated-by-phxclaw-plugin-pack>' in olmocr_manifest['integrity']['digest'])
check('olmocr-template:network-deny', olmocr_manifest['sandbox']['network'] == 'deny')
check('olmocr-template:no-generic-collision', 'ocr.read' not in olmocr_manifest['capabilities'])
olmocr_runner=read('community/examples/olmocr/phxclaw-olmocr-plugin')
check('olmocr:subprocess-no-shell', 'subprocess.run(' in olmocr_runner and 'shell=True' not in olmocr_runner)
check('olmocr:path-confinement', 'path escapes allowed root' in olmocr_runner and 'safe_under' in olmocr_runner)
check('olmocr:remote-allowlist', 'PHXCLAW_OLMOCR_SERVER_ALLOWLIST' in olmocr_runner and 'remote server origin not allowlisted' in olmocr_runner)
check('olmocr:no-cli-api-key', 'authenticated remote olmOCR is blocked' in olmocr_runner)

registry=json.loads(read('config/community/registry.json'))
check('registry:no-fake-release', registry.get('releases') == [])
check('registry:publisher', any(p.get('id') == 'wxsolucoes' for p in registry.get('publishers', [])))

failed=[x for x in checks if not x[1]]
report={'pass':len(checks)-len(failed),'fail':len(failed),'checks':[{'name':n,'ok':ok,'detail':d} for n,ok,d in checks]}
(ROOT/'PLUGIN_ECOSYSTEM_TEST_REPORT.json').write_text(json.dumps(report,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(f"PLUGIN ECOSYSTEM STATIC TEST: PASS {report['pass']} FAIL {report['fail']}")
for n,ok,d in failed:
    print('FAIL',n,d)
raise SystemExit(1 if failed else 0)
