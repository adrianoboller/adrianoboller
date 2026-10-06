#!/usr/bin/env python3
"""Leitores do dossie do PhxClaw: cada numero visivel sai daqui, com a FONTE e a DATA.

Regra da casa (papel H): todo numero visivel sai de um gerador, ou esta errado e ninguem
percebeu ainda. E a receita de um numero tambem envelhece: quando um numero depende de uma
lista, a lista sai do codigo -- aqui, nenhuma lista de fontes, de estados, de papeis, de
medidores ou de portoes e digitada; cada uma e lida do arquivo que a decide.

A DATA de cada numero:
  - o proprio arquivo diz quando mediu (`data`, `lido_em`, `certified_at`)  -> essa;
  - o arquivo esta comitado e limpo                                         -> data do commit;
  - senao                                                                    -> data do arquivo
    (mtime), e a pagina DIZ que e data do arquivo, nao data da medicao.
Nenhuma data sai do relogio da corrida: rodar o gerador duas vezes nao muda um byte.

Medidor sem arquivo de resultado nao some: volta como NAO MEDIDO, com o comando para rodar.
"""
from __future__ import annotations

import json
import re
import subprocess
from datetime import datetime, timezone
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
REPO = RAIZ.parent
AGENTES = REPO / ".claude/agents"


# ---------------------------------------------------------------- utilidades

def sh(cmd: list[str], cwd: Path = RAIZ) -> str:
    return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True).stdout


def rel(p: Path) -> str:
    try:
        return str(p.relative_to(RAIZ))
    except ValueError:
        return str(p.relative_to(REPO))


def fmt_data(d: datetime, hora: bool = True) -> str:
    return d.strftime("%d/%m/%Y %H:%M" if hora else "%d/%m/%Y")


def data_de(p: Path) -> dict:
    """Quando o conteudo deste arquivo foi fixado: commit se limpo, senao mtime declarado."""
    limpo = not sh(["git", "status", "--porcelain", "--", str(p)]).strip()
    iso = sh(["git", "log", "-1", "--format=%cI", "--", str(p)]).strip() if limpo else ""
    if iso:
        d = datetime.fromisoformat(iso).astimezone(timezone.utc)
        h = sh(["git", "log", "-1", "--format=%h", "--", str(p)]).strip()
        return {"quando": d, "texto": f"{fmt_data(d)} UTC", "como": f"commit {h}"}
    d = datetime.fromtimestamp(p.stat().st_mtime, timezone.utc)
    rastreado = bool(sh(["git", "ls-files", "--", str(p)]).strip())
    return {"quando": d, "texto": f"{fmt_data(d)} UTC",
            "como": "data do arquivo: " + ("mudado e nao comitado" if rastreado else "fora do git")}


def data_declarada(texto: str, fmt: str) -> dict:
    d = datetime.strptime(texto, fmt).replace(tzinfo=timezone.utc)
    return {"quando": d, "texto": fmt_data(d, hora="%H" in fmt) + (" UTC" if "%H" in fmt else ""),
            "como": "data gravada no proprio resultado"}


def br(n: float, casas: int = 0) -> str:
    s = f"{n:,.{casas}f}"
    return s.replace(",", "§").replace(".", ",").replace("§", ".")


# ---------------------------------------------------------------- projeto

def versao() -> dict:
    arq = RAIZ / "Cargo.toml"
    t = arq.read_text(encoding="utf-8")
    pac = t.split("[workspace.package]", 1)[1]
    membros = re.findall(r'^\s*"((?:crates|apps)/[^"]+)"', t.split("members", 1)[1].split("]", 1)[0], re.M)
    return {"versao": re.search(r'^version\s*=\s*"([^"]+)"', pac, re.M).group(1),
            "membros": len(membros), "fonte": rel(arq), "data": data_de(arq)}


def codigo() -> dict:
    """Linhas de Rust da arvore de trabalho (crates/ e apps/), sem target nem vendor."""
    arqs = [p for base in ("crates", "apps") for p in (RAIZ / base).rglob("*.rs")
            if "target" not in p.parts and "node_modules" not in p.parts]
    linhas = sum(len(p.read_text(errors="replace").splitlines()) for p in arqs)
    return {"linhas": linhas, "arquivos": len(arqs)}


