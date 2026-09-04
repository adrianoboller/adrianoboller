#!/usr/bin/env python3
"""Arquiva um artefato submetido pelo usuario em artefatos/<tipo>/ (bloco M).

Um artefato e qualquer coisa que o cliente manda fora da evidencia do WX:
anotacao de reuniao, PDF com as classes OOP, arquivo .sql de consultas, modelo
de relatorio impresso, manual, contrato de API, codigo PHP. Cada um so entra
com **onde_usar** preenchido: artefato sem destino declarado vira arquivo que
ninguem abre.

O que este script garante, e por isso ele existe em vez de um `cp`:
  - o tipo e um dos aceitos e onde_usar nao esta vazio;
  - o conteudo de texto nao carrega token nem chave privada (senha nunca em
    texto puro, nem dentro de um anexo que o cliente mandou sem perceber);
  - o SHA-256 identifica o arquivo, entao reenviar o mesmo arquivo nao duplica
    e reenviar um arquivo diferente com o mesmo nome e recusado, nao sobrescrito;
  - registro.json e CATALOGO.md sao regravados dos fatos, nunca a mao;
  - o bloco M do questionario recebe a mesma entrada, para que reaplicar o
    questionario num projeto novo recrie o catalogo.

Uso:
  arquivar_artefato.py --project-root . --arquivo /caminho/notas.txt \\
      --tipo anotacao --onde-usar "G1: regras ditadas pelo cliente" \\
      [--descricao "..."] [--origem cliente] [--confidencial] \\
      [--questionario .wx-migration/questionario.json]
  arquivar_artefato.py --project-root . catalogo     # so regrava o CATALOGO.md
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import sys
from datetime import date
from pathlib import Path

TIPOS = {
    "anotacao": "anotação, ata, e-mail ou transcrição do cliente",
    "classe-oop": "classes OOP em PDF ou texto (WLanguage, PHP, outra)",
    "query-sql": "consultas SQL soltas, fora do projeto",
    "relatorio": "modelo de relatório impresso pelo legado",
    "regra-de-negocio": "regra escrita, norma interna, planilha de cálculo",
    "tela": "captura ou desenho de tela fora do F0",
    "manual": "manual do usuário ou do sistema",
    "contrato-de-api": "contrato de integração, WSDL, OpenAPI, exemplo de payload",
    "codigo-php": "código PHP do legado (arquivo ou pasta compactada)",
    "dado-de-amostra": "dados anonimizados para o golden master",
    "outro": "o que não couber acima; descreva bem",
}
TOKEN = re.compile(r"ghp_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|sk-[A-Za-z0-9_-]{20,}|AKIA[0-9A-Z]{16}|xox[baprs]-[A-Za-z0-9-]{10,}|-----BEGIN [A-Z ]*PRIVATE KEY-----|eyJ[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{10,}")
# Nome de arquivo: sem caminho, sem espaco no inicio/fim, sem caractere que
# atrapalhe shell ou sistema de arquivos.
RX_NOME = re.compile(r"^[A-Za-z0-9][A-Za-z0-9 ._+-]{0,120}$")
TEXTO = {".txt", ".md", ".sql", ".csv", ".json", ".xml", ".yaml", ".yml", ".php", ".html", ".htm", ".ini", ".log", ".wl", ".cs", ".py", ".ts", ".js"}


def sha256(caminho: Path) -> str:
    h = hashlib.sha256()
    with caminho.open("rb") as f:
        for pedaco in iter(lambda: f.read(1 << 20), b""):
            h.update(pedaco)
    return h.hexdigest()


def registro_de(pasta: Path) -> dict:
    arq = pasta / "registro.json"
    if arq.is_file():
        try:
            dados = json.loads(arq.read_text(encoding="utf-8"))
            if isinstance(dados, dict) and isinstance(dados.get("itens"), list):
                return dados
        except (OSError, json.JSONDecodeError):
            pass
    return {"gerado_por": "arquivar_artefato.py", "itens": []}


def catalogo_md(reg: dict, pasta: Path) -> str:
    itens = sorted(reg.get("itens", []), key=lambda i: (i.get("tipo", ""), i.get("arquivo", "")))
    L = ["# Catálogo de artefatos", "",
         f"Gerado por `arquivar_artefato.py` em {date.today().isoformat()}. **Não edite à mão**: submeta o artefato pelo script e ele regrava este arquivo.",
         "", f"{len(itens)} artefato{'s' if len(itens) != 1 else ''} em `{pasta.name}/`. Cada linha diz onde aquele artefato entra; artefato sem destino não é submetido.", ""]
    if not itens:
        L += ["Nenhum artefato submetido ainda.", ""]
        return "\n".join(L)
    por_tipo: dict[str, list[dict]] = {}
    for i in itens:
        por_tipo.setdefault(i.get("tipo", "outro"), []).append(i)
    L += ["| tipo | quantos |", "| --- | ---: |"]
    L += [f"| `{t}` | {len(v)} |" for t, v in sorted(por_tipo.items())]
    L.append("")
    for t, v in sorted(por_tipo.items()):
        L += [f"## {t} — {TIPOS.get(t, '')}", "",
              "| arquivo | onde usar | o que é | origem | submetido | sha-256 |",
              "| --- | --- | --- | --- | --- | --- |"]
        for i in v:
            marca = " 🔒" if i.get("confidencial") else ""
            L.append(f"| `{t}/{i.get('arquivo','')}`{marca} | {i.get('onde_usar','')} | {i.get('descricao','')} | {i.get('origem','')} | {i.get('submetido_em','')} | `{(i.get('sha256') or '')[:12]}` |")
        L.append("")
    L += ["Marcados com 🔒 são confidenciais: não copie o conteúdo para documento gerado nem para resposta; cite o arquivo.", ""]
    return "\n".join(L)


def gravar_catalogo(pasta: Path, reg: dict) -> None:
    pasta.mkdir(parents=True, exist_ok=True)
    (pasta / "registro.json").write_text(json.dumps(reg, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    (pasta / "CATALOGO.md").write_text(catalogo_md(reg, pasta), encoding="utf-8")


def erro(msg: str) -> int:
    print(f"erro: {msg}", file=sys.stderr)
    return 2


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("comando", nargs="?", default="submeter", choices=["submeter", "catalogo"])
    p.add_argument("--project-root", required=True, type=Path)
    p.add_argument("--pasta", default="artefatos", help="pasta dos artefatos dentro do projeto")
    p.add_argument("--arquivo", type=Path)
    p.add_argument("--tipo", choices=sorted(TIPOS))
    p.add_argument("--onde-usar", default="")
    p.add_argument("--descricao", default="")
    p.add_argument("--origem", default="cliente")
    p.add_argument("--confidencial", action="store_true")
    p.add_argument("--questionario", type=Path, help="atualiza o bloco M com a mesma entrada")
    a = p.parse_args()

    projeto = a.project_root.resolve()
    pasta = (projeto / a.pasta).resolve()
    if projeto not in pasta.parents and pasta != projeto:
        return erro("a pasta de artefatos tem de ficar dentro do projeto")
    reg = registro_de(pasta)

    if a.comando == "catalogo":
        gravar_catalogo(pasta, reg)
        print(f"CATALOGO {pasta / 'CATALOGO.md'} ({len(reg['itens'])} artefatos)")
        return 0

    if not a.arquivo or not a.tipo:
        return erro("submeter exige --arquivo e --tipo")
    origem = a.arquivo.expanduser()
    if not origem.is_file():
        return erro(f"{origem} nao existe ou nao e arquivo")
    if not a.onde_usar.strip():
        return erro("--onde-usar e obrigatorio: artefato sem destino declarado vira arquivo que ninguem abre")
    if not RX_NOME.fullmatch(origem.name):
        return erro(f"nome de arquivo recusado: {origem.name!r}; renomeie sem caminho nem caractere especial")

    if origem.suffix.lower() in TEXTO:
        try:
            conteudo = origem.read_text(encoding="utf-8", errors="ignore")
        except OSError as e:
            return erro(f"nao consegui ler {origem}: {e}")
        if TOKEN.search(conteudo):
            return erro("o arquivo carrega token ou chave privada; tire o segredo antes de submeter (senha nunca em texto puro)")

    h = sha256(origem)
    destino = pasta / a.tipo / origem.name
    ja = next((i for i in reg["itens"] if i.get("tipo") == a.tipo and i.get("arquivo") == origem.name), None)
    if ja and ja.get("sha256") == h and (pasta / a.tipo / origem.name).is_file():
        print(f"JA ARQUIVADO {a.tipo}/{origem.name} com o mesmo conteudo (sha {h[:12]}); nada a fazer")
        return 0
    if ja and ja.get("sha256") != h:
        return erro(f"ja existe {a.tipo}/{origem.name} com outro conteudo (sha {ja.get('sha256','')[:12]}); renomeie o novo, o arquivado nao se sobrescreve")
    if any(i.get("sha256") == h and i is not ja for i in reg["itens"]):
        outro = next(i for i in reg["itens"] if i.get("sha256") == h)
        print(f"JA ARQUIVADO como {outro.get('tipo')}/{outro.get('arquivo')} (mesmo sha-256); nada a fazer")
        return 0

    destino.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(origem, destino)
    item = {"arquivo": origem.name, "tipo": a.tipo, "onde_usar": a.onde_usar.strip(), "descricao": a.descricao.strip(),
            "origem": a.origem.strip(), "confidencial": bool(a.confidencial), "submetido_em": date.today().isoformat(),
            "bytes": destino.stat().st_size, "sha256": h}
    if ja:
        reg["itens"][reg["itens"].index(ja)] = item
    else:
        reg["itens"].append(item)
    gravar_catalogo(pasta, reg)
    print(f"ARQUIVADO {destino.relative_to(projeto)} ({item['bytes']} bytes, sha {h[:12]})")

    if a.questionario:
        try:
            q = json.loads(a.questionario.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as e:
            return erro(f"nao consegui ler o questionario: {e}")
        m = q.setdefault("M_artefatos", {"pasta": f"./{a.pasta}", "itens": [], "observacao": ""})
        itens = m.setdefault("itens", [])
        publico = {k: v for k, v in item.items() if k not in {"bytes", "sha256"}}
        antigo = next((i for i in itens if i.get("arquivo") == item["arquivo"] and i.get("tipo") == item["tipo"]), None)
        if antigo:
            itens[itens.index(antigo)] = publico
        else:
            itens.append(publico)
        a.questionario.write_text(json.dumps(q, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        print(f"QUESTIONARIO {a.questionario} (bloco M com {len(itens)} artefatos)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
