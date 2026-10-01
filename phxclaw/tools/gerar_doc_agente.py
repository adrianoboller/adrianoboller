#!/usr/bin/env python3
"""Gera os trechos medidos do docs/AGENTE_AUTONOMO.md -- nenhuma lista de ferramenta, de
comando ou contagem da equipe e digitada no documento.

De onde sai cada trecho (entre os marcadores <!-- gerado:NOME:inicio/fim -->):

  ferramentas  <- `phxclaw ferramentas` (a mesma Montagem do agente, nesta maquina)
  condicionais <- o fonte de crates/phxclaw-agent/src: nome de ToolSpec que existe no codigo
                  e NAO foi montado aqui (falta configuracao, token, canal ou feature)
  cli          <- `phxclaw --help` (o texto que o binario imprime)
  equipe       <- `phxclaw equipe listar` (o "N de M papeis" que a CLI conta)

Por que o fonte entra junto do binario: a montagem varia por maquina (sem SMTP nao ha
send_email, sem token nao ha github). So o binario esconderia a ferramenta que nao montou;
so o fonte nao diria o que de fato roda. Os dois juntos dizem as duas coisas.

E o gerador DIZ quando faz menos: se o binario e mais velho que o fonte do agente ou da CLI,
a tabela sai mesmo assim (e o que roda hoje), mas o aviso vai para o documento e para a saida,
fora das linhas de exito. Medidor com binario velho mede o passado.

Uso: python3 tools/gerar_doc_agente.py [binario]   (padrao: target/debug/phxclaw)
"""

import datetime
import json
import os
import re
import subprocess
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
DOC = RAIZ / "docs" / "AGENTE_AUTONOMO.md"
FONTE_AGENTE = RAIZ / "crates" / "phxclaw-agent" / "src"
FONTE_CLI = RAIZ / "apps" / "phxclaw" / "src"
# Ferramentas do proprio motor, que nao passam pela Montagem: aparecem ao modelo por outro
# caminho (fim da tarefa, pergunta ao usuario) e nao sao "ferramenta montada".
DO_MOTOR = {"final_answer", "ask_user"}


def rodar(bin_, *args, env=None):
    r = subprocess.run([str(bin_), *args], capture_output=True, text=True, env=env, timeout=120)
    if r.returncode != 0:
        sys.exit(f"ERRO: {bin_} {' '.join(args)} saiu {r.returncode}: {r.stderr.strip()}")
    return r.stdout


def mais_novo(*pastas):
    arquivos = [p for d in pastas for p in d.rglob("*.rs")]
    return max(arquivos, key=lambda p: p.stat().st_mtime)


def nomes_no_fonte():
    """Nome de ToolSpec por arquivo. Literais direto; os das forjas se expandem pelo
    `fn nome` do enum Forja, para a lista sair do codigo e nao de uma copia aqui."""
    achados = {}
    for arq in sorted(FONTE_AGENTE.rglob("*.rs")):
        txt = arq.read_text(encoding="utf-8")
        rel = arq.relative_to(RAIZ).as_posix()
        for m in re.finditer(r'ToolSpec\s*\{\s*name:\s*"([a-z0-9_]+)"', txt):
            achados.setdefault(m.group(1), rel)
        if re.search(r'name:\s*f\.into\(\)', txt):
            forjas = re.findall(r'Forja::\w+\s*=>\s*"([a-z]+)",', txt)
            # so os do `fn nome` (o primeiro match de cada variante)
            vistos = []
            for f in forjas:
                if f not in vistos:
                    vistos.append(f)
            escrita = re.search(r'format!\("\{f\}(_\w+)"\)', txt)
            for f in vistos:
                achados.setdefault(f, rel)
                if escrita:
                    achados.setdefault(f + escrita.group(1), rel)
    return achados


def trocar(texto, nome, corpo):
    ini, fim = f"<!-- gerado:{nome}:inicio -->", f"<!-- gerado:{nome}:fim -->"
    padrao = re.compile(re.escape(ini) + r".*?" + re.escape(fim), re.S)
    if not padrao.search(texto):
        sys.exit(f"ERRO: marcador {ini} ausente em {DOC}")
    return padrao.sub(lambda _: f"{ini}\n{corpo.rstrip()}\n{fim}", texto)


def celula(s):
    return s.replace("|", "\\|").replace("\n", " ")