def git() -> dict:
    ramo = sh(["git", "branch", "--show-current"]).strip()
    total = int(sh(["git", "rev-list", "--count", "HEAD", "--", "."]).strip() or 0)
    cabeca = sh(["git", "log", "-1", "--format=%h%x1f%cI"]).strip().split("\x1f")
    ultimos = []
    for l in sh(["git", "log", "-12", "--format=%h%x1f%cI%x1f%s", "--", "."]).splitlines():
        h, iso, s = l.split("\x1f", 2)
        ultimos.append({"hash": h, "data": fmt_data(datetime.fromisoformat(iso).astimezone(timezone.utc)), "assunto": s})
    sujos = [l for l in sh(["git", "status", "--porcelain", "--", "."]).splitlines() if l.strip()]
    d = datetime.fromisoformat(cabeca[1]).astimezone(timezone.utc)
    return {"ramo": ramo, "commits": total, "cabeca": cabeca[0],
            "cabeca_data": {"quando": d, "texto": f"{fmt_data(d)} UTC", "como": f"commit {cabeca[0]}"},
            "ultimos": ultimos, "sujos": len(sujos),
            "sujos_novos": sum(1 for l in sujos if l.startswith("??"))}


def cognicoes() -> dict:
    pasta = RAIZ / "docs/cognicao"
    estados: dict[str, int] = {}
    for p in sorted(pasta.glob("cognicao_*.md")):
        m = re.search(r"\*\*Estado:\*\*\s*([A-ZÁÉÍÓÚÇ]+)", p.read_text(encoding="utf-8"))
        e = m.group(1) if m else "SEM ESTADO"
        estados[e] = estados.get(e, 0) + 1
    return {"total": sum(estados.values()), "estados": estados, "fonte": rel(pasta)}


# ---------------------------------------------------------------- absorcao

def nomes_de_produto() -> dict[str, str]:
    """Os nomes de exibicao das fontes saem da interface (NOMES_PRODUTO do app.js): um lugar so."""
    js = (RAIZ / "apps/phxclaw-ui/assets/app.js").read_text(encoding="utf-8")
    bloco = re.search(r"const NOMES_PRODUTO\s*=\s*\{([^}]*)\}", js).group(1)
    return dict(re.findall(r"(\w+):\s*'([^']+)'", bloco))


def absorcao() -> dict:
    arq = RAIZ / "docs/absorcao/absorcao.json"
    d = json.loads(arq.read_text(encoding="utf-8"))
    nomes = nomes_de_produto()
    fontes = []
    for chave, v in d.items():
        # Os pontos «com bibliotecas» saem da porcentagem que o gerar_absorcao.py gravou (o peso
        # do meio ponto mora la, nao aqui), arredondados ao meio ponto que ela representa.
        pontos = round(v["pct_com_bibliotecas"] * v["total"] / 100 * 2) / 2
        fontes.append({"chave": chave, "nome": nomes.get(chave, chave), "sem_nome": chave not in nomes,
                       **{k: v[k] for k in ("total", "no_agente", "parcial", "nao", "pct_agente",
                                            "pct_com_bibliotecas", "falta", "pela_metade", "lido_em")},
                       "pontos": pontos})
    total = sum(f["total"] for f in fontes)
    agente = sum(f["no_agente"] for f in fontes)
    pontos = sum(f["pontos"] for f in fontes)
    lidos = sorted({f["lido_em"] for f in fontes})
    return {"fontes": fontes, "itens": total, "no_agente": agente,
            "parcial": sum(f["parcial"] for f in fontes), "nao": sum(f["nao"] for f in fontes),
            "pct_agente": 100 * agente / total if total else 0.0,
            "pct_bibliotecas": 100 * pontos / total if total else 0.0,
            "fonte": rel(arq), "data": data_de(arq),
            "lido_em": data_declarada(lidos[-1], "%d/%m/%Y") if lidos else None}


# ---------------------------------------------------------------- sprints

ORDEM_ESTADOS = ["CONCLUÍDA", "EM EXECUÇÃO", "PLANEJADA", "BLOQUEADA"]



def esteira() -> dict:
    """A esteira de producao (docs/ESTEIRA_ABSORCAO.md): itens E*/U* com o estado na ultima coluna.
    Era a tabela do dossie anterior; volta pelo mesmo leitor, para a pagina nova nao perder dado."""
    arq = RAIZ / "docs/ESTEIRA_ABSORCAO.md"
    itens = []
    for linha in arq.read_text(encoding="utf-8").splitlines():
        m = re.match(r"\|\s*([EU]\d+[a-z]?)\s*\|\s*(.+?)\s*\|(.+)\|\s*$", linha)
        if not m:
            continue
        ultima = [c.strip() for c in m.group(3).split("|")][-1]
        # ⏸ e «depois da versao» (decisao do dono, 24/09): visivel, fora da conta
        estado = ("feito" if "✓" in ultima or "☑" in ultima else
                  "depois" if "⏸" in ultima else
                  "bloqueado" if "bloqueado" in ultima else "aberto")
        nota = ultima.replace("☐", "").replace("⏸", "").strip() if estado != "aberto" else ""
        itens.append({"id": m.group(1), "texto": m.group(2), "estado": estado, "nota": nota})
    if not itens:
        raise SystemExit(f"esteira: nenhum item lido de {rel(arq)} -- o formato mudou")
    return {"itens": itens, "fonte": rel(arq), "data": data_de(arq)}

