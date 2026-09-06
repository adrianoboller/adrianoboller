#!/usr/bin/env python3
"""Gera o fluxograma do plugin -- desenho e números saindo do repositório.

Por que virou gerador: esta era a última página mantida à mão, e ficou seis
lançamentos atrás. Dizia «19 comandos» quando havia 29, e não mostrava nenhum
dos portões novos. É exatamente o defeito que este projeto já teve quatro vezes,
e a regra que saiu dele: número visível sai de gerador, ou envelhece calado.

O que o desenho tenta mostrar não é o nome das peças -- é o MECANISMO: onde cada
portão nega passagem, e para onde a coisa volta quando ele nega. Portão que só
aparece como caixa no meio da linha não explica nada; a seta de volta explica.

Uso: python3 docs/dossie/gerar-fluxo.py [saida.html]
"""
from __future__ import annotations

import html as H
import json
import re
import subprocess
import sys
from datetime import date
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
E = H.escape

# ---------------------------------------------------------------- medições

def medir() -> dict:
    versao = json.loads((RAIZ / ".claude-plugin/plugin.json").read_text(encoding="utf-8"))["version"]
    hooks = json.loads((RAIZ / "hooks/hooks.json").read_text(encoding="utf-8"))["hooks"]
    testes = len(re.findall(r"^\s+def test_", (RAIZ / "tests/testes.py").read_text(encoding="utf-8"), re.M))
    cen = len(re.findall(r'^\s+\("\d+ ', (RAIZ / "tests/cenarios.py").read_text(encoding="utf-8"), re.M))
    passos = len(re.findall(r'passo\(', (RAIZ / "tests/fluxo.py").read_text(encoding="utf-8")))
    perguntas = subprocess.run(
        [sys.executable, str(RAIZ / "skills/conversao-wx/scripts/listar_perguntas.py"), "--json"],
        capture_output=True, text=True)
    return {
        "versao": versao,
        "comandos": len(list((RAIZ / "commands").glob("*.md"))),
        "scripts": len(list((RAIZ / "skills/conversao-wx/scripts").glob("*.py"))),
        "agentes": len(list((RAIZ / "agents").glob("*.md"))),
        "skills": len([p for p in (RAIZ / "skills").iterdir() if p.is_dir()]),
        "hooks": {evento: len(v) for evento, v in hooks.items()},
        "hooks_total": sum(len(v) for v in hooks.values()),
        "testes": testes,
        "cenarios": cen,
        "passos_do_fluxo": passos or 13,
        "perguntas": len(json.loads(perguntas.stdout)) if perguntas.returncode == 0 else 0,
    }


# ---------------------------------------------------------------- o desenho

L = 1360  # largura do viewBox
COR = {"entrada": "var(--az)", "portao": "var(--a)", "prova": "var(--vd)",
       "governo": "var(--am)", "hook": "var(--m)"}


def caixa(x, y, w, h, titulo, sub="", cor="currentColor", tracejada=False) -> str:
    traco = ' stroke-dasharray="5 4"' if tracejada else ""
    s = [f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="7" fill="none" stroke="{cor}" stroke-width="1.5"{traco}/>',
         f'<text x="{x + w / 2}" y="{y + (20 if sub else h / 2 + 4)}" text-anchor="middle" '
         f'fill="currentColor" font-size="12.5" font-weight="600">{E(titulo)}</text>']
    if sub:
        for i, linha in enumerate(sub.split("\n")):
            s.append(f'<text x="{x + w / 2}" y="{y + 37 + i * 13}" text-anchor="middle" '
                     f'fill="var(--m)" font-size="10.5">{E(linha)}</text>')
    return "".join(s)


def seta(x1, y1, x2, y2, rotulo="", cor="currentColor", tracejada=False, desvio=0) -> str:
    traco = ' stroke-dasharray="5 4"' if tracejada else ""
    if desvio:
        d = (f"M{x1} {y1} C {x1 + desvio} {y1}, {x2 - desvio} {y2}, {x2} {y2}")
    else:
        d = f"M{x1} {y1} L{x2} {y2}"
    s = [f'<path d="{d}" fill="none" stroke="{cor}" stroke-width="1.4"{traco} marker-end="url(#pf)"/>']
    if rotulo:
        mx, my = (x1 + x2) / 2, (y1 + y2) / 2
        s.append(f'<text x="{mx}" y="{my - 5}" text-anchor="middle" fill="var(--m)" '
                 f'font-size="10">{E(rotulo)}</text>')
    return "".join(s)


