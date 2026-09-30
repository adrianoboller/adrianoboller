#!/usr/bin/env python3
import json,shutil,subprocess
exe=shutil.which('hermes'); out={'available':bool(exe),'path':exe}
if exe:
 r=subprocess.run([exe,'--version'],text=True,capture_output=True,timeout=5); out.update({'returncode':r.returncode,'version':(r.stdout or r.stderr).strip()[:200]})
print(json.dumps(out,indent=2)); raise SystemExit(0 if exe else 2)