def sprints() -> dict:
    arq = RAIZ / "docs/absorcao/SPRINTS.md"
    texto = arq.read_text(encoding="utf-8")
    visao = texto.split("## Visão geral", 1)[1].split("\n## ", 1)[0]
    itens = []
    for linha in visao.splitlines():
        cols = [c.strip() for c in linha.strip().strip("|").split("|")]
        if len(cols) != 5 or not re.match(r"^(SP\d+|UI-R\d+)$", cols[0]):
            continue
        estado_txt = cols[4]
        estado = next((e for e in ORDEM_ESTADOS if estado_txt.upper().startswith(e)), None)
        if estado is None:
            raise SystemExit(f"PARADA: estado de sprint desconhecido em SPRINTS.md: {cols[0]} «{estado_txt}»")
        detalhe = re.search(r"\(([^)]*)\)", estado_txt)
        itens.append({"id": cols[0], "onda": cols[1], "foco": cols[2], "chaves": cols[3],
                      "estado": estado, "detalhe": detalhe.group(1) if detalhe else ""})
    if not itens:
        raise SystemExit("PARADA: a tabela «Visão geral» do SPRINTS.md nao deu nenhuma sprint")
    contagem = {e: sum(1 for i in itens if i["estado"] == e) for e in ORDEM_ESTADOS}
    return {"itens": itens, "contagem": contagem, "total": len(itens),
            "fonte": rel(arq), "data": data_de(arq), "texto": texto}


def secao_md(texto: str, titulo_comeca: str) -> str:
    """Corpo de uma secao `## ...` do Markdown, ate a proxima `## ` (vazio se nao houver)."""
    for parte in texto.split("\n## ")[1:]:
        if parte.startswith(titulo_comeca):
            return parte.split("\n", 1)[1] if "\n" in parte else ""
    return ""


def lista_numerada(corpo: str) -> list[str]:
    itens, atual = [], None
    for l in corpo.splitlines():
        m = re.match(r"^(\d+)\.\s+(.*)", l)
        if m:
            atual = m.group(2)
            itens.append(atual)
        elif atual is not None and l.startswith("   ") and l.strip():
            itens[-1] += " " + l.strip()
        elif not l.strip() or not l.startswith(" "):
            atual = None if not l.strip() else atual
    return itens


def achados(sp: dict) -> dict:
    """SEC, DBA, QA e prova F de 01/10 -- tudo lido das secoes do SPRINTS.md."""
    t = sp["texto"]
    sec = secao_md(t, "Achados de segurança")
    altos_frase = sec.split("\n- ", 1)[0]
    altos = re.findall(r"\b([AMB]\d)(?:–([AMB]?\d))?", altos_frase)
    codigos_altos = []
    for ini, fim in altos:
        if fim:
            letra = ini[0]
            for n in range(int(ini[1:]), int(fim.lstrip(letra)) + 1):
                codigos_altos.append(f"{letra}{n}")
        else:
            codigos_altos.append(ini)
    sec_itens = [l[2:].strip() for l in sec.splitlines() if l.startswith("- ")]
    dba = lista_numerada(secao_md(t, "Parecer do DBA"))
    qa = lista_numerada(secao_md(t, "Inventário do QA"))
    f = [l for l in secao_md(t, "Prova F").splitlines() if l.startswith("- ☐")]
    sp13 = lista_numerada(secao_md(t, "SP000013"))
    precisa = []
    for l in secao_md(t, "SP000014").splitlines():
        cols = [c.strip() for c in l.strip().strip("|").split("|")]
        if len(cols) == 2 and cols[0] not in ("Precisa", "---") and not set(cols[0]) <= {"-"}:
            precisa.append({"precisa": cols[0], "para": cols[1]})
    return {"altos": codigos_altos, "altos_frase": altos_frase.strip(), "sec": sec_itens,
            "dba": dba, "qa": qa, "prova_f": f, "sp13": sp13, "sp14": precisa}


# ---------------------------------------------------------------- ferramentas e equipe

def ferramentas() -> dict:
    arq = RAIZ / "apps/phxclaw-ui/assets/ferramentas.json"
    d = json.loads(arq.read_text(encoding="utf-8"))
    fs = d["ferramentas"]
    grupos: dict[str, list] = {}
    for f in fs:
        grupos.setdefault(f["grupo"], []).append(f)
    caps = sorted({f["capacidade"] for f in fs})
    return {"total": len(fs), "declarado": d.get("total"), "versao": d.get("versao"),
            "concedidas": sum(1 for f in fs if f["concedida"]),
            "negadas": [f for f in fs if not f["concedida"]],
            "grupos": sorted(grupos.items(), key=lambda kv: (-len(kv[1]), kv[0])),
            "capacidades": len(caps), "observacao": d.get("observacao", ""),
            "gerado_por": d.get("gerado_por", ""), "fonte": rel(arq), "data": data_de(arq)}


