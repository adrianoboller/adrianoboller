#!/usr/bin/env python3
"""Reescreve, no MANUAL.md, a tabela de comandos e a de scripts.

O manual listava SEIS comandos quando havia 33, e nenhum dos quatro ultimos.
E o mesmo defeito de sempre neste projeto, agora no documento que o cliente
mais le: lista escrita a mao envelhece calada.

Aqui as duas tabelas ficam entre marcadores e sao regeradas do repositorio --
a de comandos sai do `description` de cada `commands/*.md`, a de scripts sai da
primeira linha da docstring de cada script. O resto do manual continua escrito
por gente, que e onde ele tem valor.

Uso: python3 docs/dossie/atualizar-manual.py [--conferir]
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
MANUAL = RAIZ / "MANUAL.md"
INI_C, FIM_C = "<!-- comandos: gerado -->", "<!-- fim dos comandos -->"
INI_S, FIM_S = "<!-- scripts: gerado -->", "<!-- fim dos scripts -->"

# a ordem de uso, a mesma da folha de comandos: quem le quer saber por onde comecar
try:
    sys.path.insert(0, str(RAIZ / "docs/dossie"))
    from importlib import import_module
    _gc = import_module("gerar-comandos".replace("-", "_")) if False else None
except Exception:  # noqa: BLE001
    _gc = None
ORDEM = ["questionario", "pergunta", "progresso", "comandos", "artefato", "pdf", "dependencias",
         "preflight", "converter", "interface", "estilo-telas", "golden", "testes-da-matriz",
         "constraints", "evidencia", "efeito", "grafo", "procedencia", "replay", "gemeo",
         "pmo", "equipe", "contrato", "telemetria", "identidade", "log", "ambiente", "help-wl",
         "rag", "exportar", "zelador", "licenca", "laudo-tokens"]


# o que cada destino e, dito uma vez aqui e nao a mao no manual; o golden vem do medidor
DESTINO = {"clientes-php-mysql": "PHP + MySQL → Rust (`tiny_http` + `mysql`) + React 19",
           "estoque-wx": "WINDEV 2025 (só PDFs) → Rust + Axum + PostgreSQL 16 + React 19"}


def descricao(arq: Path) -> str:
    texto = arq.read_text(encoding="utf-8")
    m = re.search(r'^description:\s*"?(.*?)"?\s*$', texto, re.M)
    return (m.group(1) if m else "").strip()


def primeira_linha_da_docstring(arq: Path) -> str:
    texto = arq.read_text(encoding="utf-8")
    m = re.search(r'^"""(.+?)$', texto, re.M)
    return (m.group(1) if m else "").strip().rstrip(".")


def tabela_de_comandos() -> str:
    arquivos = {p.stem: p for p in (RAIZ / "commands").glob("*.md")}
    faltando = sorted(set(arquivos) - set(ORDEM))
    if faltando:
        # comando novo sem lugar na ordem PARA o gerador, de proposito: senao ele
        # sumiria do manual e ninguem veria
        raise SystemExit(f"comando sem lugar na ORDEM: {', '.join(faltando)}")
    linhas = ["| Comando | O que faz |", "| --- | --- |"]
    for nome in ORDEM:
        if nome in arquivos:
            linhas.append(f"| `/wx-claude-code:{nome}` | {descricao(arquivos[nome])} |")
    return "\n".join(linhas)


def tabela_de_scripts() -> str:
    pasta = RAIZ / "skills/conversao-wx/scripts"
    linhas = ["| Script | Faz |", "| --- | --- |"]
    for p in sorted(pasta.glob("*.py")):
        if p.name in {"registro.py"}:
            continue
        linhas.append(f"| `{p.name}` | {primeira_linha_da_docstring(p)} |")
    return "\n".join(linhas)


def trocar(texto: str, ini: str, fim: str, novo: str) -> str:
    if ini not in texto or fim not in texto:
        raise SystemExit(f"MANUAL.md sem os marcadores {ini} … {fim}")
    a, b = texto.index(ini) + len(ini), texto.index(fim)
    return texto[:a] + "\n" + novo + "\n" + texto[b:]


def main() -> int:
    conferir = "--conferir" in sys.argv
    atual = MANUAL.read_text(encoding="utf-8")
    novo = trocar(atual, INI_C, FIM_C, tabela_de_comandos())
    novo = trocar(novo, INI_S, FIM_S, tabela_de_scripts())
    # o capitulo 1 dizia «5 comandos, 3 skills, 27 testes» quatro meses depois
    # de deixar de ser verdade: os numeros do manual tambem saem do medidor
    import json
    n = json.loads((RAIZ / "docs/dossie/numeros.json").read_text(encoding="utf-8"))
    novo = trocar(novo, "<!-- numeros: gerado -->", "<!-- fim dos numeros -->",
                  f"{n['comandos']} comandos, {n['skills']} skills, {n['agentes']} agentes").replace(
        "<!-- numeros: gerado -->\n", "<!-- numeros: gerado -->").replace("\n<!-- fim dos numeros -->", "<!-- fim dos numeros -->")
    # os videos e os destinos provados tambem saem do medidor: duracao, cenas e
    # golden sao numeros que envelhecem calados quando digitados
    videos = [("wx-claude-code-video-de-uso.mp4", "o uso, da instalação ao Rust", n["video_duracao"], n["video_cenas"]),
              ("wx-claude-code-video-php.mp4", "de PHP para Rust, sem nada de WINDEV", n["video_php_duracao"], n["video_php_cenas"]),
              ("wx-claude-code-video-bateria.mp4", "a bateria de testes rodando", n["video_bateria_duracao"], n["video_bateria_cenas"]),
              ("wx-claude-code-video-primeiro.mp4", "o primeiro projeto: instalação, licença e um CRUD PHP + MySQL → Rust + MySQL + React", n["video_primeiro_duracao"], n["video_primeiro_cenas"]),
              ("wx-claude-code-video-windev.mp4", "os PDFs do WINDEV → Rust + Axum + PostgreSQL + React", n["video_windev_duracao"], n["video_windev_cenas"])]
    tabela_videos = "\n".join(["| Vídeo (`docs/video/`) | O que mostra | Duração | Cenas |", "| --- | --- | --- | --- |"]
                               + [f"| `{a}` | {b} | {c} | {d} |" for a, b, c, d in videos])
    novo = trocar(novo, "<!-- videos: gerado -->", "<!-- fim dos videos -->", tabela_videos)
    destinos = "\n".join(["| Exemplo | Destino | Golden master |", "| --- | --- | --- |"]
                         + [f"| `exemplos/{d['exemplo']}/destino/` | {DESTINO.get(d['exemplo'], 'ver o LEIA-ME do exemplo')} | {d['golden']} |" for d in n.get("destinos", [])])
    novo = trocar(novo, "<!-- destinos: gerado -->", "<!-- fim dos destinos -->", destinos)
    if conferir:
        if novo != atual:
            print("MANUAL.md desatualizado: rode docs/dossie/atualizar-manual.py", file=sys.stderr)
            return 1
        print("MANUAL.md em dia")
        return 0
    MANUAL.write_text(novo, encoding="utf-8")
    n = len(re.findall(r"^\| `/wx", tabela_de_comandos(), re.M))
    s = len(re.findall(r"^\| `", tabela_de_scripts(), re.M)) - 1
    print(f"ok {MANUAL} ({n} comandos, {s} scripts)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
