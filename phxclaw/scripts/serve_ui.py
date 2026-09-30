#!/usr/bin/env python3
from http.server import ThreadingHTTPServer, SimpleHTTPRequestHandler
from pathlib import Path
import argparse
import os

ROOT = Path(__file__).resolve().parents[1] / "apps" / "phxclaw-ui"

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Serve the PhxClaw UI preview")
    parser.add_argument("--port", type=int, default=8080)
    args = parser.parse_args()
    os.chdir(ROOT)
    server = ThreadingHTTPServer(("127.0.0.1", args.port), SimpleHTTPRequestHandler)
    print(f"PhxClaw UI: http://127.0.0.1:{args.port}/")
    server.serve_forever()