def equipe() -> dict:
    arq = RAIZ / "apps/phxclaw-ui/assets/equipe.json"
    d = json.loads(arq.read_text(encoding="utf-8"))
    areas = [{"nome": m["nome"], "n": len(m["papeis"]),
              "humanos": sum(1 for p in m["papeis"] if p.get("humano")),
              "criticos": sum(1 for p in m["papeis"] if p.get("criticidade") == "Crítica"),
              "exemplos": [p["nome"] for p in m["papeis"][:3]]} for m in d["macroareas"]]
    papeis = [p for m in d["macroareas"] for p in m["papeis"]]
    return {"areas": sorted(areas, key=lambda a: (-a["n"], a["nome"])), "total": len(papeis),
            "declarado": d.get("total"), "humanos": [p["nome"] for p in papeis if p.get("humano")],
            "integrador": next((p for p in papeis if p["id"] == max(x["id"] for x in papeis)), None),
            "planilha": d.get("fonte", ""), "gerado_por": d.get("gerado_por", ""),
            "fonte": rel(arq), "data": data_de(arq)}


# ---------------------------------------------------------------- organograma de construcao

def papeis_de_construcao() -> dict:
    """As duas tabelas do .claude/agents/README.md: os dez papeis e os acrescimos."""
    arq = AGENTES / "README.md"
    linhas = []
    for l in arq.read_text(encoding="utf-8").splitlines():
        cols = [c.strip() for c in l.strip().strip("|").split("|")]
        if len(cols) != 3 or cols[0] in ("Papel", "Novo") or set(cols[0]) <= {"-"}:
            continue
        m = re.match(r"^([A-J]|SEC|INT|RES-[a-z]+)\b(?:\s*—\s*(.*))?", cols[0])
        if not m:
            continue
        sigla = m.group(1)
        nome = (m.group(2) or "").strip()
        agente = re.sub(r"[`*]", "", cols[1])
        linhas.append({"sigla": sigla, "nome": nome, "agente": agente, "porque": cols[2]})
    # Um papel pode ter dois agentes (H: documentacao e tradutor): junta pela sigla.
    papeis: dict[str, dict] = {}
    for x in linhas:
        if x["sigla"] in papeis:
            papeis[x["sigla"]]["agentes"].append(x["agente"])
        else:
            papeis[x["sigla"]] = {**x, "agentes": [x["agente"]]}
    for sigla, p in papeis.items():
        if not p["nome"]:
            p["nome"] = {"SEC": "Segurança", "INT": "Integrador"}.get(sigla, sigla)
    return {"papeis": papeis, "fonte": rel(arq), "data": data_de(arq)}


def roteiro_do_integrador() -> dict:
    arq = AGENTES / "integrador.md"
    t = arq.read_text(encoding="utf-8")
    passos = []
    for item in lista_numerada(secao_md(t, "O roteiro do Go/NoGo")):
        m = re.match(r"\*\*(.+?)\*\*", item)
        passos.append((m.group(1) if m else item).rstrip("."))
    conselho = []
    for l in secao_md(t, "Vários integradores").splitlines():
        cols = [c.strip() for c in l.strip().strip("|").split("|")]
        if len(cols) == 2 and not set(cols[0]) <= {"-"} and cols[0] != "pareceres registrados":
            conselho.append({"se": cols[0], "entao": re.sub(r"\*\*", "", cols[1])})
    if not passos or not conselho:
        raise SystemExit("PARADA: o roteiro ou a tabela do conselho sumiu do integrador.md")
    return {"passos": passos, "conselho": conselho, "fonte": rel(arq), "data": data_de(arq)}


# ---------------------------------------------------------------- o portao unico

# Rotulo de cada passo do `call_tool_com`, pela chamada que o faz. A ORDEM sai do codigo; esta
# tabela so da nome. Passo no codigo sem rotulo aqui, ou rotulo sem passo no codigo, PARA.
PASSOS_DO_PORTAO = [
    ("caps.contains(", "capacidade", "Capacidade concedida", "ferramenta fora da política é negada, mesmo pedida pelo nome"),
    ("crate::esquema::validar(", "esquema", "Esquema do argumento", "argumento inválido volta ao modelo com todos os erros e o caminho de cada campo"),
    ("self.veredito_de_comando(", "regras", "Regras de comando", "permitir, negar ou perguntar, para toda ferramenta que cria processo"),
    ("crate::segredos::recusa_no_shell(", "segredos", "Segredo: o shell não grava no git", "com a varredura exigida, só o git_write grava história — e ele passa pelo gitleaks"),
    ("crate::hooks::Evento::AntesDaFerramenta", "hooks", "Hook PreToolUse", "saída de bloqueio recusa a chamada, com o motivo"),
    ("crate::checkpoint::no_portao(", "checkpoint", "Ponto de restauração", "antes de toda escrita, depois das regras e do hook"),
]
EXECUTA = "t.run("


