#!/usr/bin/env python3
"""Recebe o aviso de instalacao e manda o e-mail para quem vende.

E a outra ponta do `aviso` que o `emitir.py --aviso URL` poe dentro do serial:
quando o cliente instala, o `licenca.py` faz UM POST para ca, com o que o
LICENCA.md diz que vai -- id do serial, empresa, impressao da maquina, versao,
data e o hash dos termos aceitos. Nada do projeto do cliente chega aqui.

O que este receptor faz com isso, nesta ordem:

  1. Confere que o id existe no SEU livro de emissoes. Aviso de serial que voce
     nunca emitiu e lixo ou golpe, e nao vira e-mail.
  2. Grava no livro de instalacoes (`~/.wx-serial/instalacoes.jsonl`, 0600).
  3. Compara a impressao da maquina com as anteriores do mesmo serial: se e
     uma maquina NOVA para um serial que ja foi instalado, o assunto do e-mail
     diz «POSSIVEL RECOMPARTILHAMENTO». E o unico jeito de perceber isso.
  4. Manda o e-mail por SMTP (stdlib), com as credenciais que vem do ambiente:

       WX_SMTP_HOST  WX_SMTP_PORTA (587)  WX_SMTP_USUARIO  WX_SMTP_SENHA
       WX_AVISO_PARA (o seu e-mail)  WX_AVISO_DE (remetente; padrao = usuario)

     Sem WX_SMTP_HOST configurado, ele grava e imprime no terminal -- e diz
     que nao mandou e-mail, em vez de fingir.

Zero dependencia: http.server e smtplib. Ponha atras de um HTTPS seu (nginx,
Caddy, tunel) -- ele mesmo so fala HTTP.

Uso:
  receber.py [--porta 8765] [--endereco 0.0.0.0]
"""
from __future__ import annotations

import argparse
import json
import os
import smtplib
import sys
from datetime import datetime, timezone
from email.message import EmailMessage
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

CASA = Path(os.environ.get("WX_SERIAL_DIR") or (Path.home() / ".wx-serial"))
EMISSOES = CASA / "emissoes.jsonl"
INSTALACOES = CASA / "instalacoes.jsonl"


def ler_jsonl(p: Path) -> list[dict]:
    if not p.is_file():
        return []
    saida = []
    for linha in p.read_text(encoding="utf-8").splitlines():
        try:
            saida.append(json.loads(linha))
        except ValueError:
            continue
    return saida


def gravar(p: Path, registro: dict) -> None:
    p.parent.mkdir(parents=True, exist_ok=True)
    fd = os.open(p, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o600)
    with os.fdopen(fd, "a", encoding="utf-8") as f:
        f.write(json.dumps(registro, ensure_ascii=False) + "\n")


def classificar(aviso: dict) -> tuple[str, dict | None]:
    """(classe, emissao). Classes: desconhecido, primeira, repetida, maquina-nova."""
    emissao = next((e for e in ler_jsonl(EMISSOES) if e.get("id") == aviso.get("id")), None)
    if emissao is None:
        return "desconhecido", None
    anteriores = [i for i in ler_jsonl(INSTALACOES) if i.get("id") == aviso.get("id")]
    if not anteriores:
        return "primeira", emissao
    maquinas = {i.get("maquina") for i in anteriores}
    return ("repetida" if aviso.get("maquina") in maquinas else "maquina-nova"), emissao


