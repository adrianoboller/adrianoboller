#!/usr/bin/env python3
"""Gancho de fim do PhxJev: a resposta final traz a saida do script INTEIRA.

O selo tornou a edicao visivel, mas nao a impediu: na corrida do 321 o juiz
resumiu a saida em vez de cola-la. Este gancho roda quando o agente vai
encerrar o turno; se o turno produziu um `selo:` e a resposta final nao
contem cada linha do original, recusa o fim (codigo 2) dizendo o que falta.
Uma segunda tentativa passa (`stop_hook_active`): o gancho corrige, nao prende.
"""
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import phxjev  # noqa: E402

SELO = re.compile(r"selo: ([0-9a-f]{12})")


def blocos(r):
    c = (r.get("message") or {}).get("content")
    if isinstance(c, str):
        return [{"type": "text", "text": c}]
    return c if isinstance(c, list) else []


def texto_de_resultado(b):
    c = b.get("content")
    if isinstance(c, list):
        return "\n".join(x.get("text", "") for x in c if isinstance(x, dict))
    return c if isinstance(c, str) else ""


MARCA_DO_COMANDO = ("Use a skill `phxjev`", "/phxjev-")


def turno(linhas):
    """(selos gerados no turno, texto final do agente, turno veio de /phxjev-?)."""
    inicio = 0
    for i, r in enumerate(linhas):
        if r.get("type") == "user" and any(b.get("type") == "text" for b in blocos(r)):
            inicio = i
    pedido = "\n".join(b.get("text", "") for b in blocos(linhas[inicio])) if linhas else ""
    do_comando = any(m in pedido for m in MARCA_DO_COMANDO)
    selos, ultimo_resultado = [], inicio
    for i in range(inicio, len(linhas)):
        for b in blocos(linhas[i]):
            if b.get("type") == "tool_result":
                selos += SELO.findall(texto_de_resultado(b))
                ultimo_resultado = i
    final = "\n".join(
        b.get("text", "")
        for r in linhas[ultimo_resultado:]
        if r.get("type") == "assistant"
        for b in blocos(r)
        if b.get("type") == "text"
    )
    return list(dict.fromkeys(selos)), final, do_comando


def faltas(linhas, registro):
    selos, final, do_comando = turno(linhas)
    presentes = {l.strip() for l in final.splitlines()}
    out = []
    # O /phxjev-escolher do 175 respondeu sem chamar o script: sem selo, nao
    # havia o que conferir. Turno que veio de um comando PhxJev tem de ter um.
    if do_comando and not selos:
        out.append(("nenhum", ["o comando /phxjev- terminou sem passar pelo phxjev.py veredito"]))
    for selo in selos:
        try:
            original = phxjev.cmd_mostrar(selo, registro)
        except phxjev.Invalido:
            continue  # selo de outro registro: nao e deste repositorio julgar
        falta = [l for l in original.splitlines() if l.strip() and l.strip() not in presentes]
        if falta or f"selo: {selo}" not in final:
            out.append((selo, falta))
    return out


def main():
    entrada = json.load(sys.stdin)
    if entrada.get("stop_hook_active"):
        return 0
    caminho = entrada.get("transcript_path")
    if not caminho or not os.path.exists(caminho):
        return 0
    with open(caminho, encoding="utf-8") as f:
        linhas = [json.loads(l) for l in f if l.strip()]
    achado = faltas(linhas, phxjev.REGISTRO)
    if not achado:
        return 0
    msg = ["PhxJev: a resposta final nao traz a saida do phxjev.py sem edicao."]
    for selo, falta in achado:
        msg.append(f"selo {selo}: {len(falta)} linha(s) ausentes ou alteradas, ex.: {falta[:2]}")
    msg.append("Cole a saida inteira, sem encurtar (phxjev.py mostrar <selo> reimprime), "
               "e inclua a linha 'selo: ...'. O resumo pode vir DEPOIS dela.")
    print("\n".join(msg), file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main())