def portao_unico() -> dict:
    arq = RAIZ / "crates/phxclaw-agent/src/motor.rs"
    t = arq.read_text(encoding="utf-8")
    ini = t.index("async fn call_tool_com(")
    corpo = t[ini: t.index("let (mut texto, outcome", ini)]
    usados = {m for m in re.findall(r"crate::(\w+)::", corpo)}
    conhecidos = {p[1] for p in PASSOS_DO_PORTAO}
    sem_rotulo = sorted(usados - conhecidos)
    if sem_rotulo:
        raise SystemExit(f"PARADA: o call_tool_com chama modulo(s) sem rotulo no dossie: {sem_rotulo}")
    achados_ = []
    for chamada, _mod, nome, efeito in PASSOS_DO_PORTAO:
        pos = corpo.find(chamada)
        if pos < 0:
            raise SystemExit(f"PARADA: rotulo sem passo no codigo: «{nome}» ({chamada}) nao esta no call_tool_com")
        achados_.append((pos, nome, efeito))
    exe = corpo.find(EXECUTA)
    if exe < 0:
        raise SystemExit("PARADA: o call_tool_com nao chama mais a ferramenta por t.run(")
    depois = [n for p, n, _e in achados_ if p > exe]
    if depois:
        raise SystemExit(f"PARADA: passo(s) do portao DEPOIS de rodar a ferramenta: {depois}")
    passos = [{"nome": n, "efeito": e} for _p, n, e in sorted(achados_)]
    linha = t[:ini].count("\n") + 1
    return {"passos": passos, "linha": linha, "fonte": rel(arq), "data": data_de(arq)}


def fora_do_bwrap() -> dict:
    arq = RAIZ / "crates/phxclaw-agent/tests/guardas.rs"
    t = arq.read_text(encoding="utf-8")
    bloco = t.split("const FORA_DO_BWRAP", 1)[1].split("];", 1)[0]
    entradas = []
    for m in re.finditer(r'\(\s*"([^"]+)",\s*"((?:[^"\\]|\\.)*)",\s*"((?:[^"\\]|\\\n|\\.)*)",?\s*\)', bloco, re.S):
        motivo = re.sub(r"\\\n", "", m.group(3)).replace('\\"', '"')
        entradas.append({"arquivo": m.group(1), "trecho": m.group(2).replace('\\"', '"'),
                         "motivo": motivo.split(". ")[0].rstrip(".") + "."})
    testes = re.findall(r"#\[(?:tokio::)?test\]\s*(?:async\s+)?fn\s+(\w+)", t)
    return {"excecoes": entradas, "guardas": testes, "fonte": rel(arq), "data": data_de(arq)}


def broker() -> dict:
    arq = RAIZ / "crates/phxclaw-secret-broker/src/lib.rs"
    notas = []
    for l in arq.read_text(encoding="utf-8").splitlines():
        m = re.match(r"//!\s+-\s+(.*)", l)
        if m:
            notas.append(m.group(1))
        elif notas and re.match(r"//!\s{3,}\S", l):
            notas[-1] += " " + l[3:].strip()
    return {"notas": notas, "fonte": rel(arq), "data": data_de(arq)}


def gitleaks() -> dict:
    arq = RAIZ / "crates/phxclaw-agent/src/segredos.rs"
    t = arq.read_text(encoding="utf-8")
    decisoes = re.findall(r"^//! - \*\*(.+?)\*\*", t, re.M)
    return {"decisoes": decisoes, "fonte": rel(arq), "data": data_de(arq)}


# ---------------------------------------------------------------- interface

def tokens_da_marca() -> dict:
    arq = RAIZ / "apps/phxclaw-ui/assets/app.css"
    css = arq.read_text(encoding="utf-8")
    sem_coment = re.sub(r"/\*.*?\*/", "", css, flags=re.S)
    escuro = re.search(r"^:root\{(.*?)\n\}", sem_coment, re.S | re.M).group(1)
    claro = re.search(r'^:root\[data-tema="claro"\]\{(.*?)\n\}', sem_coment, re.S | re.M).group(1)
    par = lambda b: dict(re.findall(r"(--[a-z0-9-]+)\s*:\s*([^;]+);", b))
    return {"escuro": par(escuro), "claro": par(claro), "fonte": rel(arq), "data": data_de(arq)}