def faixa(y, titulo) -> str:
    """O rotulo da faixa ganha fundo porque as setas verticais cruzam por cima.

    Sem isto, "2 · O PRIMEIRO PORTAO" aparecia com uma seta atravessando a
    palavra. Fundo resolve todos os cruzamentos de uma vez, e continua certo
    quando o desenho mudar -- reposicionar seta a seta nao continuaria.
    """
    # 6,6 px por caractere MAIS o letter-spacing de 1,4: sem somar o espaçamento
    # o fundo ficava curto e a última palavra continuava cruzada pela seta
    larg = len(titulo) * 8.0 + 12
    return (f'<rect x="10" y="{y - 11}" width="{larg}" height="15" fill="var(--p)"/>'
            f'<text x="14" y="{y}" fill="var(--m)" font-size="11" letter-spacing="1.4">{E(titulo)}</text>')


def desenho(m: dict) -> str:
    """As setas de RETORNO contornam por fora; cruzar caixa deixa a linha ilegível.

    Isto foi visto abrindo a página no navegador, não lendo o código: a curva de
    volta do G0 passava por cima da própria caixa do G0, e a linha vermelha do
    reprovado atravessava três caixas com o rótulo em cima de uma delas. É a
    mesma lição do CSS global -- componente novo se abre e se olha.
    """
    p, rotulos = [], []
    p.append('<defs><marker id="pf" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" '
             'markerHeight="7" orient="auto-start-reverse">'
             '<path d="M0 0 L10 5 L0 10 z" fill="currentColor"/></marker></defs>')

    # 1 · ENTRADA -------------------------------------------------------
    rotulos.append(faixa(24, "1 · O QUE O CLIENTE ENTREGA, E O QUE VIRA CONTEXTO"))
    p.append(caixa(14, 36, 200, 62, "anexos do legado",
                   "SQL, PDF, código-fonte,\nscreenshots, artefatos", COR["entrada"]))
    p.append(seta(214, 67, 268, 67, "lê"))
    p.append(caixa(268, 36, 210, 62, "questionário",
                   f"{m['perguntas']} perguntas com id\nbloco 0 e letras A–M", COR["entrada"]))
    p.append(seta(478, 67, 532, 67, "aplica"))
    p.append(caixa(532, 36, 220, 62, "contexto do projeto",
                   "CLAUDE.md, INDEX_FILES,\nrespostas por id, esqueleto", COR["entrada"]))
    p.append(seta(752, 67, 806, 67))
    p.append(caixa(806, 36, 200, 62, "contrato ativo",
                   "o que vale hoje;\no superado fica no histórico", COR["governo"]))
    p.append(seta(1006, 67, 1030, 67))
    p.append(caixa(1030, 36, 316, 62, "restrições do projeto",
                   "id, origem, severidade e validador\n(sem validador = INCONCLUSIVA)", COR["governo"]))

    # 2 · PORTÃO DE ENTRADA --------------------------------------------
    rotulos.append(faixa(148, "2 · O PRIMEIRO PORTÃO — E O QUE ELE FAZ QUANDO NEGA"))
    p.append(caixa(268, 162, 210, 66, "G0 · pré-flight",
                   "inventário com hash\nREADY · CONDITIONAL · BLOCKED", COR["portao"]))
    p.append(seta(373, 98, 373, 162, "evidências"))
    p.append(caixa(548, 162, 240, 66, "hook portão G0",
                   "BLOCKED nega escrita de código\nfora de .wx-migration", COR["hook"], True))
    p.append(seta(478, 195, 548, 195, "nega", COR["portao"]))
    # retorno POR FORA, contornando à direita e por baixo das duas caixas
    p.append(f'<path d="M668 228 V252 H240 V195 H{264}" fill="none" stroke="{COR["portao"]}" '
             'stroke-width="1.4" stroke-dasharray="5 4" marker-end="url(#pf)"/>')
    p.append(f'<text x="470" y="266" text-anchor="middle" fill="{COR["portao"]}" '
             'font-size="10">bloqueado: volta para as evidências, e nenhum código é escrito</text>')

    # 3 · CONVERSÃO -----------------------------------------------------
    rotulos.append(faixa(300, "3 · CONVERSÃO, GATE A GATE, COM APROVAÇÃO HUMANA"))
    gates = [("G1", "inventário"), ("G2", "arquitetura"), ("G3", "dados"), ("G4", "piloto"),
             ("G5", "ondas"), ("G6", "homologação"), ("G7", "virada")]
    x = 14
    for i, (g, oq) in enumerate(gates):
        p.append(caixa(x, 314, 172, 52, f"{g} · {oq}", "", COR["entrada"]))
        if i:
            p.append(seta(x - 18, 340, x, 340))
        x += 190
    p.append(f'<path d="M373 228 V284 H100 V314" fill="none" stroke="{COR["entrada"]}" '
             'stroke-width="1.4" marker-end="url(#pf)"/>')
    p.append('<text x="205" y="280" text-anchor="middle" fill="var(--m)" font-size="10">G0 aprovado</text>')

    # 4 · PROVAR --------------------------------------------------------
    rotulos.append(faixa(404, "4 · PROVAR O RESULTADO — DOIS PORTÕES, NÃO UM"))
    p.append(caixa(14, 418, 200, 66, "F-GATE",
                   "funciona?\ntestes e golden master", COR["prova"]))
    p.append(caixa(234, 418, 200, 66, "C-GATE",
                   "está conforme?\nvalidador por restrição", COR["prova"]))
    p.append(caixa(454, 418, 220, 66, "evidência",
                   "VERIFICADO · PARCIAL ·\nNÃO VERIFICADO · FALHOU", COR["prova"]))
    p.append(caixa(694, 418, 220, 66, "grafo",
                   "código sem requisito,\nteste sem evidência…", COR["prova"]))
    p.append(caixa(934, 418, 200, 66, "efeito",
                   "leu o estado real?\nVERIFICADO · DIVERGENTE", COR["prova"]))
    p.append(caixa(1154, 418, 192, 66, "QA independente",
                   "quem valida não\nconserta o que detecta", COR["governo"]))
    for a, b in ((214, 234), (434, 454), (674, 694), (914, 934), (1134, 1154)):
        p.append(seta(a, 451, b, 451))
    p.append(seta(100, 366, 100, 418, "resultado"))
    # reprovado: sobe pela DIREITA, fora das caixas, e volta ao gate
    p.append(f'<path d="M1250 418 V392 H1300 V290 H480 V314" fill="none" stroke="{COR["portao"]}" '
             'stroke-width="1.4" stroke-dasharray="5 4" marker-end="url(#pf)"/>')
    p.append(f'<text x="900" y="285" text-anchor="middle" fill="{COR["portao"]}" '
             'font-size="10">reprovado no F-GATE ou no C-GATE: volta ao gate da conversão</text>')

    # 5 · ENTREGAR E AUDITAR --------------------------------------------
    rotulos.append(faixa(524, "5 · ENTREGAR — E O QUE O CLIENTE REGULADO PEDE JUNTO"))
    p.append(caixa(14, 538, 210, 62, "exportar",
                   "sete pastas, SHA-256\nde cada arquivo, sem segredo", COR["entrada"]))
    itens = [("procedência", "SLSA + CycloneDX"), ("decisão", "base ainda vale?"),
             ("gêmeo da sprint", "estado com hash"), ("telemetria", "OTLP no disco"),
             ("identidade", "SPIFFE + atestado")]
    x = 244
    for titulo, sub in itens:
        p.append(caixa(x, 538, 214, 62, titulo, sub, COR["governo"]))
        x += 224
    p.append(seta(224, 569, 244, 569))
    p.append(f'<path d="M1250 484 V538" fill="none" stroke="{COR["prova"]}" stroke-width="1.4" '
             'marker-end="url(#pf)"/>')
    p.append(f'<text x="1288" y="516" text-anchor="middle" fill="var(--m)" font-size="10">aprovado</text>')

    # 6 · HOOKS ---------------------------------------------------------
    p.append(f'<path d="M14 650 H1346" stroke="{COR["hook"]}" stroke-width="1.2" '
             'stroke-dasharray="5 4" fill="none"/>')
    rotulos.append(faixa(636, f"6 · OS {m['hooks_total']} HOOKS — CORREM POR BAIXO DE TUDO, EM TODA SESSÃO"))
    eventos = [("SessionStart", "licença, zelador,\nidentificação"),
               ("UserPromptSubmit", "identificação da sprint,\nRAG com localizador"),
               ("PreToolUse", "licença, anexos, segredos,\nG0, papel da sessão"),
               ("PostToolUse", "sincroniza o PMO,\nImpeccable na tela"),
               ("Stop", "revisão de design\nao terminar")]
    x, larg = 14, 262
    for nome, oq in eventos:
        n = m["hooks"].get(nome, 0)
        p.append(caixa(x, 662, larg, 66, f"{nome} ({n})", oq, COR["hook"], True))
        # a seta sobe da barra até a faixa de cima, em coluna livre
        p.append(f'<path d="M{x + larg / 2} 650 V614" fill="none" stroke="{COR["hook"]}" '
                 'stroke-width="1.2" stroke-dasharray="4 3" marker-end="url(#pf)"/>')
        x += larg + 12

    return (f'<svg viewBox="0 0 {L} 748" role="img" aria-label="Fluxo do WX Claude Code: '
            'os anexos viram contexto, o portão G0 nega escrita quando bloqueia e devolve para '
            'as evidências, a conversão atravessa sete gates, dois portões provam o resultado e '
            'devolvem ao gate quando reprovam, a entrega sai com os documentos de auditoria, e '
            'os hooks correm por baixo de tudo">'
            + "".join(p) + "".join(rotulos) + "</svg>")


