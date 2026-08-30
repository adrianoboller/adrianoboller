#!/usr/bin/env python3
"""Extrai a base de conhecimento do transcrito da sessao.

Existe por um motivo simples: uma base montada a mao e uma base que ninguem
consegue refazer quando a sessao seguinte acrescentar mais coisa. O transcrito
tem 99 MB e 29 mil linhas -- ninguem le isso, e ninguem deveria precisar.

    python3 base-de-conhecimento/extrair.py [caminho-do-jsonl]

Sem argumento, procura o transcrito desta sessao. Grava tudo em
`base-de-conhecimento/`, sobrescrevendo o que gerou antes.
"""
import json, os, re, sys, hashlib
from datetime import datetime

RAIZ = os.path.dirname(os.path.abspath(__file__))
PADRAO = "/root/.claude/projects/-home-user-adrianoboller"


def achar_transcrito():
    if len(sys.argv) > 1:
        return sys.argv[1]
    if os.path.isdir(PADRAO):
        alvos = [os.path.join(PADRAO, f) for f in os.listdir(PADRAO) if f.endswith(".jsonl")]
        if alvos:
            return max(alvos, key=os.path.getsize)
    sys.exit("nao achei o transcrito; passe o caminho como argumento")


# Ruido que nao e conversa: lembrete do sistema, resposta de hook, eco de
# ferramenta. Sem esse crivo, os pedidos do dono somem no meio do maquinario.
RUIDO = re.compile(
    r"<system-reminder>|<command-name>|Stop hook feedback|"
    r"Caveat: The messages below|\[SYSTEM NOTIFICATION|<task-notification>|"
    r"^\s*$", re.S)


def limpo(t):
    if not isinstance(t, str) or not t.strip():
        return None
    if RUIDO.search(t):
        # Um pedido de verdade pode vir COM um lembrete colado; guarda o que
        # sobra depois de tirar os blocos, e descarta se so havia ruido.
        t = re.sub(r"<system-reminder>.*?</system-reminder>", "", t, flags=re.S)
        t = re.sub(r"<task-notification>.*?</task-notification>", "", t, flags=re.S)
        t = re.sub(r"<command-name>.*?</command-message>", "", t, flags=re.S)
        for marca in ("Stop hook feedback", "[SYSTEM NOTIFICATION", "Caveat: The messages"):
            if t.strip().startswith(marca):
                return None
        if not t.strip():
            return None
    return t.strip()


def quando(d):
    ts = d.get("timestamp") or ""
    try:
        return datetime.fromisoformat(ts.replace("Z", "+00:00")).strftime("%d/%m %H:%M")
    except Exception:
        return ""