def qualificacao() -> dict:
    """O veredito mais novo (qualificar.json) e o relatorio de partida (RELATORIO_*.md)."""
    rel_md = sorted((RAIZ / "docs/ui/qualificacao").glob("RELATORIO_*.md"))
    partida = None
    if rel_md:
        t = rel_md[-1].read_text(encoding="utf-8")
        corpo = secao_md(t, "Veredito por tela")
        vered = re.findall(r"^- (.+?) — (NÃO QUALIFICADA|QUALIFICADA COM RESSALVAS|QUALIFICADA)\b", corpo, re.M)
        cont: dict[str, int] = {}
        for _tela, v in vered:
            cont[v] = cont.get(v, 0) + 1
        sev = {}
        for nome in ("Bloqueia", "Grave", "Médio", "Cosmético"):
            sev[nome] = len(re.findall(r"^[A-Z]\d+ ·", secao_md("\n" + t.split("## Achados", 1)[1].replace("\n### ", "\n## "), nome), re.M))
        data = re.search(r"(\d{4}-\d{2}-\d{2})", rel_md[-1].name).group(1)
        partida = {"telas": len(vered), "contagem": cont, "severidade": sev, "fonte": rel(rel_md[-1]),
                   "data": data_declarada(data, "%Y-%m-%d")}
    arq = RAIZ / "tests/desktop/out/qualificacao/qualificar.json"
    atual = None
    if arq.exists():
        d = json.loads(arq.read_text(encoding="utf-8"))
        por_tema = {}
        for tema, telas in d["veredito"].items():
            c: dict[str, int] = {}
            for v in telas.values():
                c[v["veredito"]] = c.get(v["veredito"], 0) + 1
            por_tema[tema] = {"telas": len(telas), "contagem": c,
                              "qualificadas": c.get("QUALIFICADA", 0)}
        sondas = d.get("achados", [])
        atual = {"por_tema": por_tema, "sondas": len(sondas),
                 "sondas_ok": sum(1 for s in sondas if s.get("ok")), "fonte": rel(arq), "data": data_de(arq)}
    return {"partida": partida, "atual": atual,
            "comando": "PLAYWRIGHT_BROWSERS_PATH=/opt/pw-browsers node tests/desktop/qualificacao/qualificar.mjs"}


def fidelidade() -> list[dict]:
    """Cada JSON de docs/ui/fidelidade: o comando, a data gravada nele e o resumo que ele tem."""
    out = []
    for arq in sorted((RAIZ / "docs/ui/fidelidade").glob("*.json")):
        d = json.loads(arq.read_text(encoding="utf-8"))
        data = data_declarada(d["data"], "%Y-%m-%d %H:%M") if "data" in d else data_de(arq)
        resumo = []
        if "so_ocr" in d:
            tt = d["so_ocr"]["agregado"]["totais"]
            resumo.append(f"só OCR, {d['so_ocr']['telas_n']} telas: {tt['achados']} de {tt['campos_na_origem']} campos achados, "
                          f"{tt['inventados']} inventados, {tt['perdidos']} perdidos")
            cm = d.get("com_modelo", {})
            if "agregado" in cm:
                t2 = cm["agregado"]["totais"]
                resumo.append(f"com modelo ({cm.get('modelo', '?')}), {cm['telas_n']} telas: {t2['achados']} de "
                              f"{t2['campos_na_origem']} campos, {t2['inventados']} inventados, {t2['perdidos']} perdidos")
            elif "nao_medido" in cm:
                resumo.append(f"com modelo: NÃO MEDIDO ({cm['nao_medido']})")
        if "casos" in d:
            c = d["casos"]
            larg = d.get("por_largura", [])
            rol = sum(x.get("telas_com_rolagem", 0) for x in larg)
            # A regra do alvo de toque e «>= 44 px a 390»: so a largura 390 entra nesta conta.
            a390 = next((x for x in larg if x.get("largura") == 390), None)
            peq = "não medido a 390" if a390 is None else f"{a390.get('alvos_menores_que_44', 0)} de {a390.get('alvos', 0)}"
            resumo.append(f"alvo {d.get('alvo')}: {len(d.get('telas', []))} medições em {len(larg)} larguras; "
                          f"fronteiras de contêiner {c['fronteiras_de_conteiner']['conferidas']} conferidas, "
                          f"{c['fronteiras_de_conteiner']['divergentes']} divergentes; telas com rolagem lateral {rol}; "
                          f"alvos de toque menores que 44 px a 390: {peq}")
        if "passou" in d and "total" in d:
            resumo.append(f"{d['passou']} de {d['total']} verificações passaram")
        out.append({"arquivo": arq.name, "fonte": rel(arq), "comando": d.get("comando", "—"),
                    "data": data, "resumo": resumo or ["(formato sem resumo conhecido)"]})
    return out


