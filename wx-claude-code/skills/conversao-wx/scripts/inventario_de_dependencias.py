#!/usr/bin/env python3
"""O que o legado usa DE FORA dele -- e que a conversao vai ter de resolver.

Cada dependencia externa e uma decisao de conversao. Hoje elas aparecem uma a
uma, quando o agente tropeca: chega no `INIRead` e descobre que ha um arquivo
de configuracao; chega no `SOAPExecute` e descobre que ha um webservice.

Este script varre o texto que o projeto ja tem -- as evidencias extraidas dos
PDFs, o codigo do legado -- e lista o que achou, com `arquivo#linha`. Ele acha
por SINAL: a chamada de funcao que so existe quando a dependencia existe.

O que ele NAO ve, e diz que nao ve, em vez de deixar a lista parecer completa:
componente `.wdk` referenciado so no projeto, DLL declarada no IDE e nunca
chamada no codigo impresso, driver de impressora configurado fora do codigo,
e qualquer coisa que nao esteja no texto que chegou. A lista e um PISO, nao um
inventario fechado -- e o relatorio diz isso na primeira linha.

Uso:
  inventario_de_dependencias.py [--project-root .] [--json] [--gravar]
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

# (categoria, o que significa na conversao, [(sinal, linguagem)])
# O sinal e a chamada que so existe quando a dependencia existe. Nada de
# palavra solta: "Email" casaria com um comentario sobre e-mail.
CATALOGO: list[tuple[str, str, list[tuple[str, str]]]] = [
    ("configuracao", "arquivo de configuração ou registro do Windows lido em tempo de execução", [
        (r"\bINIRead\s*\(", "wl"), (r"\bINIWrite\s*\(", "wl"),
        (r"\bRegistry(?:QueryValue|SetValue|Exist)\s*\(", "wl"),
        (r"\bparse_ini_file\s*\(", "php"), (r"\bgetenv\s*\(", "php")]),
    ("banco externo", "conexão com servidor de banco fora do processo", [
        (r"\bhAccessHFClientServer\b", "wl"), (r"\bHOpenConnection\s*\(", "wl"),
        (r"\bhAccessODBC\b|\bhNativeAccess\w*\b", "wl"),
        (r"\bnew\s+PDO\s*\(", "php"), (r"\bmysqli_connect\s*\(", "php")]),
    ("dll e api", "biblioteca nativa chamada diretamente", [
        (r"\bAPI\s*\(", "wl"), (r"\bCallDLL32\s*\(", "wl"), (r"\bLoadDLL\s*\(", "wl"),
        (r"\bDotNetRun\b|\bdotNet\w+\s*\(", "wl"),
        (r"\bLoadLibrary\w*\s*\(", "cpp"), (r"#include\s*<windows\.h>", "cpp"),
        (r"\bdl_open\b|\bdlopen\s*\(", "cpp")]),
    ("com e activex", "objeto COM/OLE do sistema ou do Office", [
        (r"\bis\s+(?:ole|activeX)Object\b", "wl"), (r"\bActiveXEvent\s*\(", "wl"),
        (r"\bCreateObject\s*\(|\bnew\s+COM\s*\(", "php"),
        (r"\bxls(?:Open|Close|Data)\s*\(|\bdocOpen\s*\(", "wl")]),
    ("webservice", "serviço de terceiro chamado pela rede", [
        (r"\bSOAP(?:Execute|Run|Add)\w*\s*\(", "wl"),
        (r"\bHTTPRequest\s*\(|\bHTTPSend\s*\(|\bRESTSend\s*\(", "wl"),
        (r"\bcurl_init\s*\(|\bfile_get_contents\s*\(\s*['\"]https?:", "php")]),
    ("e-mail", "envio ou leitura de e-mail", [
        (r"\bEmail(?:SendMessage|StartSession|OpenSMTPSession|ReadMessage)\w*\s*\(", "wl"),
        (r"\bmail\s*\(|\bPHPMailer\b", "php")]),
    ("ftp", "transferência de arquivo por FTP", [
        (r"\bFTP(?:Connect|Send|Get|Disconnect)\w*\s*\(", "wl"), (r"\bftp_connect\s*\(", "php")]),
    ("impressao", "relatório ou impressora acionada pelo código", [
        (r"\biPrint(?:Report)?\s*\(", "wl"), (r"\biDestination\s*\(", "wl"),
        (r"\biConfigure\w*\s*\(", "wl")]),
    ("componente", "componente WX interno ou externo (.wdk)", [
        (r"\bComponent(?:Open|Execute|Info)\s*\(", "wl"), (r"\.wdk\b", "wl")]),
    ("agendamento e processo", "outro programa ou serviço acionado", [
        (r"\bExeRun\s*\(|\bLanceAppli\s*\(", "wl"), (r"\bShellExecute\w*\s*\(", "cpp"),
        (r"\bexec\s*\(|\bshell_exec\s*\(|\bproc_open\s*\(", "php")]),
]

# Onde procurar. O texto que o projeto ja tem: evidencia extraida e fonte.
EXTENSOES = {".md", ".txt", ".wl", ".wlg", ".php", ".c", ".cpp", ".h", ".hpp", ".sql", ".clw", ".txa"}
IGNORAR = {".git", "node_modules", "target", "__pycache__", ".venv"}

# O que este inventario nao alcanca, dito sempre -- a lista e um piso.
CEGO = [
    "componente .wdk referenciado só no projeto, sem chamada no código impresso",
    "DLL declarada no IDE (Descrição do projeto → Bibliotecas) e nunca chamada",
    "driver de impressora e de scanner configurados fora do código",
    "conexão criada pelo assistente do IDE, sem HOpenConnection no texto",
    "qualquer coisa que não esteja no texto que chegou (PDF cortado, anexo faltando)",
]


def arquivos(raiz: Path):
    for p in sorted(raiz.rglob("*")):
        if p.is_file() and p.suffix.lower() in EXTENSOES:
            if not any(parte in IGNORAR for parte in p.parts):
                yield p


def varrer(raiz: Path) -> list[dict]:
    achados = []
    compilado = [(cat, o_que, [(re.compile(rx, re.I), ling) for rx, ling in sinais])
                 for cat, o_que, sinais in CATALOGO]
    for arq in arquivos(raiz):
        try:
            texto = arq.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        for n, linha in enumerate(texto.splitlines(), 1):
            for cat, o_que, sinais in compilado:
                for rx, ling in sinais:
                    m = rx.search(linha)
                    if m:
                        achados.append({
                            "categoria": cat, "significa": o_que, "linguagem": ling,
                            "sinal": m.group(0).strip(),
                            "onde": f"{arq.relative_to(raiz)}#{n}",
                            "trecho": linha.strip()[:160],
                        })
                        break
    return achados


def relatorio(achados: list[dict]) -> str:
    por_cat: dict[str, list[dict]] = {}
    for a in achados:
        por_cat.setdefault(a["categoria"], []).append(a)
    L = ["# Dependências externas do legado", "",
         "Gerado por `inventario_de_dependencias.py`; não edite à mão.", "",
         "**Esta lista é um piso, não um inventário fechado.** Ela acha o que tem",
         "sinal no texto que chegou. O que ela não alcança está no fim.", ""]
    if not achados:
        L += ["Nenhum sinal encontrado no texto disponível — o que **não** quer dizer",
              "que não haja dependência: veja o que o inventário não alcança, abaixo.", ""]
    for cat, itens in por_cat.items():
        L += [f"## {cat} ({len(itens)})", "", f"_{itens[0]['significa']}_", "",
              "| sinal | onde | trecho |", "| --- | --- | --- |"]
        vistos = set()
        for i in itens:
            chave = (i["sinal"].lower(), i["onde"])
            if chave in vistos:
                continue
            vistos.add(chave)
            trecho = i["trecho"].replace("|", "\\|")
            L.append(f"| `{i['sinal']}` | `{i['onde']}` | {trecho} |")
        L.append("")
    L += ["## O que este inventário NÃO alcança", ""]
    L += [f"- {c}" for c in CEGO]
    L += ["", "Cada item acima vira `GAP-*` se ninguém confirmar que não existe.", ""]
    return "\n".join(L)


def main() -> int:
    p = argparse.ArgumentParser(description="dependências externas achadas no texto do legado")
    p.add_argument("--project-root", default=".")
    p.add_argument("--json", action="store_true")
    p.add_argument("--gravar", action="store_true", help="grava .wx-migration/dependencias.md e .json")
    args = p.parse_args()
    raiz = Path(args.project_root).resolve()
    achados = varrer(raiz)
    cats = sorted({a["categoria"] for a in achados})
    if args.json:
        print(json.dumps({"achados": achados, "categorias": cats,
                          "nao_alcanca": CEGO}, ensure_ascii=False, indent=2))
    else:
        print(relatorio(achados))
    if args.gravar:
        pasta = raiz / ".wx-migration"
        pasta.mkdir(parents=True, exist_ok=True)
        (pasta / "dependencias.md").write_text(relatorio(achados), encoding="utf-8")
        (pasta / "dependencias.json").write_text(
            json.dumps({"achados": achados, "categorias": cats, "nao_alcanca": CEGO},
                       ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        print(f"\ngravado em {pasta / 'dependencias.md'} ({len(achados)} sinais, {len(cats)} categorias)",
              file=sys.stderr)
    return 0


try:
    import registro
except ImportError:
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import registro


if __name__ == "__main__":
    sys.exit(registro.envolver(__file__, main))