def main():
    bin_ = Path(sys.argv[1]) if len(sys.argv) > 1 else RAIZ / "target" / "debug" / "phxclaw"
    if not bin_.is_file():
        sys.exit(f"ERRO: {bin_} nao existe; rode `cargo build -p phxclaw` antes")
    env = {k: v for k, v in os.environ.items() if k != "PHXCLAW_CAPACIDADES"}

    hoje = datetime.date.today().isoformat()
    m_bin = datetime.datetime.fromtimestamp(bin_.stat().st_mtime)
    novo = mais_novo(FONTE_AGENTE, FONTE_CLI)
    m_fonte = datetime.datetime.fromtimestamp(novo.stat().st_mtime)
    velho = m_bin < m_fonte

    f = json.loads(rodar(bin_, "ferramentas", env=env))
    lista = f["ferramentas"]
    montadas = {t["nome"] for t in lista}

    aviso = ""
    if velho:
        aviso = (
            f"\n> **Binario mais velho que o fonte**: binario de {m_bin:%Y-%m-%d %H:%M}, "
            f"`{novo.relative_to(RAIZ).as_posix()}` de {m_fonte:%Y-%m-%d %H:%M}. A tabela e o "
            "que o binario monta, nao necessariamente o que o fonte de hoje montaria.\n"
        )

    linhas = [
        f"Medido em {hoje} por `python3 tools/gerar_doc_agente.py`, de `phxclaw ferramentas` "
        f"(versao {f['versao']}, binario de {m_bin:%Y-%m-%d %H:%M}), com `PHXCLAW_CAPACIDADES` "
        f"no padrao. **{f['total']} ferramentas montadas nesta maquina**, "
        f"{sum(t['concedida'] for t in lista)} concedidas por padrao.",
        aviso,
        "| Capacidade | Padrao | Ferramentas |",
        "|---|---|---|",
    ]
    por_cap = {}
    for t in lista:
        por_cap.setdefault(t["capacidade"], []).append(t)
    for cap in sorted(por_cap):
        ts = por_cap[cap]
        padrao = "sim" if all(t["concedida"] for t in ts) else "**nao**"
        nomes = ", ".join("`" + t["nome"] + "`" for t in ts)
        linhas.append(f"| `{cap}` | {padrao} | {nomes} |")
    linhas += ["", "<details><summary>Descricao de cada uma (a que o modelo le)</summary>", ""]
    linhas += ["| Ferramenta | Capacidade | Descricao |", "|---|---|---|"]
    for t in lista:
        linhas.append(f"| `{t['nome']}` | `{t['capacidade']}` | {celula(t['descricao'])} |")
    linhas += ["", "</details>"]
    texto = DOC.read_text(encoding="utf-8")
    texto = trocar(texto, "ferramentas", "\n".join(linhas))

    fonte = nomes_no_fonte()
    ausentes = sorted((n, a) for n, a in fonte.items() if n not in montadas and n not in DO_MOTOR)
    nao_no_fonte = sorted(montadas - set(fonte))
    cond = [
        f"Medido em {hoje}: **{len(ausentes)} ferramentas existem no fonte e nao montaram "
        "nesta maquina** (dependem de configuracao, token, canal ou feature de compilacao; "
        "a condicao de cada uma esta em `crates/phxclaw-agent/src/montagem.rs`).",
        "",
        "| Ferramenta | Definida em |",
        "|---|---|",
    ]
    cond += [f"| `{n}` | `{a}` |" for n, a in ausentes]
    if nao_no_fonte:
        # Ferramenta MCP externa ou nome dinamico que o extrator nao le: diz, nao esconde.
        cond += ["", "Montadas sem nome literal no fonte (MCP externo ou nome dinamico): "
                 + ", ".join(f"`{n}`" for n in nao_no_fonte) + "."]
    texto = trocar(texto, "condicionais", "\n".join(cond))

    ajuda = rodar(bin_, "--help", env=env)
    texto = trocar(texto, "cli", f"Saida de `phxclaw --help`, gerada em {hoje}:\n\n```text\n"
                   f"{ajuda.rstrip()}\n```")

    eq = rodar(bin_, "equipe", "listar", env=env).splitlines()[0]
    m = re.match(r"(\d+) de (\d+) papeis", eq)
    if not m:
        sys.exit(f"ERRO: primeira linha de `equipe listar` mudou de forma: {eq!r}")
    texto = trocar(texto, "equipe", f"**{m.group(2)} papeis** carregados de `config/agents` "
                   f"(medido em {hoje} por `phxclaw equipe listar`).")

    DOC.write_text(texto, encoding="utf-8")
    print(f"{DOC.relative_to(RAIZ)}: {f['total']} montadas, {len(ausentes)} so no fonte, "
          f"{m.group(2)} papeis, ajuda da CLI")
    if velho:
        print(f"FEZ MENOS: binario ({m_bin:%H:%M}) mais velho que {novo.relative_to(RAIZ)} "
              f"({m_fonte:%H:%M}); recompile e rode de novo", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
