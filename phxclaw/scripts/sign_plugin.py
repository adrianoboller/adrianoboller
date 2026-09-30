#!/usr/bin/env python3
"""Sign a PhxClaw plugin manifest with an externally supplied Ed25519 private key.

The private key is never stored by this script. Provide a raw 32-byte Ed25519 private
seed through PHXCLAW_ED25519_PRIVATE_KEY_B64 and a signer id already present in
the trust store.
"""
from __future__ import annotations
import base64
import hashlib
import json
import os
import sys
from pathlib import Path

try:
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
except Exception as exc:  # pragma: no cover
    raise SystemExit(f"cryptography is required for signing: {exc}")

ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    if len(sys.argv) != 2:
        raise SystemExit("usage: scripts/sign_plugin.py path/to/plugin.plugin.json")
    path = Path(sys.argv[1]).resolve()
    manifest = json.loads(path.read_text(encoding="utf-8"))
    artifact = (ROOT / manifest["integrity"]["artifact"]).resolve()
    if ROOT.resolve() not in artifact.parents and artifact != ROOT.resolve():
        raise SystemExit("artifact escapes package root")
    digest = hashlib.sha256(artifact.read_bytes()).hexdigest()
    manifest["integrity"]["digest"] = digest

    raw = base64.b64decode(os.environ["PHXCLAW_ED25519_PRIVATE_KEY_B64"], validate=True)
    if len(raw) != 32:
        raise SystemExit("private key seed must decode to exactly 32 bytes")
    private_key = Ed25519PrivateKey.from_private_bytes(raw)
    message = (
        "PHXCLAW-PLUGIN-V1\n"
        f"uuid={manifest['uuid']}\n"
        f"name={manifest['name']}\n"
        f"version={manifest['version']}\n"
        f"sha256={digest}\n"
    ).encode("utf-8")
    manifest["integrity"]["signature"] = base64.b64encode(private_key.sign(message)).decode("ascii")
    path.write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"signed {path.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