# ---------------------------------------------------------------- a página

def pagina(m: dict) -> str:
    hooks_linhas = "".join(
        f"<tr><td><b>{E(nome)}</b></td><td class=\"mono\">{E(evento)}</td><td>{E(oq)}</td></tr>"
        for nome, evento, oq in [
            ("portão G0", "PreToolUse", "nenhuma escrita fora de .wx-migration antes do G0 passar; falha fechado"),
            ("papel da sessão", "PreToolUse", "com WX_PAPEL=qa, quem valida não escreve o produto que valida; sem papel declarado, nada muda"),
            ("guarda de anexos e segredos", "PreToolUse", "inputs/ e artefatos/ somente leitura; token em arquivo ou comando é recusado"),
            ("licença", "PreToolUse · SessionStart", "serial confere a máquina antes de rodar script do plugin"),
            ("sincronizar PMO", "PostToolUse", "Kanban acompanha a matriz sem ninguém digitar"),
            ("identificação", "UserPromptSubmit", "BlocoNNNN-SPNNNNN-Título · data em toda resposta"),
            ("RAG", "UserPromptSubmit", "injeta o trecho mais próximo com arquivo#linha, e o tema do Help"),
            ("zelador", "SessionStart", "limpa temporário uma vez por dia e mede o que liberou"),
            ("impeccable", "PostToolUse · Stop", "revisão de design da tela que acabou de mudar"),
        ])
    provas = "".join(
        f"<tr><td><b>{E(a)}</b></td><td class=\"mono\">{E(b)}</td><td>{E(c)}</td></tr>"
        for a, b, c in [
            ("tests/testes.py", f"{m['testes']} testes", "cada peça isolada: validação, hooks, licença, PMO, roteador, evidência, restrições, grafo, procedência"),
            ("tests/fluxo.py", f"{m['passos_do_fluxo']} passos", "o caminho inteiro num projeto novo: questionário → contexto → G0 → artefato → PDF → PMO → RAG → entrega → registro"),
            ("tests/cenarios.py", f"{m['cenarios']} cenários", "os outros caminhos: sem licença, PDF que é foto, legado PHP, F-GATE verde com C-GATE reprovado, entrega auditável"),
            ("validate_plugin_bundle.py --strict", "1 comando", "arquivos obrigatórios, manifesto, e roda a bateria por dentro"),
            ("claude plugin validate", "1 comando", "o manifesto pelos olhos do próprio Claude Code"),
        ])
    gerados = json.loads((RAIZ / "docs/dossie/numeros.json").read_text(encoding="utf-8")).get(
        "arquivos_gerados_pelo_questionario", "INDISPONÍVEL")
    ETAPAS = [
        ("Instalar e ativar", "claude plugin install", [
            ("licença", "serial confere a máquina; sem ele, os scripts do plugin recusam"),
            ("comandos", f"o índice: {m['comandos']} comandos e os {m['perguntas']} ids das perguntas")]),
        ("Perguntar", "/wx-claude-code:questionario", [
            ("bloco 0", "empresa, diretores, logotipos, prazo, orçamento, riscos, GitHub, aprovador (0.1–0.16)"),
            ("A–E", "evidências: SQL da análise e os PDFs — ou o código-fonte, quando o legado não é WX"),
            ("F", "tela modelo e estilo (F0–F13)"),
            ("G", "corpus do Help WLanguage (12k)"),
            ("H · I", "destino: qualquer linguagem; perfil «outra» é aceito"),
            ("J", "economia de tokens e modelo local"),
            ("K", "ambiente (K0–K7) e backup e replicação (K8)"),
            ("L", "kickoff, hooks, MCP, implantação, esqueleto de ERP (L1–L6)"),
            ("M", "artefatos do cliente, um por vez, com onde usar")]),
        ("Gerar o contexto", "aplicar_questionario.py", [
            ("CLAUDE.md · INDEX_FILES.md", "o que a primeira sessão lê antes de qualquer comando"),
            ("respostas_questionario.md", f"as {m['perguntas']} respostas com índice por id, para os agentes"),
            ("esqueleto", "AGENTS, CONTEXT, ADRs, domínios, database, src, tests, workflows"),
            (f"{gerados} arquivos", "nada é sobrescrito ao reaplicar")]),
        ("Provar a entrada", "/wx-claude-code:preflight — G0", [
            ("inventário", "cada evidência com hash e classificação"),
            ("legado sem WX", "o código-fonte é a evidência central; PDF não é exigido de quem nunca o teve"),
            ("relatório", "o hook portão G0 falha fechado se não conseguir lê-lo")]),
        ("Converter", "/wx-claude-code:converter — G1 a G7", [
            ("G1 inventário", "BR-, QRY-, UI-, RPT-, INT-, DB- com origem localizável"),
            ("G2 arquitetura", "o que cada peça vira; a estratégia escolhida em H"),
            ("G3 dados", "esquema migrado, chaves e integridade"),
            ("G4 piloto", "uma vertical inteira, com golden master"),
            ("G5 ondas", "módulo a módulo, telas pelo Impeccable"),
            ("G6 homologação", "paralelo com o legado"),
            ("G7 virada", "só com backup do legado restaurado, não copiado")]),
        ("Provar o resultado", "constraints · evidencia · grafo · efeito", [
            ("F-GATE e C-GATE", "funciona? e está conforme? — a Sprint precisa dos dois"),
            ("evidência", "quatro estados, e o que ela NÃO prova é campo obrigatório"),
            ("grafo", "código sem requisito, requisito sem teste, teste sem evidência, prova vencida"),
            ("efeito", "lê o estado real; inconclusivo tem código de saída próprio")]),
        ("Governar", "/wx-claude-code:pmo · contrato", [
            ("sprints", "BlocoNNNN-SPNNNNN, cada uma em .md zipado"),
            ("contrato ativo", "o que vale hoje; o superado fica no histórico com o motivo"),
            ("papel da sessão", "quem valida não escreve o produto que valida"),
            ("equipe A–J", "zelador, pesquisador, documentador, qualidade, tarefas, GP, testes, status, base, tradutor")]),
        ("Entregar e auditar", "/wx-claude-code:exportar · procedencia", [
            ("sete pastas", "manifesto com SHA-256 de cada arquivo, sem segredo"),
            ("procedência", "SLSA e CycloneDX, com o nível de SLSA marcado INDISPONÍVEL e o porquê"),
            ("gêmeo e decisão", "a sprint fotografada e a base de cada decisão, para reconferir depois"),
            ("identidade", "SPIFFE assinado e o atestado — que declara não ser attestation")]),
    ]
    etapas = "".join(
        f'<section class="etapa"><div class="cab"><span class="num">{i}</span>'
        f'<h2>{E(titulo)}</h2><code>{E(cmd)}</code></div><ul>'
        + "".join(f"<li><b>{E(a)}</b><span>{E(b)}</span></li>" for a, b in itens)
        + "</ul></section>"
        for i, (titulo, cmd, itens) in enumerate(ETAPAS, 1))

    kpis = "".join(f"<div><b>{v}</b><small>{E(r)}</small></div>" for v, r in [
        (m["comandos"], "comandos"), (m["agentes"], "agentes"), (m["skills"], "skills"),
        (m["scripts"], "scripts"), (m["hooks_total"], "hooks"), (m["testes"], "testes")])

    return f'''<title>Fluxo do WX Claude Code</title>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Exo+2:wght@600;800&family=Source+Serif+4:opsz,wght@8..60,400;8..60,600&family=JetBrains+Mono:wght@400&display=swap">
<style>
:root{{--g:#FBFAF7;--p:#FFFFFF;--i:#14161F;--m:#6B6F82;--l:#D9D6CE;--a:#C63C0A;--az:#1F5FBF;--vd:#1F7A4D;--am:#9A6A00;--grid:#ECEAE3;--a2:#1F5FBF}}
@media(prefers-color-scheme:dark){{:root:not([data-theme="light"]){{--g:#0B0D17;--p:#121527;--i:#EDEDF3;--m:#9AA0B8;--l:#2E3454;--a:#E2261C;--az:#6FA3FF;--vd:#2FBF71;--am:#F7B733;--grid:#1B1F33;--a2:#6FA3FF}}}}
:root[data-theme="dark"]{{--g:#0B0D17;--p:#121527;--i:#EDEDF3;--m:#9AA0B8;--l:#2E3454;--a:#E2261C;--az:#6FA3FF;--vd:#2FBF71;--am:#F7B733;--grid:#1B1F33;--a2:#6FA3FF}}
*{{box-sizing:border-box}}
body{{margin:0;background:var(--g);color:var(--i);font-family:"Source Serif 4",Georgia,serif;font-size:16px;line-height:1.5}}
.w{{max-width:1240px;margin:0 auto;padding:28px 20px 40px}}
h1{{font-family:"Exo 2",sans-serif;font-weight:800;font-size:28px;color:var(--a);margin:0 0 4px}}
.sub{{color:var(--m);margin:0 0 18px;max-width:78ch}}
figure{{margin:0;background:var(--p);border:1px solid var(--l);border-radius:12px;padding:16px;overflow-x:auto}}
svg{{width:100%;height:auto;min-width:900px;font-family:"Exo 2",system-ui,sans-serif;color:var(--i)}}
figcaption{{color:var(--m);font-size:14px;margin-top:12px;max-width:82ch}}
h2.sec{{font-family:"Exo 2",sans-serif;font-size:21px;margin:38px 0 8px}}
.scroll{{overflow-x:auto;border:1px solid var(--l);background:var(--p);margin-top:10px}}
table{{border-collapse:collapse;width:100%;font-size:13.5px}}
th{{font-family:"Exo 2",sans-serif;font-size:11px;letter-spacing:.08em;text-transform:uppercase;text-align:left;color:var(--m);padding:9px 11px;border-bottom:2px solid var(--l)}}
td{{padding:8px 11px;border-bottom:1px solid var(--grid);vertical-align:top}}
td.mono{{font-family:"JetBrains Mono",monospace;font-size:12px;color:var(--a2);white-space:nowrap}}
tr{{page-break-inside:avoid}}
.nota{{color:var(--m);font-size:14px;max-width:80ch;margin-top:14px}}
.kpis{{display:grid;grid-template-columns:repeat(6,1fr);gap:10px;margin:18px 0 4px}}
.kpis div{{background:var(--p);border:1px solid var(--l);padding:10px 12px;font-family:"Exo 2",sans-serif}}
.kpis b{{display:block;font-size:22px;font-weight:800;font-variant-numeric:tabular-nums}}
.kpis small{{color:var(--m);font-size:11px;letter-spacing:.05em;text-transform:uppercase}}
.etapa{{margin-top:20px;border-left:3px solid var(--a);padding-left:18px}}
.cab{{display:flex;align-items:baseline;gap:12px;flex-wrap:wrap}}
.cab .num{{font-family:"Exo 2",sans-serif;font-weight:800;font-size:22px;color:var(--a)}}
.cab h2{{font-family:"Exo 2",sans-serif;font-size:19px;font-weight:700;margin:0}}
.cab code{{font-size:12.5px;color:var(--m)}}
.etapa ul{{list-style:none;margin:10px 0 0;padding:0;display:grid;grid-template-columns:repeat(auto-fill,minmax(320px,1fr));gap:6px}}
.etapa li{{display:grid;grid-template-columns:minmax(130px,34%) 1fr;gap:10px;align-items:baseline;background:var(--p);border:1px solid var(--l);padding:7px 11px}}
.etapa li b{{font-family:"Exo 2",sans-serif;font-size:13px}}
.etapa li span{{color:var(--m);font-size:13.5px}}
code{{font-family:"JetBrains Mono",monospace;font-size:12.5px;background:var(--grid);padding:1px 5px;border-radius:3px}}
@media print{{.w{{padding:0 0 8px}} svg{{min-width:0}}}}
</style>
<div class="w">
<h1>Fluxo atual do WX Claude Code</h1>
<p class="sub">Versão {E(m["versao"])}, medida em {date.today().isoformat()}. O desenho mostra o mecanismo, não a lista de peças: <b>onde cada portão nega passagem e para onde a coisa volta quando ele nega</b>. Portão desenhado como caixa no meio da linha não explica nada; a seta de volta explica.</p>

<div class="kpis">{kpis}</div>

<figure>
{desenho(m)}
<figcaption>Da entrada à entrega. As setas cheias são o caminho quando passa; as <b>tracejadas em vermelho</b> são o que acontece quando um portão nega — o G0 bloqueado impede escrever código e devolve para as evidências; o resultado reprovado no F-GATE ou no C-GATE volta ao gate da conversão. Os hooks, embaixo, correm por baixo de tudo, em toda sessão, sem ninguém pedir.</figcaption>
</figure>

<h2 class="sec">As guardas, que rodam sozinhas</h2>
<p class="nota" style="margin-top:0">Regra que depende de alguém lembrar não é regra. Estas rodam nos eventos do Claude Code, e cada negativa entra no registro de operações. A do <b>papel da sessão</b> é a mais nova e segue a regra da casa: sem papel declarado, nada muda para quem já usava o plugin.</p>
<div class="scroll"><table><thead><tr><th style="width:22%">guarda</th><th style="width:22%">evento</th><th>o que faz valer</th></tr></thead><tbody>{hooks_linhas}</tbody></table></div>

<h2 class="sec">O que roda antes de cada commit</h2>
<p class="nota" style="margin-top:0">Três provas, em níveis que não se substituem: a bateria pega a peça quebrada, o fluxo pega a peça certa ligada errada, e os cenários pegam o caminho que ninguém imaginou.</p>
<div class="scroll"><table><thead><tr><th style="width:26%">prova</th><th style="width:12%">tamanho</th><th>o que pega</th></tr></thead><tbody>{provas}</tbody></table></div>

<h2 class="sec">As oito etapas, do install à entrega</h2>
<p class="nota" style="margin-top:0">O desenho mostra o mecanismo; a lista mostra a ordem em que um projeto passa por ele. Os números saem do repositório, não desta prosa.</p>
{etapas}

<h2 class="sec">Onde as coisas caem</h2>
<p class="nota" style="margin-top:0"><code>.wx-migration/</code> é a governança — respostas, matriz, contrato ativo, restrições, evidências, decisões capturadas, gêmeos de sprint, procedência, telemetria, PMO, prompts e logs. <code>inputs/</code> é a evidência do legado, <b>somente leitura</b>; <code>artefatos/</code> é o que o cliente mandou por fora, também somente leitura e com hash. A raiz recebe o que a primeira sessão lê (<code>CLAUDE.md</code>, <code>INDEX_FILES.md</code>) e o esqueleto do projeto. O organograma de arquivos detalha os {gerados} arquivos que o questionário pode gerar.</p>

<p class="nota">Esta página é gerada por <code>docs/dossie/gerar-fluxo.py</code> e atualizada junto das outras por <code>docs/dossie/atualizar-paginas.py</code>. Ela era a última mantida à mão — e ficou seis lançamentos atrás, dizendo «19 comandos» quando já havia {m["comandos"]}. Número visível sai de gerador, ou envelhece calado.</p>
</div>
'''


def main() -> int:
    saida = Path(sys.argv[1]) if len(sys.argv) > 1 else RAIZ / "docs/dossie/fluxo-atual.html"
    m = medir()
    saida.write_text(pagina(m), encoding="utf-8")
    print(f"ok {saida} ({m['comandos']} comandos, {m['hooks_total']} hooks, "
          f"{m['testes']} testes, {m['cenarios']} cenários)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