def main():
    T = achar_transcrito()
    pedidos, agentes, bash, escritas = [], [], [], []
    with open(T, encoding="utf-8", errors="replace") as f:
        for ln in f:
            ln = ln.strip()
            if not ln:
                continue
            try:
                d = json.loads(ln)
            except Exception:
                continue
            m = d.get("message") or {}
            c = m.get("content")
            if d.get("type") == "user" and isinstance(c, str):
                t = limpo(c)
                if t:
                    pedidos.append((quando(d), t))
            if isinstance(c, list):
                for b in c:
                    if not isinstance(b, dict) or b.get("type") != "tool_use":
                        continue
                    e = b.get("input") or {}
                    n = b.get("name")
                    if n == "Agent":
                        agentes.append((quando(d), e.get("description", ""), e.get("prompt", "")))
                    elif n == "Bash":
                        bash.append((quando(d), e.get("description", ""), e.get("command", "")))
                    elif n == "Write":
                        escritas.append((e.get("file_path", ""), e.get("content", "")))

    os.makedirs(os.path.join(RAIZ, "scripts"), exist_ok=True)
    esc = lambda p: os.path.join(RAIZ, p)

    # 1. Os pedidos, em ordem. E a historia do requisito, e o unico lugar onde
    #    ela existe inteira -- a lista de pendencias guarda o QUE ficou, nao o
    #    que foi pedido nem em que ordem.
    with open(esc("01-PEDIDOS.md"), "w", encoding="utf-8") as o:
        o.write("# Os pedidos, na ordem em que chegaram\n\n")
        o.write(f"{len(pedidos)} mensagens. Extraidas de `{os.path.basename(T)}`.\n\n---\n\n")
        for i, (t, p) in enumerate(pedidos, 1):
            o.write(f"## {i}. {t}\n\n{p}\n\n---\n\n")

    # 2. Os briefings de agente. Sao o ativo mais reaproveitavel: cada um leva
    #    a regra da casa junto do pedido, e e isso que faz o agente nao repetir
    #    erro que o projeto ja pagou.
    with open(esc("02-BRIEFINGS-DE-AGENTE.md"), "w", encoding="utf-8") as o:
        o.write("# Briefings de agente\n\n")
        o.write("O ativo mais reaproveitavel desta base. Cada briefing leva a\n")
        o.write("REGRA junto do pedido -- e por isso que o agente nao repete o erro\n")
        o.write("que o projeto ja pagou. Copie o formato, troque o assunto.\n\n")
        o.write(f"{len(agentes)} despachos.\n\n---\n\n")
        for i, (t, desc, pr) in enumerate(agentes, 1):
            o.write(f"## {i}. {desc}  ·  {t}\n\n```\n{pr}\n```\n\n---\n\n")

    # 3. Os scripts em Python que rodaram por heredoc. Sem isto eles morrem com
    #    a sessao, e eram medicao, extracao e conserto em massa.
    vistos, salvos = set(), 0
    pat = re.compile(r"python3\s*-\s*<<\s*'?PY'?\n(.*?)\nPY", re.S)
    for tempo, desc, cmd in bash:
        for corpo in pat.findall(cmd):
            if len(corpo) < 120:
                continue
            h = hashlib.sha256(corpo.encode()).hexdigest()[:10]
            if h in vistos:
                continue
            vistos.add(h)
            salvos += 1
            nome = re.sub(r"[^a-z0-9]+", "-", (desc or "script").lower()).strip("-")[:48]
            with open(esc(f"scripts/{salvos:03d}-{nome or 'script'}.py"), "w", encoding="utf-8") as o:
                o.write(f"# {desc}\n# {tempo}\n\n{corpo}\n")

    # 4. Os comandos de shell, sem repeticao. Interessa a RECEITA (medir, provar,
    #    limpar), nao o `ls` de cada minuto.
    vistos_b, receitas = set(), []
    for tempo, desc, cmd in bash:
        if len(cmd) < 80 or "python3 - <<" in cmd:
            continue
        h = hashlib.sha256(cmd.encode()).hexdigest()[:10]
        if h in vistos_b:
            continue
        vistos_b.add(h)
        receitas.append((tempo, desc, cmd))
    with open(esc("03-RECEITAS-DE-SHELL.md"), "w", encoding="utf-8") as o:
        o.write("# Receitas de shell\n\n")
        o.write(f"{len(receitas)} comandos distintos, sem os triviais.\n\n---\n\n")
        for tempo, desc, cmd in receitas:
            o.write(f"### {desc}  ·  {tempo}\n\n```bash\n{cmd}\n```\n\n")

    # 5. Classificar os scripts. Sem isto, mil e trezentos arquivos escondem os
    #    vinte que valem: a maioria e conserto de uma vez so, e o que se
    #    reaproveita e a MEDICAO e a PROVA.
    def classe(txt):
        t = txt.lower()
        if "time.time()" in t or "perf_counter" in t or "mediana" in t or " ms/" in t:
            return "medicao"
        if "defeito reposto" in t or ("assert" in t and "reprov" in t):
            return "prova"
        if "socket" in t and "json" in t:
            return "sonda-de-protocolo"
        if "re.sub" in t or "replace(" in t:
            return "edicao-em-massa"
        if "os.walk" in t or "glob" in t or "listdir" in t:
            return "varredura"
        return "outro"

    por_classe = {}
    for arq in sorted(os.listdir(esc("scripts"))):
        corpo = open(esc(f"scripts/{arq}"), encoding="utf-8").read()
        por_classe.setdefault(classe(corpo), []).append((arq, corpo.splitlines()[0].lstrip("# ")))
    ordem = ["medicao", "prova", "sonda-de-protocolo", "varredura", "edicao-em-massa", "outro"]
    titulo = {
        "medicao": "Medicao -- cronometram, contam, comparam",
        "prova": "Prova real -- repoem o defeito e conferem que reprova",
        "sonda-de-protocolo": "Sondas de protocolo -- falam com o servidor por soquete",
        "varredura": "Varredura -- percorrem arvore de arquivos",
        "edicao-em-massa": "Edicao em massa -- mexem no fonte por padrao",
        "outro": "Os demais",
    }
    with open(esc("scripts/00-INDICE.md"), "w", encoding="utf-8") as o:
        o.write("# Os scripts, por tecnica\n\n")
        o.write("As tres primeiras familias sao as que se reaproveitam em outro\n")
        o.write("projeto. `edicao-em-massa` e conserto de uma vez so -- guardado\n")
        o.write("pelo padrao, nao pelo conteudo.\n\n")
        for c in ordem:
            itens = por_classe.get(c, [])
            if not itens:
                continue
            o.write(f"## {titulo[c]}  ({len(itens)})\n\n")
            for arq, desc in itens:
                o.write(f"- `{arq}` -- {desc}\n")
            o.write("\n")

    with open(esc("00-INDICE.md"), "w", encoding="utf-8") as o:
        o.write(f"""# Base de conhecimento -- PhxSql

Gerada por `extrair.py` do transcrito da sessao. **Nao se edita a mao**: rode
o extrator de novo e ela se refaz.

| Arquivo | O que tem |
|---|---|
| `01-PEDIDOS.md` | {len(pedidos)} pedidos do dono, em ordem |
| `02-BRIEFINGS-DE-AGENTE.md` | {len(agentes)} briefings de agente |
| `03-RECEITAS-DE-SHELL.md` | {len(receitas)} comandos distintos |
| `scripts/` | {salvos} scripts Python, classificados em `scripts/00-INDICE.md` |
| `04-LICOES.md` | as licoes, escritas a mao (esta SIM se edita) |

Gerada em {datetime.now().strftime('%d/%m/%Y %H:%M')} de
`{os.path.basename(T)}` ({os.path.getsize(T) // (1024*1024)} MB,
{sum(1 for _ in open(T, encoding='utf-8', errors='replace'))} linhas).
""")
    print(f"pedidos:  {len(pedidos)}")
    print(f"briefings:{len(agentes)}")
    print(f"receitas: {len(receitas)}")
    print(f"scripts:  {salvos}")


if __name__ == "__main__":
    main()