def montar_email(aviso: dict, classe: str, emissao: dict) -> EmailMessage:
    m = EmailMessage()
    alerta = classe == "maquina-nova"
    m["Subject"] = (("POSSÍVEL RECOMPARTILHAMENTO — " if alerta else "Instalação — ")
                    + f"{aviso.get('cliente')} · serial {aviso.get('id')}")
    m["From"] = os.environ.get("WX_AVISO_DE") or os.environ.get("WX_SMTP_USUARIO", "wx-serial@localhost")
    m["To"] = os.environ.get("WX_AVISO_PARA", "")
    linhas = [f"Serial {aviso.get('id')} de {aviso.get('cliente')} foi instalado.", "",
              f"Quando:            {aviso.get('em')}",
              f"Máquina:           {aviso.get('maquina')}",
              f"Versão do plugin:  {aviso.get('versao_do_plugin')}",
              f"Validade:          {aviso.get('validade')}",
              f"Termos aceitos:    sha256 {str(aviso.get('termos_sha256', ''))[:16]}…",
              f"Emitido em:        {emissao.get('emitido_em')} para {emissao.get('email') or '(sem e-mail)'}", ""]
    if alerta:
        n = len([i for i in ler_jsonl(INSTALACOES) if i.get("id") == aviso.get("id")])
        linhas += [f"ATENÇÃO: este serial já tinha {n} instalação(ões) em outra máquina.",
                   "Pelo item 3 dos termos, cada empresa precisa do próprio serial."]
    elif classe == "repetida":
        linhas.append("Mesma máquina de antes: reinstalação.")
    m.set_content("\n".join(linhas))
    return m


def enviar_email(m: EmailMessage) -> dict:
    host = os.environ.get("WX_SMTP_HOST")
    if not host or not m["To"]:
        return {"enviado": False, "motivo": "WX_SMTP_HOST ou WX_AVISO_PARA não configurados"}
    porta = int(os.environ.get("WX_SMTP_PORTA", "587"))
    usuario, senha = os.environ.get("WX_SMTP_USUARIO", ""), os.environ.get("WX_SMTP_SENHA", "")
    try:
        with smtplib.SMTP(host, porta, timeout=15) as s:
            s.ehlo()
            if porta != 25:
                s.starttls()
            if usuario:
                s.login(usuario, senha)
            s.send_message(m)
        return {"enviado": True}
    except Exception as e:  # noqa: BLE001
        return {"enviado": False, "motivo": str(e)[:200]}


# ponto de troca para o teste: o teste substitui por uma funcao que so anota,
# porque teste que manda e-mail de verdade nao e teste, e spam
ENVIAR = enviar_email


def processar(aviso: dict) -> dict:
    classe, emissao = classificar(aviso)
    if classe == "desconhecido":
        return {"aceito": False, "classe": classe}
    registro = {**aviso, "recebido_em": datetime.now(timezone.utc).isoformat(timespec="seconds"),
                "classe": classe}
    gravar(INSTALACOES, registro)
    m = montar_email(aviso, classe, emissao)
    resultado = ENVIAR(m)
    return {"aceito": True, "classe": classe, "email": resultado, "assunto": m["Subject"]}


class Receptor(BaseHTTPRequestHandler):
    def do_POST(self):  # noqa: N802
        n = int(self.headers.get("Content-Length") or 0)
        if n <= 0 or n > 8192:
            self.send_response(400); self.end_headers(); return
        try:
            aviso = json.loads(self.rfile.read(n))
        except ValueError:
            self.send_response(400); self.end_headers(); return
        if aviso.get("evento") != "instalacao" or not aviso.get("id"):
            self.send_response(400); self.end_headers(); return
        r = processar(aviso)
        # desconhecido tambem responde 204: nao se ensina a quem tenta adivinhar
        # ids quais existem
        self.send_response(204); self.end_headers()
        print(f"{aviso.get('id')} {r['classe']}"
              + (f" · e-mail: {'enviado' if r.get('email', {}).get('enviado') else r.get('email', {}).get('motivo')}"
                 if r["aceito"] else " · ignorado"), flush=True)

    def log_message(self, *_):  # silencia o log padrao; o print acima e o log
        pass


def main() -> int:
    p = argparse.ArgumentParser(description="recebe avisos de instalacao e manda o e-mail")
    p.add_argument("--porta", type=int, default=8765)
    p.add_argument("--endereco", default="0.0.0.0")
    a = p.parse_args()
    if not os.environ.get("WX_SMTP_HOST"):
        print("AVISO: WX_SMTP_HOST não configurado — vou gravar as instalações e imprimir aqui, "
              "mas NÃO mandar e-mail.", file=sys.stderr)
    print(f"recebendo em http://{a.endereco}:{a.porta}/ · livro em {INSTALACOES}", flush=True)
    HTTPServer((a.endereco, a.porta), Receptor).serve_forever()
    return 0


if __name__ == "__main__":
    sys.exit(main())
