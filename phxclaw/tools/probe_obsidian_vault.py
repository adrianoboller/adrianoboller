#!/usr/bin/env python3
import argparse,json
from pathlib import Path
p=argparse.ArgumentParser(); p.add_argument('vault',type=Path); a=p.parse_args(); v=a.vault.resolve(); out={'vault':str(v),'exists':v.is_dir(),'obsidian_config':(v/'.obsidian').is_dir(),'exchange_dir':str(v/'.phxclaw/inbox')}; print(json.dumps(out,indent=2)); raise SystemExit(0 if out['exists'] else 2)