# ---------------------------------------------------------------- testes e bancadas

def certificacao() -> dict | None:
    arqs = sorted((RAIZ / "reports").glob("RELEASE_CERTIFICATION_v*.json"))
    if not arqs:
        return None
    arq = arqs[-1]
    c = json.loads(arq.read_text(encoding="utf-8"))
    d = datetime.fromisoformat(c["certified_at"]).astimezone(timezone.utc)
    obrig = [g for g in c["gates"] if g["required"]]
    # Quantos commits a arvore andou depois da certificacao: o numero envelhece, e a pagina diz quanto.
    depois = int(sh(["git", "rev-list", "--count", f"--since={c['certified_at']}", "HEAD", "--", "."]).strip() or 0)
    return {"versao": c["version"], "alvo": c.get("target", ""), "veredito": c["verdict"],
            "portoes": c["gates"], "obrig": len(obrig),
            "obrig_ok": sum(1 for g in obrig if g["status"] == "passed"),
            "data": {"quando": d, "texto": f"{fmt_data(d)} UTC", "como": "data gravada no proprio resultado"},
            "commits_depois": depois, "fonte": rel(arq),
            "comando": "python3 tools/release_certification.py"}


def medidores() -> list[dict]:
    """Os roteiros de tests/desktop (os que tem linha «Uso:»), e o arquivo de resultado de cada um.

    A lista sai das pastas, nao de uma tabela: roteiro novo entra sozinho. Quem grava resultado
    e lido pelo `writeFileSync(join(OUT, '...json'))` do proprio roteiro; quem nao grava aparece
    como NAO MEDIDO, com o comando -- o placar dele so existe na saida de quem rodou.
    """
    base = RAIZ / "tests/desktop"
    out_dir = base / "out"
    lista = []
    for arq in sorted(list(base.glob("*.mjs")) + list(base.glob("*.py")) + list((base / "qualificacao").glob("*.mjs"))):
        t = arq.read_text(encoding="utf-8")
        uso = re.search(r"Uso:\s*(.+)", t)
        if not uso:
            continue
        comando = uso.group(1).split(";")[0].strip()
        # A regua casa o NOME do arquivo .json gravado, nao a forma da chamada: o ui_paineis.mjs
        # grava por `join(SAIDA, ...)` e ficou NAO MEDIDO com o arquivo existindo (QA, 02/10/2026).
        # Roteiro Python grava por `json.dump` em `open(... .json)`; entra pelo mesmo casador.
        saidas = re.findall(r"writeFileSync\(join\(\w+,\s*'([^']+\.json)'\)", t)
        saidas += re.findall(r"open\([^)]*['\"]([A-Za-z0-9_\-]+\.json)['\"][^)]*['\"]w['\"]", t)
        saidas = [s for s in dict.fromkeys(saidas) if "parcial" not in s]
        resultado = None
        for s in saidas:
            p = (out_dir / "qualificacao" / s) if "qualificacao" in arq.parts else (out_dir / s)
            if p.exists():
                resultado = {"arquivo": rel(p), "data": data_de(p)}
        lista.append({"roteiro": rel(arq), "comando": comando, "resultado": resultado,
                      "grava": saidas})
    return lista


MARCA_PULADOS = "=== PULADOS (target/tmp/pulados.jsonl)"



# O teste que prova o fechamento de cada achado alto de SEC (01/10). A lista e conferida contra o
# fonte: nome que nao existir mais PARA a corrida, para a pagina nunca declarar fechado por um
# teste que foi renomeado.
PROVAS_SEC = {
    "A1": ["arquivo_que_o_git_chama_de_binario_nao_escapa"],
    "A2": ["stash_e_varrido_e_ramo_so_de_nome"],
    "A3": ["conselho_vazio_no_arquivo_nunca_da_go", "nogo_com_erros_em_branco_e_recusado_sem_gravar"],
    "M1": ["diff_relative_nao_esconde_arquivo_fora_da_subpasta"],
    "M2": ["mensagem_do_commit_e_varrida"],
    "B1": ["add_recusado_nao_deixa_objeto"],
}


