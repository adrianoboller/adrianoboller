#!/usr/bin/env python3
"""Monta os entregaveis em zip, um por publico, com a ficha de hashes.

Quatro pacotes, porque sao quatro maos diferentes:

  cliente/    wx-claude-code-<v>.zip   o plugin que o cliente instala
  vendedor/   wx-serial-<v>.zip        emissor + receptor; NUNCA vai ao cliente
  binarios/   wx-modelos-<v>.zip       Linux e Windows, compilados agora
  documentos/ documentos-<v>.zip       PDFs, termos, seguranca, videos

O que NAO entra no pacote do cliente, de proposito e conferido no fim:
`ferramentas/wx-serial/` (a ferramenta de quem vende), qualquer
`chave-privada.json`, livro de emissoes, `target/`, `dist/`, `.git`. O script
abre o zip depois de escrever e PROCURA esses nomes: achar qualquer um e falha,
nao aviso. E o mesmo principio do publicar.py: ler a lista de arquivos nao
prova nada; abrir o pacote prova.

Antes de empacotar, roda a bateria. Teste vermelho nao vira entrega.

Uso: python3 empacotar-entregaveis.py [--saida entregas] [--sem-teste] [--sem-binarios]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import zipfile
from datetime import date
from pathlib import Path

RAIZ = Path(__file__).resolve().parent
PROIBIDO_NO_CLIENTE = ("wx-serial/", "chave-privada", "emissoes.jsonl", "instalacoes.jsonl",
                       "/target/", "/dist/", "/.git/", ".env")
IGNORAR_SEMPRE = {".git", "target", "dist", "__pycache__", "node_modules", ".pytest_cache",
                  "entregas", ".claude"}


def versao() -> str:
    return json.loads((RAIZ / ".claude-plugin/plugin.json").read_text(encoding="utf-8"))["version"]


def sha256(p: Path) -> str:
    h = hashlib.sha256()
    with p.open("rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def arquivos_do_plugin():
    for p in sorted(RAIZ.rglob("*")):
        if not p.is_file():
            continue
        rel = p.relative_to(RAIZ)
        if any(parte in IGNORAR_SEMPRE for parte in rel.parts):
            continue
        if rel.parts[0] == "ferramentas" and len(rel.parts) > 1 and rel.parts[1] == "wx-serial":
            continue
        if rel.name in {"chave-privada.json", "emissoes.jsonl", "instalacoes.jsonl"}:
            continue
        if rel.name == "empacotar-entregaveis.py":
            continue
        # videos e prints sao prova para a VENDA, nao para o uso: vao no pacote
        # de documentos. No do cliente eram 55 MB de 91 -- medido no zip
        if rel.parts[:2] in {("docs", "video"), ("docs", "prints")}:
            continue
        # material de VENDA (apresentacao, fluxo explicado, pagina de investidor) tambem
        # vai no pacote de documentos: no do cliente fez o zip crescer de 13 para 21 MiB
        if rel.parts[:2] == ("docs", "investidor") or rel.name in {
                "apresentacao-wx-claude-code.pptx", "fluxo-explicado.html", "fluxo-explicado.pdf"}:
            continue
        yield p, rel


def conferir_zip(zip_: Path, proibidos=PROIBIDO_NO_CLIENTE) -> list[str]:
    """Abre o zip e procura o que nao pode estar la."""
    with zipfile.ZipFile(zip_) as z:
        nomes = z.namelist()
    return sorted({n for n in nomes for pr in proibidos if pr in ("/" + n) or pr in n})


CORPUS = Path("skills/conversao-wx/resources/Help_WL_12k_Json.zip")


def pacote_cliente(v: str, saida: Path, separar_corpus: bool = False) -> Path:
    """Com `separar_corpus`, o corpus (25 MiB) sai em arquivo proprio: o
    instalador ja sabe recebe-lo com `--corpus`, e cada parte fica abaixo do
    limite de 30 MiB dos canais por onde isto costuma viajar."""
    sufixo = "-sem-corpus" if separar_corpus else ""
    alvo = saida / "cliente" / f"wx-claude-code-{v}{sufixo}.zip"
    alvo.parent.mkdir(parents=True, exist_ok=True)
    n = 0
    with zipfile.ZipFile(alvo, "w", zipfile.ZIP_DEFLATED) as z:
        for p, rel in arquivos_do_plugin():
            if separar_corpus and rel == CORPUS:
                continue
            z.write(p, f"wx-claude-code/{rel.as_posix()}")
            n += 1
    # Achado instalando o zip do cliente: o instalar.sh procura
    # `.claude-plugin/marketplace.json` na pasta PAI para fazer o `claude plugin
    # install`, e o zip nao levava -- caia no `--plugin-dir`, que so vale por
    # sessao. Vai ao lado, com a versao sincronizada com a do plugin.
    mk = RAIZ.parent / ".claude-plugin/marketplace.json"
    with zipfile.ZipFile(alvo, "a", zipfile.ZIP_DEFLATED) as z:
        d = json.loads(mk.read_text(encoding="utf-8"))
        for pl in d.get("plugins", []):
            if pl.get("name") == "wx-claude-code":
                pl["version"] = v
        z.writestr(".claude-plugin/marketplace.json", json.dumps(d, ensure_ascii=False, indent=2) + "\n")
    if separar_corpus:
        import shutil
        corpus_alvo = saida / "cliente" / CORPUS.name
        shutil.copy2(RAIZ / CORPUS, corpus_alvo)
        print(f"corpus     {corpus_alvo.name}  {corpus_alvo.stat().st_size / 1048576:.1f} MiB "
              f"(instale com: ./instalar.sh --corpus {CORPUS.name})")
    achados = conferir_zip(alvo)
    if achados:
        alvo.unlink()
        raise SystemExit(f"o pacote do CLIENTE levaria o que nao pode: {achados}")
    print(f"cliente    {alvo.name}  {n} arquivos")
    return alvo


def pacote_vendedor(v: str, saida: Path) -> Path:
    alvo = saida / "vendedor" / f"wx-serial-{v}.zip"
    alvo.parent.mkdir(parents=True, exist_ok=True)
    base = RAIZ / "ferramentas/wx-serial"
    scripts = RAIZ / "skills/conversao-wx/scripts"
    with zipfile.ZipFile(alvo, "w", zipfile.ZIP_DEFLATED) as z:
        for nome in ("emitir.py", "receber.py", "LEIA-ME.md"):
            z.write(base / nome, f"wx-serial/{nome}")
        for nome in ("licenca.py", "registro.py"):
            z.write(scripts / nome, f"wx-serial/{nome}")
        z.write(RAIZ / "LICENCA.md", "wx-serial/LICENCA.md")
        z.write(RAIZ / "docs/SEGURANCA.md", "wx-serial/SEGURANCA.md")
    achados = conferir_zip(alvo, ("chave-privada", "emissoes.jsonl", "instalacoes.jsonl"))
    if achados:
        alvo.unlink()
        raise SystemExit(f"o pacote do VENDEDOR levaria segredo: {achados}")
    print(f"vendedor   {alvo.name}  emitir, receber, licenca, registro, termos, seguranca")
    return alvo


def pacote_binarios(v: str, saida: Path, pular: bool) -> Path | None:
    if pular:
        print("binarios   pulado (--sem-binarios)")
        return None
    r = subprocess.run([sys.executable, str(RAIZ / "ferramentas/wx-modelos/publicar.py"), "--sem-teste"],
                       capture_output=True, text=True, cwd=RAIZ / "ferramentas/wx-modelos")
    if r.returncode != 0:
        print(f"binarios   FALHOU no publicar.py:\n{r.stdout[-800:]}{r.stderr[-800:]}")
        return None
    dist = RAIZ / "ferramentas/wx-modelos/dist"
    alvo = saida / "binarios" / f"wx-modelos-{v}.zip"
    alvo.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(alvo, "w", zipfile.ZIP_DEFLATED) as z:
        for p in sorted(dist.iterdir()):
            z.write(p, f"wx-modelos/{p.name}")
        z.write(RAIZ / "ferramentas/wx-modelos/LEIA-ME.md", "wx-modelos/LEIA-ME.md")
        z.write(RAIZ / "ferramentas/wx-modelos/exemplo-catalogo.json", "wx-modelos/exemplo-catalogo.json")
    print(f"binarios   {alvo.name}  {[p.name for p in dist.iterdir() if not p.suffix in ('.md', '.json')]}")
    return alvo


def pacote_documentos(v: str, saida: Path) -> Path:
    alvo = saida / "documentos" / f"documentos-{v}.zip"
    alvo.parent.mkdir(parents=True, exist_ok=True)
    itens = [
        ("docs/manual-de-uso.pdf", "manual-de-uso.pdf"),
        ("docs/apresentacao-wx-claude-code.pptx", "apresentacao-wx-claude-code.pptx"),
        ("docs/fluxo-explicado.pdf", "fluxo-explicado.pdf"),
        ("docs/apresentacao.pdf", "roteiro-de-apresentacao.pdf"),
        ("docs/workflow.pdf", "workflow.pdf"),
        ("docs/dossie/fluxo-atual.pdf", "fluxograma.pdf"),
        ("docs/o-que-falta.pdf", "o-que-falta.pdf"),
        ("docs/comandos.pdf", "comandos.pdf"),
        ("docs/relatorio-de-cenarios.pdf", "relatorio-de-cenarios.pdf"),
        ("docs/ativacao-do-serial.pdf", "ativacao-do-serial.pdf"),
        ("docs/ativacao-passo-a-passo.pdf", "ativacao-passo-a-passo.pdf"),
        ("docs/dossie/dossie-wx-claude-code.pdf", "dossie.pdf"),
        ("LICENCA.md", "LICENCA.md"),
        ("docs/SEGURANCA.md", "SEGURANCA.md"),
        ("MANUAL.md", "MANUAL.md"),
        ("README.md", "README.md"),
        ("docs/investidor/wx-claude-code-para-investidores.pdf", "para-investidores.pdf"),
    ]
    faltando, n = [], 0
    with zipfile.ZipFile(alvo, "w", zipfile.ZIP_DEFLATED) as z:
        for origem, nome in itens:
            p = RAIZ / origem
            if p.is_file():
                z.write(p, f"documentos/{nome}")
                n += 1
            else:
                faltando.append(origem)
    print(f"documentos {alvo.name}  {n} arquivos" + (f"  (faltaram: {faltando})" if faltando else ""))
    return alvo


def pacote_videos(v: str, saida: Path) -> Path:
    """Os sete videos num zip proprio: junto dos PDFs o pacote de documentos
    passava de 30 MiB, o limite dos canais por onde isto viaja."""
    alvo = saida / "videos" / f"videos-{v}.zip"
    alvo.parent.mkdir(parents=True, exist_ok=True)
    itens = [("wx-claude-code-video-de-uso.mp4", "video-de-uso.mp4"),
             ("wx-claude-code-video-php.mp4", "video-legado-php.mp4"),
             ("wx-claude-code-video-bateria.mp4", "video-bateria-de-testes.mp4"),
             ("wx-claude-code-video-primeiro.mp4", "video-primeiro-projeto.mp4"),
             ("wx-claude-code-video-windev.mp4", "video-windev-para-rust-react.mp4"),
             ("wx-claude-code-video-passos.mp4", "video-passo-a-passo.mp4"),
             ("wx-claude-code-video-ativacao.mp4", "video-ativacao.mp4")]
    n = 0
    with zipfile.ZipFile(alvo, "w", zipfile.ZIP_STORED) as z:
        for origem, nome in itens:
            p = RAIZ / "docs/video" / origem
            if p.is_file():
                z.write(p, f"videos/{nome}")
                n += 1
    print(f"videos     {alvo.name}  {n} arquivos  {alvo.stat().st_size / 1048576:.1f} MiB")
    return alvo


def main() -> int:
    ap = argparse.ArgumentParser(description="monta os entregaveis em zip")
    ap.add_argument("--saida", type=Path, default=RAIZ / "entregas")
    ap.add_argument("--sem-teste", action="store_true")
    ap.add_argument("--sem-binarios", action="store_true")
    ap.add_argument("--separar-corpus", action="store_true",
                    help="plugin e corpus em zips separados (cada um abaixo de 30 MiB)")
    a = ap.parse_args()
    v = versao()
    if not a.sem_teste:
        print("bateria …", end=" ", flush=True)
        t = subprocess.run([sys.executable, str(RAIZ / "tests/testes.py")], capture_output=True, text=True)
        if t.returncode != 0:
            print("FALHOU\n" + (t.stderr.strip().splitlines() or ["?"])[-1])
            print("teste vermelho nao vira entrega; nada foi empacotado.")
            return 1
        print("ok")
    saida = a.saida.resolve()
    saida.mkdir(parents=True, exist_ok=True)
    pacotes = [pacote_cliente(v, saida, a.separar_corpus), pacote_vendedor(v, saida),
               pacote_binarios(v, saida, a.sem_binarios), pacote_documentos(v, saida), pacote_videos(v, saida)]
    ficha = {"versao": v, "empacotado_em": date.today().isoformat(), "pacotes": []}
    linhas = [f"# Entregáveis WX Claude Code {v}", "", f"Empacotado em {ficha['empacotado_em']}.", "",
              "| arquivo | para quem | bytes | SHA-256 |", "| --- | --- | --- | --- |"]
    publico = {"cliente": "quem compra: o plugin para instalar",
               "vendedor": "SÓ VOCÊ: emissor de serial e receptor do aviso",
               "binarios": "quem usa modelo local: wx-modelos Linux e Windows",
               "documentos": "reunião e contrato: PDFs, termos, segurança, apresentação",
               "videos": "reunião: os sete vídeos, saída real de sessão"}
    if a.separar_corpus:
        pacotes.append(saida / "cliente" / CORPUS.name)
        publico[CORPUS.name] = "quem compra: o corpus do Help, para ./instalar.sh --corpus"
    for p in pacotes:
        if p is None:
            continue
        item = {"pacote": p.parent.name, "arquivo": p.name, "bytes": p.stat().st_size, "sha256": sha256(p)}
        ficha["pacotes"].append(item)
        quem = publico.get(p.name) or publico[item["pacote"]]
        linhas.append(f"| {item['arquivo']} | {quem} | {item['bytes']:,} | `{item['sha256']}` |")
    linhas += ["", "O pacote **vendedor** nunca vai ao cliente: ele carrega o emissor de serial.",
               "O pacote do cliente foi ABERTO depois de escrito e não contém `wx-serial/`, chave",
               "privada, livro de emissões, `target/`, `dist/` nem `.git/`.", ""]
    (saida / "ENTREGA.md").write_text("\n".join(linhas), encoding="utf-8")
    (saida / "entrega.json").write_text(json.dumps(ficha, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"\nficha em {saida / 'ENTREGA.md'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
