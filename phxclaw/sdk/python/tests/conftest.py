"""Sobe o `phxclaw` de verdade (`target/debug/phxclaw`) contra um Ollama de roteiro.

O modelo falso fala o fio do Ollama (`POST /api/chat`) e decide a resposta pelo que ja
esta na conversa, sem estado: sem resultado de ferramenta ainda, pede `write_file`;
com ele, chama `final_answer`. E o mesmo papel do `ScriptedLlm` dos testes Rust, so que
do lado de fora do processo -- o binario nao sabe que e teste.
"""

from __future__ import annotations

import json
import os
import socket
import subprocess
import threading
import time
import urllib.request
from collections.abc import Iterator
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any

import pytest

RAIZ = Path(__file__).resolve().parents[3]
TOKEN = "token-do-sdk-python-com-tamanho-suficiente"


def binario() -> Path:
    b = Path(os.environ.get("PHXCLAW_BIN", RAIZ / "target" / "debug" / "phxclaw"))
    if not b.is_file():
        pytest.skip(f"binario ausente: {b} (cargo build -p phxclaw)")
    return b


class _Ollama(BaseHTTPRequestHandler):
    pedidos: list[dict[str, Any]] = []

    def log_message(self, *_: Any) -> None:
        pass

    def do_POST(self) -> None:  # noqa: N802 -- nome do http.server
        n = int(self.headers.get("Content-Length", 0))
        corpo = json.loads(self.rfile.read(n))
        _Ollama.pedidos.append(corpo)
        ja_rodou = any(m.get("role") == "tool" for m in corpo.get("messages", []))
        if ja_rodou:
            chamada = {"name": "final_answer", "arguments": {"answer": "feito: nota.txt"}}
        else:
            chamada = {
                "name": "write_file",
                "arguments": {"path": "nota.txt", "content": "ola do sdk python"},
            }
        resp = {
            "model": corpo.get("model"),
            "message": {"role": "assistant", "content": "", "tool_calls": [{"function": chamada}]},
            "done": True,
            "prompt_eval_count": 11,
            "eval_count": 7,
        }
        dados = json.dumps(resp).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(dados)))
        self.end_headers()
        self.wfile.write(dados)


@pytest.fixture(scope="session")
def ollama_falso() -> Iterator[str]:
    srv = ThreadingHTTPServer(("127.0.0.1", 0), _Ollama)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    yield f"http://127.0.0.1:{srv.server_address[1]}"
    srv.shutdown()


def porta_livre() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return int(s.getsockname()[1])


def ambiente(**extra: str) -> dict[str, str]:
    # Ambiente minimo: nada de chave de provedor nem de busca herdada do processo do teste.
    env = {k: v for k, v in os.environ.items() if k in ("PATH", "HOME", "LANG")}
    env.update(NO_PROXY="127.0.0.1,localhost", PHXCLAW_CAPACIDADES="fs.read,fs.write,shell.exec")
    env.update(extra)
    return env


@pytest.fixture()
def servidor(tmp_path: Path, ollama_falso: str) -> Iterator[str]:
    porta = porta_livre()
    p = subprocess.Popen(
        [str(binario()), "servir", "--porta", str(porta), "--pasta", str(tmp_path / "agente")],
        env=ambiente(PHXCLAW_API_TOKEN=TOKEN, OLLAMA_HOST=ollama_falso),
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    url = f"http://127.0.0.1:{porta}"
    abrir = urllib.request.build_opener(urllib.request.ProxyHandler({})).open
    fim = time.monotonic() + 30
    while True:
        try:
            with abrir(url + "/health", timeout=1):
                break
        except OSError:
            if p.poll() is not None or time.monotonic() > fim:
                saida = p.stdout.read().decode() if p.stdout else ""
                p.kill()
                pytest.fail(f"servir nao subiu: {saida}")
            time.sleep(0.1)
    yield url
    p.terminate()
    p.wait(timeout=10)
