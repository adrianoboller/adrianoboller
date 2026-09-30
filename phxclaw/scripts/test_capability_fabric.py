#!/usr/bin/env python3
from __future__ import annotations

import datetime as dt
import json
import os
import secrets
import shutil
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ART = ROOT / "plugins" / "builtin" / "artifacts"
REPORT = ROOT / "CAPABILITY_TEST_REPORT.json"
PROTOCOL = "phxclaw-process-v1"


def uuid7() -> str:
    ms = int(time.time() * 1000) & ((1 << 48) - 1)
    rand_a = secrets.randbits(12)
    rand_b = secrets.randbits(62)
    value = (ms << 80) | (0x7 << 76) | (rand_a << 64) | (0b10 << 62) | rand_b
    h = f"{value:032x}"
    return f"{h[:8]}-{h[8:12]}-{h[12:16]}-{h[16:20]}-{h[20:]}"


def envelope(kind: str, payload: dict | None = None) -> dict:
    return {
        "protocol": PROTOCOL,
        "message_uuid": uuid7(),
        "kind": kind,
        "emitted_at": dt.datetime.now(dt.timezone.utc).isoformat(),
        "payload": payload or {},
    }


def invoke(tool: Path, kind: str, payload: dict | None = None, env: dict | None = None, timeout: int = 30) -> dict:
    merged = os.environ.copy()
    if env:
        merged.update(env)
    proc = subprocess.run(
        [str(tool)],
        input=json.dumps(envelope(kind, payload), ensure_ascii=False) + "\n",
        text=True,
        capture_output=True,
        cwd=ROOT,
        env=merged,
        timeout=timeout,
    )
    if proc.returncode != 0:
        raise RuntimeError(f"{tool.name} exit {proc.returncode}: {proc.stderr[-1000:]}")
    line = proc.stdout.strip().splitlines()[-1]
    result = json.loads(line)
    if result.get("status") != "ok":
        raise RuntimeError(f"{tool.name}: {result.get('error')}")
    return result["payload"]


def check(name: str, fn, results: list[dict]):
    started = time.monotonic()
    try:
        detail = fn()
        results.append({"name": name, "status": "PASS", "elapsed_ms": round((time.monotonic()-started)*1000), "detail": detail})
        print(f"PASS {name}", flush=True)
    except Exception as exc:
        results.append({"name": name, "status": "FAIL", "elapsed_ms": round((time.monotonic()-started)*1000), "detail": str(exc)})
        print(f"FAIL {name}: {exc}", flush=True)


def skip(name: str, reason: str, results: list[dict]):
    results.append({"name": name, "status": "SKIP", "elapsed_ms": 0, "detail": reason})
    print(f"SKIP {name}: {reason}", flush=True)