def fechamento_sec(suite_arquivo: Path | None) -> dict:
    """Por achado alto: os testes que o provam e o que a saida GUARDADA da suite diz de cada um
    (ok, falhou, nao rodou). Sem arquivo da suite, tudo e NAO MEDIDO."""
    fontes = list((RAIZ / "crates/phxclaw-agent").rglob("*.rs"))
    codigo = "\n".join(f.read_text(errors="replace") for f in fontes)
    texto = suite_arquivo.read_text(errors="replace") if suite_arquivo else ""
    saida = {}
    for cod, testes in PROVAS_SEC.items():
        linhas = []
        for t in testes:
            if f"fn {t}(" not in codigo:
                raise SystemExit(f"fechamento SEC {cod}: o teste {t} nao existe mais no fonte -- atualize PROVAS_SEC")
            m = re.search(rf"^test (?:\S+::)?{re.escape(t)} \.\.\. (ok|FAILED|ignored)$", texto, re.M)
            linhas.append({"teste": t, "estado": m.group(1) if m else "nao rodou"})
        saida[cod] = {"testes": linhas, "fechado": all(l["estado"] == "ok" for l in linhas)}
    return saida

def suite(arquivo: Path | None) -> dict | None:
    """Placar do `cargo test` a partir de uma saida GUARDADA (--suite ARQ); sem ela, None."""
    if arquivo is None:
        return None
    texto = arquivo.read_text(errors="replace")
    r = [tuple(map(int, m)) for m in re.findall(
        r"test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored", texto)]
    # O arquivo nao diz se a corrida foi do workspace inteiro: conta os crates que tiveram os
    # proprios unitarios rodados e compara com os membros. Menos que todos e PARCIAL, e diz.
    crates = set(re.findall(r"Running unittests \S+ \(target/\S+/deps/([A-Za-z0-9_]+)-[0-9a-f]+\)", texto))
    membros = versao()["membros"]
    # Teste que pulou sai "ok" do libtest. Os pulos REGISTRADOS (tests/comum/pulado.rs) so
    # valem se vierem da MESMA corrida: a rotina da suite apaga o registro antes e o anexa ao
    # proprio arquivo depois do marcador abaixo. O registro solto em target/tmp acumula
    # corridas e nao se le -- sem o bloco, os pulos registrados desta suite sao NAO MEDIDOS.
    pulados = None
    if MARCA_PULADOS in texto:
        vistos = set()
        for lin in texto.split(MARCA_PULADOS, 1)[1].splitlines():
            try:
                d = json.loads(lin)
            except ValueError:
                continue
            vistos.add((d.get("crate"), d.get("teste")))
        pulados = len(vistos)
    # Pulo que so IMPRIME (sem o registro) nao se conta pela saida guardada: o libtest captura
    # o eprintln de quem passa. Conta-se o LUGAR no codigo -- texto com «pulado/pulada» em
    # qualquer caixa dentro de uma string --, para a pagina dizer quantos ainda podem ter
    # pulado calados (migracao na SP000013).
    calados = []
    pulo = re.compile(r'"[^"]*\bpulad[oa]', re.I)
    for arq in sorted(list(REPO.glob("phxclaw/crates/*/tests/*.rs")) + list(REPO.glob("phxclaw/apps/*/tests/*.rs"))
                      + list(REPO.glob("phxclaw/crates/*/src/**/tests.rs"))):
        # O crate de apoio e a casa do modulo e da guarda: os testes dele PLANTAM o texto
        # que a regra reprova (prova nos dois sentidos), e por isso fica fora da conta.
        if "phxclaw-test-support" in arq.parts:
            continue
        for n, lin in enumerate(arq.read_text(errors="replace").splitlines(), 1):
            if lin.lstrip().startswith("//") or "pulado::pular" in lin:
                continue
            if pulo.search(lin):
                calados.append(f"{rel(arq)}:{n}")
    return {"passam": sum(x[0] for x in r), "falham": sum(x[1] for x in r),
            "ignorados": sum(x[2] for x in r), "pulados": pulados, "calados": calados, "crates": len(crates),
            "membros": membros, "inteira": len(crates) >= membros,
            "fonte": str(arquivo), "data": data_de(arquivo)}


# ---------------------------------------------------------------- petreas

def petreas_gerais() -> dict:
    """As clausulas petreas que valem para TODO projeto: os titulos «Cláusula pétrea:» do CLAUDE.md."""
    arq = REPO / "CLAUDE.md"
    t = arq.read_text(encoding="utf-8")
    titulos = re.findall(r"^## Cláusula pétrea: (.+)$", t, re.M)
    return {"titulos": titulos, "fonte": rel(arq), "data": data_de(arq)}


def portoes_de_sprint(sp: dict) -> list[str]:
    m = re.search(r"Portões de toda sprint \(não se repetem abaixo\):\s*(.+?)(?:\n\n|\n---)", sp["texto"], re.S)
    if not m:
        return []
    frase = " ".join(m.group(1).split())
    return [p.strip().rstrip(".") for p in re.split(r",\s*(?![^()]*\))", frase) if p.strip()]