def main() -> int:
    results: list[dict] = []
    docs = ROOT / "var" / "documents" / "selftest"
    media = ROOT / "var" / "media" / "selftest"
    for d in (docs, media):
        if d.exists():
            shutil.rmtree(d)
        d.mkdir(parents=True, exist_ok=True)

    office = ART / "office-document-tool"
    media_tool = ART / "media-intelligence-tool"
    system_tool = ART / "system-automation-tool"

    check("process.office.health", lambda: invoke(office, "health"), results)
    check("process.media.health", lambda: invoke(media_tool, "health"), results)

    fixtures = {
        "txt": {"path": "var/documents/selftest/sample.txt", "text": "PhxClaw TXT 2026"},
        "py": {"path": "var/documents/selftest/sample.py", "text": "print('PhxClaw PY 2026')\n"},
        "xml": {"path": "var/documents/selftest/sample.xml", "text": "<phoenix><version>0.5</version></phoenix>"},
        "json": {"path": "var/documents/selftest/sample.json", "structured": {"project": "PhxClaw", "version": "0.5"}},
        "csv": {"path": "var/documents/selftest/sample.csv", "structured": [{"name": "PhxClaw", "version": "0.5"}, {"name": "Claw", "version": "2026"}]},
        "xlsx": {"path": "var/documents/selftest/sample.xlsx", "structured": {"sheets": {"Status": [["capability", "state"], ["xlsx", "ok"], ["uuidv7", "ok"]]}}},
        "docx": {"path": "var/documents/selftest/sample.docx", "structured": {"paragraphs": ["PhxClaw DOCX", "Capability Fabric 0.5"], "tables": [[ ["capability", "state"], ["docx", "ok"] ]]}},
        "pdf": {"path": "var/documents/selftest/sample.pdf", "text": "PhxClaw PDF\nCapability Fabric 0.5"},
    }

    for fmt, payload in fixtures.items():
        def roundtrip(fmt=fmt, payload=payload):
            out = invoke(office, "execute", {"capability": "document.write", "payload": {**payload, "format": fmt}})
            p = ROOT / payload["path"]
            if not p.exists() or p.stat().st_size <= 0:
                raise RuntimeError(f"file missing/empty: {p}")
            read = invoke(office, "execute", {"capability": "document.read", "payload": {"path": payload["path"], "format": fmt}})
            return {"bytes": p.stat().st_size, "read_format": read.get("format"), "metadata": read.get("metadata", {})}
        check(f"document.{fmt}.read_write", roundtrip, results)

    def make_ocr_fixture():
        from PIL import Image, ImageDraw, ImageFont
        img = Image.new("RGB", (1400, 320), "white")
        draw = ImageDraw.Draw(img)
        font_path = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"
        font = ImageFont.truetype(font_path, 96) if Path(font_path).exists() else ImageFont.load_default()
        draw.text((60, 80), "PHOENIX CLAW 2026", fill="black", font=font)
        path = media / "ocr.png"
        img.save(path)
        out = invoke(media_tool, "execute", {"capability": "ocr.read", "payload": {"input": "var/media/selftest/ocr.png", "output_txt": "var/media/selftest/ocr.txt", "languages": ["eng"], "psm": 6}})
        text = out.get("text", "")
        if "PHOENIX" not in text.upper():
            raise RuntimeError(f"unexpected OCR text: {text!r}")
        txt = media / "ocr.txt"
        if not txt.exists():
            raise RuntimeError("OCR TXT not generated")
        return {"text": text.strip(), "txt_bytes": txt.stat().st_size}
    if shutil.which("tesseract"):
        check("ocr.image_to_txt", make_ocr_fixture, results)
    else:
        skip("ocr.image_to_txt", "tesseract not installed", results)

    def tts_test():
        out = invoke(media_tool, "execute", {"capability": "speech.tts", "payload": {"text": "PhxClaw capability test", "output_audio": "var/media/selftest/tts.wav", "language": "en-us"}})
        p = media / "tts.wav"
        if not p.exists() or p.stat().st_size <= 44:
            raise RuntimeError("TTS WAV not generated")
        return {"backend": out.get("backend"), "bytes": p.stat().st_size}
    if shutil.which("espeak") or shutil.which("espeak-ng"):
        check("speech.tts", tts_test, results)
    else:
        skip("speech.tts", "eSpeak/eSpeak-NG not installed", results)

    def command_test():
        out = invoke(system_tool, "execute", {"capability": "system.command.execute", "payload": {"shell": "direct", "program": (shutil.which("echo") or "/bin/echo"), "args": ["PHOENIX_COMMAND_OK"], "timeout_ms": 5000}}, env={"PHXCLAW_SYSTEM_AUTOMATION": "1"})
        if out.get("exit_code") != 0 or "PHOENIX_COMMAND_OK" not in out.get("stdout", ""):
            raise RuntimeError(str(out))
        return {"exit_code": out["exit_code"], "stdout": out["stdout"].strip()}
    check("system.command.execute.direct", command_test, results)

    # Negative gate: command execution must be rejected by default.
    def command_denied():
        proc = subprocess.run(
            [str(system_tool)],
            input=json.dumps(envelope("execute", {"capability": "system.command.execute", "payload": {"shell": "direct", "program": (shutil.which("echo") or "/bin/echo"), "args": ["NO"]}})) + "\n",
            text=True, capture_output=True, cwd=ROOT, timeout=30,
            env={k:v for k,v in os.environ.items() if k != "PHXCLAW_SYSTEM_AUTOMATION"},
        )
        obj = json.loads(proc.stdout.strip().splitlines()[-1])
        if obj.get("status") != "rejected" or (obj.get("error") or {}).get("code") != "permission_denied":
            raise RuntimeError(f"expected fail-closed, got {obj}")
        return {"status": obj["status"], "code": obj["error"]["code"]}
    check("system.command.fail_closed", command_denied, results)

    if shutil.which("cargo"):
        check("rust.cargo_check", lambda: {"stdout": subprocess.run(["cargo", "check", "--workspace"], cwd=ROOT, capture_output=True, text=True, timeout=600, check=True).stdout[-2000:]}, results)
    else:
        skip("rust.cargo_check", "cargo/rustc unavailable in current build environment", results)

    if shutil.which("psql"):
        skip("postgres.e2e", "psql exists but no test DSN was supplied; no external DB is touched implicitly", results)
    else:
        skip("postgres.e2e", "PostgreSQL client/server unavailable in current build environment", results)

    if shutil.which("ollama"):
        skip("ollama.e2e", "Ollama binary exists but no implicit model pull/server mutation is performed by self-test", results)
    else:
        skip("ollama.e2e", "Ollama unavailable in current build environment", results)

    if os.getenv("DISPLAY") or os.getenv("WAYLAND_DISPLAY"):
        skip("desktop.input_and_screen.e2e", "desktop session detected, but intrusive input/capture is intentionally not executed by unattended self-test", results)
    else:
        skip("desktop.input_and_screen.e2e", "headless build environment; requires real desktop permission/session", results)

    media_health = invoke(media_tool, "health")
    if media_health.get("stt_configured"):
        skip("speech.stt", "Whisper configured but self-test needs a known speech fixture/model to assert transcription", results)
    else:
        skip("speech.stt", "Whisper runtime/model not configured in current environment", results)

    summary = {
        "version": "0.13.0",
        "generated_at": dt.datetime.now(dt.timezone.utc).isoformat(),
        "pass": sum(r["status"] == "PASS" for r in results),
        "fail": sum(r["status"] == "FAIL" for r in results),
        "skip": sum(r["status"] == "SKIP" for r in results),
        "results": results,
    }
    REPORT.write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(summary, ensure_ascii=False, indent=2))
    return 1 if summary["fail"] else 0


if __name__ == "__main__":
    raise SystemExit(main())
