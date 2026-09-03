#!/usr/bin/env python3
"""Mede o `servidor.rs`: regioes, funcoes, o que cada uma toca e o acoplamento.

Nenhum numero do FRONTEIRAS-DO-SERVIDOR.md se digita -- todos saem daqui.
Uso:  python3 docs/fronteiras/mapa-do-servidor.py [caminho/do/servidor.rs]

Sai em Markdown na saida padrao; `--json` sai em JSON para outro gerador.
"""

import json
import re
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
PADRAO = RAIZ / "crates/phxsql-server/src/servidor.rs"


# ---------------------------------------------------------------- tokenizacao
def limpar(fonte: str) -> str:
    """Troca comentarios e literais por espaco, preservando quebras de linha.

    Existe porque a contagem de chaves e o crivo do que a funcao TOCA nao
    podem enxergar uma chave dentro de uma string nem um `self.dados` dentro
    de um comentario -- foi assim que a primeira medicao contou regioes que
    nao existiam.
    """
    saida = []
    i, n = 0, len(fonte)
    while i < n:
        c = fonte[i]
        prox = fonte[i + 1] if i + 1 < n else ""
        if c == "/" and prox == "/":
            j = fonte.find("\n", i)
            j = n if j < 0 else j
            saida.append(" " * (j - i))
            i = j
        elif c == "/" and prox == "*":
            j = fonte.find("*/", i + 2)
            j = n if j < 0 else j + 2
            saida.append("".join(ch if ch == "\n" else " " for ch in fonte[i:j]))
            i = j
        elif c == "r" and fonte[i + 1 : i + 2] in ('"', "#"):
            m = re.match(r'r(#*)"', fonte[i:])
            if not m:
                saida.append(c)
                i += 1
                continue
            fecha = '"' + m.group(1)
            j = fonte.find(fecha, i + m.end())
            j = n if j < 0 else j + len(fecha)
            saida.append("".join(ch if ch == "\n" else " " for ch in fonte[i:j]))
            i = j
        elif c == '"':
            j = i + 1
            while j < n:
                if fonte[j] == "\\":
                    j += 2
                    continue
                if fonte[j] == '"':
                    j += 1
                    break
                j += 1
            saida.append("".join(ch if ch == "\n" else " " for ch in fonte[i:j]))
            i = j
        elif c == "'" and re.match(r"'(\\.|[^'\\])'", fonte[i:]):
            m = re.match(r"'(\\.|[^'\\])'", fonte[i:])
            saida.append(" " * m.end())
            i += m.end()
        else:
            saida.append(c)
            i += 1
    return "".join(saida)


def fim_do_bloco(limpo: str, linhas_limpo: list, ini: int) -> int:
    """Linha (1-based) da chave que fecha o bloco aberto a partir de `ini`."""
    prof = 0
    viu = False
    for k in range(ini - 1, len(linhas_limpo)):
        for ch in linhas_limpo[k]:
            if ch == "{":
                prof += 1
                viu = True
            elif ch == "}":
                prof -= 1
                if viu and prof == 0:
                    return k + 1
    return len(linhas_limpo)


# ------------------------------------------------------------------- o crivo
# O que cada regiao TOCA. A costura natural sai daqui, nao do nome da funcao.
DOMINIOS = {
    "trava-de-dados": [r"\btravar_dados\b", r"\bself\.dados\b", r"\bInstancia\b"],
    "rede": [
        r"\bTcpStream\b", r"\bTcpListener\b", r"\bSocketAddr\b", r"\bBufReader\b",
        r"\bCanal\b", r"\bfluxo\b", r"\bset_read_timeout\b", r"\bshutdown\b",
    ],
    "catalogo-e-disco": [
        r"\bphxsql_store::", r"\bTable::", r"\bSchema\b", r"\bVisao\b",
        r"\babrir_tabela\b", r"\bPathBuf\b", r"\bstd::fs::",
    ],
    "interface-web": [r"\bcrate::http\b", r"\bhttp::", r"\bSessoes\b", r"\bsessoes\b"],
    "replicacao-e-cluster": [
        r"\bcrate::cluster\b", r"\bEstadoOrigem\b", r"\bbidirecional\b",
        r"\bdiario\b", r"\bMarcaDoDiario\b", r"\breplic", r"\bpulso\b", r"\barbitro\b",
    ],
    "sql": [r"\bphxsql_sql::", r"\bcrate::rotinas\b", r"\bMotorDoServidor\b"],
    "permissao": [
        r"\bAtividade\b", r"\bUsuario\b", r"\bpode_em\b", r"\bviolacao_",
        r"\bBlacklist\b", r"\blista_negra\b", r"\bportoes_do_pedido\b",
    ],
    "transacao-e-travas": [r"\bcrate::transacao\b", r"\bcrate::travas\b", r"\btransacoes\b", r"\btravas\b"],
    "observacao": [r"\bprofiler\b", r"\btelemetria\b", r"\bmonitor\b", r"\bLogAcessos\b"],
    "config": [r"\bself\.config\b", r"\bConfig\b", r"\bDurabilidade\b", r"\bPapel\b"],
}
REGEX = {d: re.compile("|".join(p)) for d, p in DOMINIOS.items()}

# Cada regiao do arquivo, pelo que ela e -- nao pelo nome que tem.
def familia(nome: str, corpo: str) -> str:
    if nome.startswith("op_"):
        return "operacao"
    if nome.startswith("atender") or nome in ("escutar", "despachar", "aceitar_ate_mandarem_parar"):
        return "porta"
    if nome.startswith("laco_") or nome.startswith("subir_") or nome.startswith("rodada_"):
        return "thread-de-fundo"
    if nome.startswith("pagina") or nome.startswith("html"):
        return "tela"
    return "apoio"


# As regioes contiguas do `impl Servidor`, ancoradas na PRIMEIRA e na ULTIMA
# funcao de cada uma -- nunca em numero de linha.
#
# A primeira versao usava intervalos (687, 982). Uma frente vizinha
# acrescentou 8 linhas ao arquivo enquanto este documento era escrito, e o
# `resposta_erro` caiu para fora da ultima regiao: a conferencia de cobertura
# acusou 274 de 275. Numero de linha e a receita que envelhece a cada edicao
# de outra pessoa; nome de funcao sobrevive a insercao acima dele.
REGIOES = [
    ("arranque-e-identidade", "novo", "telemetria"),
    ("porta-e-aceitacao", "escutar", "anotar_porta_no_ar"),
    ("firewall-e-mensagens", "violacao_grave", "semear_mensagens"),
    ("relogios-de-fundo", "ligar_relogio_de_gravacao", "subir_amostrador"),
    ("replicacao", "subir_replicacao", "alcancar_tabela"),
    ("cluster", "subir_cluster", "op_cluster_estado"),
    ("bidirecional", "rodada_bidirecional", "aplicar_por_chave"),
    ("backup-agendado", "subir_backup_agendado", "limpar_backups_velhos"),
    ("config-e-servico", "configuracao_json", "acordar_o_accept"),
    ("jobs", "op_jobs", "texto_do_aviso_de_parado"),
    ("web-http-rest", "subir_web", "api_http"),
    ("porta-de-dados-e-aperto", "atender", "recusar_texto_claro"),
    ("portoes-e-despacho", "despachar", "abrir_travada"),
    ("operacoes-de-dados", "sobreposicao", "op_reindexar"),
    ("erros-de-resposta", "campos_do_erro", "resposta_erro"),
]

# As portas de entrada, e o portao que cada uma tem de alcancar. A pergunta
# que esta tabela responde e a que ja custou caro aqui: quem NAO passa pelo
# portao unico.
PORTAS = [
    "escutar", "atender", "atender_http", "atender_rest", "atender_swagger",
    "executar_derivado", "executar_job",
]
PORTOES = ["despachar", "portoes_do_pedido", "executar"]

# Os itens privados que os testes de dentro do arquivo alcancam. Sao eles que
# dizem quais modulos de teste podem virar IRMAOS e quais tem de ficar filhos.
PRIVADOS_NO_TESTE = (
    r"\b(Sessao|TravaMedida|Janela|despachar|portoes_do_pedido|executar"
    r"|travar_dados|abrir_travada|OPS_ESCRITA|OPS_NO_SPARE|OPS_DE_REPLICACAO"
    r"|OPS_DE_TRANSACAO|OPS_EMPILHAVEIS)\b"
)


def medir(caminho: Path) -> dict:
    fonte = caminho.read_text(encoding="utf-8")
    limpo = limpar(fonte)
    # `split` na ultima quebra inventa uma linha vazia: sem isto o script
    # diz 23164 onde `wc -l` diz 23163.
    if fonte.endswith("\n"):
        fonte, limpo = fonte[:-1], limpo[:-1]
    linhas = fonte.split("\n")
    linhas_l = limpo.split("\n")
    total = len(linhas)

    # ---- topo: os itens de nivel zero
    topo = []
    re_topo = re.compile(
        r"^(pub(\([a-z()]+\))? )?(unsafe )?(impl|struct|enum|trait|fn|mod|const|static|type)\b"
    )
    for i, l in enumerate(linhas_l, 1):
        if re_topo.match(l):
            fim = fim_do_bloco(limpo, linhas_l, i) if "{" in l or l.rstrip().endswith("(") else i
            # itens sem corpo (const/type de uma linha) terminam no ';'
            if l.lstrip().startswith(("const", "static", "type")):
                j = i
                while j <= total and ";" not in linhas_l[j - 1]:
                    j += 1
                fim = j
            topo.append({"linha": i, "fim": fim, "texto": linhas[i - 1].strip(), "bruto": l.strip()})

    # ---- fronteira codigo / teste
    testes = [i for i, l in enumerate(linhas_l, 1) if linhas[i - 1].strip() == "#[cfg(test)]"]
    mods_teste = []
    for t in testes:
        j = t
        while j <= total and not linhas_l[j - 1].lstrip().startswith("mod "):
            j += 1
        if j <= total:
            fim = fim_do_bloco(limpo, linhas_l, j)
            nome = re.search(r"mod\s+(\w+)", linhas[j - 1]).group(1)
            mods_teste.append({"nome": nome, "ini": t, "fim": fim, "linhas": fim - t + 1})
    prim_teste = mods_teste[0]["ini"] if mods_teste else total + 1

    # ---- blocos impl
    impls = []
    for it in topo:
        if it["bruto"].startswith("impl") and it["fim"] > it["linha"]:
            impls.append(
                {
                    "assinatura": it["texto"].rstrip("{").strip(),
                    "ini": it["linha"],
                    "fim": it["fim"],
                    "linhas": it["fim"] - it["linha"] + 1,
                    "de_teste": it["linha"] >= prim_teste,
                }
            )

    # ---- funcoes: metodos (indentacao 4) e livres (indentacao 0)
    funcoes = []
    re_fn = re.compile(r"^(?P<ind>\s*)(pub(\([a-z()]+\))?\s+)?(async\s+)?(unsafe\s+)?fn\s+(?P<nome>\w+)")
    for i, l in enumerate(linhas_l, 1):
        m = re_fn.match(l)
        if not m or len(m.group("ind")) > 4:
            continue
        fim = fim_do_bloco(limpo, linhas_l, i)
        corpo = "\n".join(linhas_l[i - 1 : fim])
        corpo_bruto = "\n".join(linhas[i - 1 : fim])
        # Pelo INTERVALO, nunca pela assinatura: `impl Servidor` aparece
        # duas vezes, e casar por texto dava as 275 funcoes aos dois.
        dono_b = None
        for b in impls:
            if b["ini"] <= i <= b["fim"] and (
                dono_b is None or b["ini"] > dono_b["ini"]
            ):
                dono_b = b
        dono = f'{dono_b["assinatura"]}@{dono_b["ini"]}' if dono_b else "(livre)"
        nome = m.group("nome")
        toca = sorted(d for d, r in REGEX.items() if r.search(corpo))
        funcoes.append(
            {
                "nome": nome,
                "ini": i,
                "fim": fim,
                "linhas": fim - i + 1,
                "dono": dono,
                "de_teste": i >= prim_teste,
                "toca": toca,
                "campos_self": sorted(set(re.findall(r"self\.(\w+)", corpo))),
                "publica": bool(m.group(2)),
                "familia": familia(nome, corpo),
                "chama_outras": len(set(re.findall(r"self\.(\w+)\s*\(", corpo))),
                "usa_pedido_tabela": bool(re.search(r'texto_ou\("tabela"', corpo_bruto)),
            }
        )

    prod = [f for f in funcoes if not f["de_teste"]]

    # ---- os campos do `self` gordo
    campos = []
    st = next((it for it in topo if it["bruto"].startswith("pub struct Servidor")), None)
    if st:
        for k in range(st["linha"], st["fim"]):
            m = re.match(r"^\s{4}(pub\s+)?(\w+):\s", linhas_l[k])
            if m:
                campos.append(m.group(2))
    uso_campo = {c: 0 for c in campos}
    for f in prod:
        for c in f["campos_self"]:
            if c in uso_campo:
                uso_campo[c] += 1

    # ---- o que o arquivo importa do resto da arvore
    usos = sorted(set(re.findall(r"\bcrate::(\w+)", limpo)))
    usos_prod = {}
    for u in usos:
        r = re.compile(r"\bcrate::" + u + r"\b")
        usos_prod[u] = sum(1 for f in prod if r.search("\n".join(linhas_l[f["ini"] - 1 : f["fim"]])))


    # ---------------------------------------------------------- as regioes
    limpo_l = linhas_l
    def corpo(f):
        return "\n".join(limpo_l[f["ini"] - 1 : f["fim"]])

    do_servidor = [f for f in prod if f["dono"].startswith("impl Servidor")]
    por_nome = {f["nome"]: f for f in do_servidor}

    # o que os `use` do topo trazem, para contar quantos cada regiao precisa
    cabeca = "\n".join(linhas[: min(60, total)])
    importados = set()
    for mm in re.finditer(r"use\s+[\w:]*\{([^}]*)\}|use\s+([\w:]+)(?:\s+as\s+(\w+))?;", cabeca):
        if mm.group(1):
            for t in mm.group(1).split(","):
                t = t.strip().split(" as ")[-1].strip()
                if t and t != "self":
                    importados.add(t.split("::")[-1])
        elif mm.group(2):
            importados.add((mm.group(3) or mm.group(2)).split("::")[-1])
    importados = {n for n in importados if n and n[0].isalpha()}

    campos_set = set(campos)
    # nome da funcao -> intervalo de linhas, resolvido AGORA contra o arquivo
    pos = {}
    for nome, prim, ult in REGIOES:
        f0, f1 = por_nome.get(prim), por_nome.get(ult)
        if not f0 or not f1:
            faltando = prim if not f0 else ult
            raise SystemExit(
                f"a regiao {nome!r} ancora em {faltando!r}, que nao existe mais no "
                f"arquivo. Renomearam ou removeram a funcao: ajuste REGIOES."
            )
        pos[nome] = (f0["ini"], f1["fim"])
    regioes = []
    for nome, _prim, _ult in REGIOES:
        a, b = pos[nome]
        dentro = [f for f in do_servidor if a <= f["ini"] <= b]
        nomes_dentro = {f["nome"] for f in dentro}
        sai, entra = set(), set()
        junto = "\n".join(corpo(f) for f in dentro)
        for f in dentro:
            for n in set(re.findall(r"self\.(\w+)\s*\(", corpo(f))):
                if n in por_nome and n not in nomes_dentro:
                    sai.add(n)
        for f in do_servidor:
            if f["nome"] in nomes_dentro:
                continue
            for n in set(re.findall(r"self\.(\w+)\s*\(", corpo(f))):
                if n in nomes_dentro:
                    entra.add(n)
        cs = set()
        for f in dentro:
            cs |= set(f["campos_self"]) & campos_set
        regioes.append(
            {
                "nome": nome, "ini": a, "fim": b,
                "funcoes": len(dentro),
                "linhas": sum(f["linhas"] for f in dentro),
                "sai": sorted(sai), "entra": sorted(entra),
                "campos": sorted(cs),
                "imports": sorted(n for n in importados if re.search(r"\b" + re.escape(n) + r"\b", junto)),
                "trava_de_dados": "travar_dados" in junto,
            }
        )
    # Conferencia: as regioes tem de cobrir o `impl Servidor` inteiro. Sem
    # isto, uma funcao nova cairia num vao e sumiria da conta.
    cobertas = sum(r["funcoes"] for r in regioes)
    fora = [f["nome"] for f in do_servidor
            if not any(a <= f["ini"] <= b for a, b in pos.values())]

    # ------------------------------------------- as portas e o portao unico
    chama = {
        f["nome"]: sorted(n for n in set(re.findall(r"self\.(\w+)\s*\(", corpo(f))) if n in por_nome)
        for f in do_servidor
    }

    def rota(ini, alvo):
        """A rota MAIS CURTA da porta ate o portao -- em largura, nao em
        profundidade.

        Em profundidade o script dizia que `executar_derivado` alcanca o
        portao «via executar > op_job_rodar > rodar_job > executar_job»,
        quando ele chama `portoes_do_pedido` na linha seguinte. Rota que
        existe nao e a rota que o pedido faz, e um mapa que mostra a errada
        ensina o mecanismo errado.
        """
        from collections import deque
        fila = deque([[ini]])
        visto = {ini}
        while fila:
            cam = fila.popleft()
            if cam[-1] == alvo:
                return cam
            if len(cam) > 7:
                continue
            for n in chama.get(cam[-1], []):
                if n not in visto:
                    visto.add(n)
                    fila.append(cam + [n])
        return None

    portas = []
    for nome in PORTAS:
        f = por_nome.get(nome)
        if not f:
            continue
        achou = None
        for alvo in PORTOES:
            cam = rota(nome, alvo)
            if cam:
                achou = (alvo, cam[1:-1])
                break
        portas.append(
            {"nome": nome, "linha": f["ini"], "linhas": f["linhas"],
             "portao": achou[0] if achou else None,
             "via": achou[1] if achou else []}
        )

    # ------------------------------- o campo que o portao le, e quem nao tem
    ops = [f for f in do_servidor if f["nome"].startswith("op_")]
    outro_campo, propria, com_tabela = [], [], []
    # O RECEPTOR importa, e a primeira versao deste crivo errou justamente
    # nisso: `op_juntar` le `"tabela"` -- mas em `pa.texto_ou("tabela")`, um
    # objeto ANINHADO. O portao le `pedido.texto_ou("tabela")` no pedido de
    # cima, e ali o campo vem vazio. Contar pela presenca do literal dava
    # `juntar` como coberto, que e exatamente o furo que ele foi.
    re_no_pedido = re.compile(r'\b(?:p|pedido)\s*\.\s*(?:texto_ou|campo)\(\s*"tabela"')
    re_aninhado = re.compile(r'\b(?!p\b|pedido\b)(\w+)\s*\.\s*(?:texto_ou|campo)\(\s*"tabela"')
    re_outro = re.compile(
        r'\b(?:p|pedido)\s*\.\s*(?:texto_ou|campo)\(\s*"(destino|destino_database|tabelas|origem_tabela|alvo)"'
    )
    re_prop = re.compile(r"pode_em|pode_ver_tabela|Atividade::")
    for f in ops:
        cru = "\n".join(linhas[f["ini"] - 1 : f["fim"]])
        campos_cegos = sorted(set(re_outro.findall(cru)))
        receptores = sorted(set(re_aninhado.findall(cru)))
        if re_no_pedido.search(cru):
            com_tabela.append(f["nome"])
        if campos_cegos or receptores:
            # O crivo acha o campo pelo NOME, e nome nao diz o que a coisa e:
            # o `"destino"` do `backup` e um diretorio, nao uma tabela. Quem
            # decide e o uso -- vira caminho de arquivo, ou vira tabela aberta
            # e conferida. Sem esta separacao a tabela publicaria dois furos
            # que nao existem, e furo inventado gasta a mesma leitura que o
            # verdadeiro.
            eh_caminho = bool(re.search(r"Path::new\(&destino|PathBuf::from\(&destino", cru))
            outro_campo.append(
                {
                    "nome": f["nome"], "linha": f["ini"], "campos": campos_cegos,
                    # a tabela nomeada dentro de outro objeto -- `a.tabela`
                    "aninhada_em": receptores,
                    "alvo": "caminho de arquivo" if eh_caminho and not receptores else "tabela",
                    "confere_por_conta": bool(re_prop.search(cru)),
                }
            )
        if re_prop.search(cru):
            propria.append({"nome": f["nome"], "linha": f["ini"]})

    # ------------------------------------------- o estado global de modulo
    globais = []
    # `const` junto de `static`: as listas `OPS_*` sao lidas de varias regioes
    # e de fora do modulo, e um filho que redeclarasse uma delas teria uma
    # segunda verdade sobre o que e escrita. Mesma familia de risco que os
    # `thread_local!`, por outro caminho.
    for mm in re.finditer(r"(?:static|const)\s+([A-Z_][A-Z0-9_]*)\s*:", limpo):
        nome = mm.group(1)
        decl = limpo[: mm.start()].count("\n") + 1
        usos_l = [i for i, l in enumerate(linhas_l, 1)
                  if re.search(r"\b" + nome + r"\b", l) and i != decl]
        if not usos_l:
            continue
        regs = sorted({r["nome"] for r in regioes for u in usos_l if r["ini"] <= u <= r["fim"]})
        globais.append({"nome": nome, "declarado": decl, "usos": usos_l,
                        "regioes": regs,
                        # constante declarada e usada so dentro de `mod testes_*`
                        # nao e fronteira de nada: fica fora da tabela.
                        "so_em_teste": decl >= prim_teste,
                        "fora_de_regiao": [u for u in usos_l
                                           if not any(r["ini"] <= u <= r["fim"] for r in regioes)]})

    # ------------------------------------------------------------- o risco
    re_priv = re.compile(PRIVADOS_NO_TESTE)
    for t in mods_teste:
        t["privados"] = sorted(set(re_priv.findall("\n".join(linhas[t["ini"] - 1 : t["fim"]]))))

    # ------------------------------------------------------- o carimbo
    # A arvore e compartilhada. Enquanto este documento era escrito, um vizinho
    # acrescentou 9 linhas ao `servidor.rs` e TODA linha citada envelheceu de
    # uma vez. O carimbo existe para que a proxima sessao descubra isso com um
    # `wc -l` em vez de com um numero errado publicado.
    import hashlib, subprocess, datetime
    sha = hashlib.sha256(caminho.read_bytes()).hexdigest()
    try:
        git = subprocess.run(["git", "-C", str(RAIZ), "rev-parse", "--short", "HEAD"],
                             capture_output=True, text=True, timeout=10).stdout.strip()
        sujo = subprocess.run(["git", "-C", str(RAIZ), "status", "--porcelain",
                               str(caminho.relative_to(RAIZ))],
                              capture_output=True, text=True, timeout=10).stdout.strip()
        if sujo:
            git += " (com mudanca nao commitada neste arquivo)"
    except Exception:
        git = "(sem git)"
    pasta = caminho.parent
    linhas_crate = sum(
        len(f.read_text(encoding="utf-8", errors="replace").split("\n"))
        for f in sorted(pasta.glob("*.rs"))
    )

    # ------------------------------- quem, de fora do modulo, cita `servidor::`
    externas = []
    base = RAIZ
    for f in sorted(base.glob("crates/*/tests/*.rs")) + sorted(base.glob("crates/*/examples/*.rs")) \
            + sorted(base.glob("crates/phxsql-server/src/*.rs")):
        if f == caminho:
            continue
        try:
            txt = f.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        for i, l in enumerate(txt.split("\n"), 1):
            for it in re.findall(r"servidor::(\w+)", l):
                externas.append({"arquivo": str(f.relative_to(base)), "linha": i, "item": it})
    testes_fora = sorted((RAIZ / "crates/phxsql-server/tests").glob("*.rs"))
    linhas_testes_fora = sum(
        len(f.read_text(encoding="utf-8", errors="replace").split("\n")) for f in testes_fora
    )
    sem_modulo = sum(
        1 for f in testes_fora
        if "servidor::" not in f.read_text(encoding="utf-8", errors="replace")
    )

    return {
        "arquivo": str(caminho.relative_to(RAIZ)),
        "linhas": total,
        "linhas_de_codigo": prim_teste - 1,
        "linhas_de_teste": total - prim_teste + 1,
        "topo": topo,
        "impls": impls,
        "funcoes": funcoes,
        "producao": prod,
        "mods_teste": mods_teste,
        "campos_do_servidor": campos,
        "uso_por_campo": uso_campo,
        "usos_do_crate": usos_prod,
        "regioes": regioes,
        "regioes_cobrem": cobertas,
        "fora_das_regioes": fora,
        "portas": portas,
        "ops": len(ops),
        "ops_com_campo_tabela": len(com_tabela),
        "ops_por_outro_campo": outro_campo,
        "ops_com_conferencia_propria": propria,
        "globais_de_modulo": globais,
        "medido_em": datetime.date.today().isoformat(),
        "sha256": sha,
        "git": git,
        "linhas_do_crate": linhas_crate,
        "fatia_do_crate": round(100 * total / linhas_crate),
        "imports_no_topo": len(importados),
        "referencias_externas": externas,
        "baterias_de_fora": len(testes_fora),
        "linhas_de_teste_de_fora": linhas_testes_fora,
        "baterias_sem_o_modulo": sem_modulo,
    }


# ------------------------------------------------------------------- relatorio
def tabela(cab, linhas):
    largura = [len(c) for c in cab]
    for l in linhas:
        for i, c in enumerate(l):
            largura[i] = max(largura[i], len(str(c)))
    out = ["| " + " | ".join(c.ljust(largura[i]) for i, c in enumerate(cab)) + " |"]
    out.append("|" + "|".join("-" * (w + 2) for w in largura) + "|")
    for l in linhas:
        out.append("| " + " | ".join(str(c).ljust(largura[i]) for i, c in enumerate(l)) + " |")
    return "\n".join(out)


def relatorio(m: dict) -> str:
    p = m["producao"]
    o = []
    o.append(f"ARQUIVO: {m['arquivo']}")
    o.append(f"linhas totais ............ {m['linhas']}")
    o.append(f"  codigo ................. {m['linhas_de_codigo']}")
    o.append(f"  testes ................. {m['linhas_de_teste']} em {len(m['mods_teste'])} modulos")
    o.append(f"funcoes de producao ...... {len(p)}")
    o.append(f"blocos impl .............. {len([b for b in m['impls'] if not b['de_teste']])}")
    o.append(f"campos do `Servidor` ..... {len(m['campos_do_servidor'])}")
    o.append("")
    o.append("BLOCOS IMPL (producao)")
    o.append(
        tabela(
            ["bloco", "ini", "fim", "linhas", "fns"],
            [
                [
                    b["assinatura"][:60],
                    b["ini"],
                    b["fim"],
                    b["linhas"],
                    len([f for f in p if f["dono"] == f'{b["assinatura"]}@{b["ini"]}']),
                ]
                for b in m["impls"]
                if not b["de_teste"]
            ],
        )
    )
    o.append("")
    o.append("O QUE AS FUNCOES TOCAM (producao)")
    cont = {}
    lin = {}
    for f in p:
        for d in f["toca"]:
            cont[d] = cont.get(d, 0) + 1
            lin[d] = lin.get(d, 0) + f["linhas"]
    o.append(
        tabela(
            ["dominio", "funcoes", "linhas"],
            sorted([[d, c, lin[d]] for d, c in cont.items()], key=lambda r: -r[2]),
        )
    )
    o.append("")
    o.append("QUANTOS DOMINIOS CADA FUNCAO ATRAVESSA")
    dist = {}
    for f in p:
        dist[len(f["toca"])] = dist.get(len(f["toca"]), 0) + 1
    o.append(
        tabela(
            ["dominios", "funcoes", "linhas"],
            [
                [k, v, sum(f["linhas"] for f in p if len(f["toca"]) == k)]
                for k, v in sorted(dist.items())
            ],
        )
    )
    o.append("")
    o.append("FAMILIAS")
    fam = {}
    for f in p:
        fam.setdefault(f["familia"], [0, 0])
        fam[f["familia"]][0] += 1
        fam[f["familia"]][1] += f["linhas"]
    o.append(
        tabela(
            ["familia", "funcoes", "linhas"],
            sorted([[k, v[0], v[1]] for k, v in fam.items()], key=lambda r: -r[2]),
        )
    )
    o.append("")
    o.append("AS 25 MAIORES FUNCOES")
    o.append(
        tabela(
            ["funcao", "linha", "linhas", "toca"],
            [
                [f["nome"], f["ini"], f["linhas"], ",".join(f["toca"]) or "-"]
                for f in sorted(p, key=lambda f: -f["linhas"])[:25]
            ],
        )
    )
    o.append("")
    o.append("CAMPOS DO `Servidor` POR NUMERO DE FUNCOES QUE OS LEEM")
    o.append(
        tabela(
            ["campo", "funcoes"],
            sorted([[c, n] for c, n in m["uso_por_campo"].items()], key=lambda r: -r[1]),
        )
    )
    o.append("")
    o.append("MODULOS DO CRATE QUE O `servidor.rs` ALCANCA")
    o.append(
        tabela(
            ["crate::", "funcoes"],
            sorted([[u, n] for u, n in m["usos_do_crate"].items() if n], key=lambda r: -r[1]),
        )
    )
    o.append("")
    o.append("AS REGIOES, E O QUE CADA UMA CUSTA PARA SAIR")
    o.append("(sai = metodos de fora que ela chama; entra = metodos dela que")
    o.append(" o resto chama; campos = campos do `Servidor` que ela toca)")
    o.append(
        tabela(
            ["regiao", "linhas", "fns", "sai", "entra", "campos", "imports", "trava"],
            [
                [r["nome"], r["linhas"], r["funcoes"], len(r["sai"]), len(r["entra"]),
                 len(r["campos"]), len(r["imports"]), "sim" if r["trava_de_dados"] else "nao"]
                for r in m["regioes"]
            ],
        )
    )
    o.append("")
    o.append(f"cobertura das regioes: {m['regioes_cobrem']} de "
             f"{len([f for f in p if f['dono'].startswith('impl Servidor')])} metodos"
             + (f"  FORA: {', '.join(m['fora_das_regioes'])}" if m["fora_das_regioes"] else ""))
    o.append("")
    o.append("AS PORTAS DE ENTRADA E O PORTAO QUE ALCANCAM")
    o.append(
        tabela(
            ["porta", "linha", "linhas", "portao", "via"],
            [
                [x["nome"], x["linha"], x["linhas"], x["portao"] or "NENHUM",
                 " > ".join(x["via"]) or "direto"]
                for x in m["portas"]
            ],
        )
    )
    o.append("")
    o.append("O CAMPO QUE O PORTAO LE, E QUEM NAO TEM ESSE CAMPO")
    o.append(f"  operacoes op_* .......................... {m['ops']}")
    o.append(f"  leem o campo \"tabela\" ................... {m['ops_com_campo_tabela']}")
    o.append(f"  nomeiam tabela por OUTRO campo .......... {len(m['ops_por_outro_campo'])}")
    o.append(f"  com conferencia PROPRIA de permissao .... {len(m['ops_com_conferencia_propria'])}")
    o.append(
        tabela(
            ["operacao", "linha", "campo do pedido", "tabela aninhada em", "o que e", "confere por conta"],
            [[x["nome"], x["linha"], ", ".join(x["campos"]) or "-",
              ", ".join(f'{r}.tabela' for r in x["aninhada_em"]) or "-",
              x["alvo"], "sim" if x["confere_por_conta"] else "NAO"]
             for x in m["ops_por_outro_campo"]],
        )
    )
    o.append("")
    o.append("ESTADO E CONSTANTES DE MODULO (o que um filho novo nao pode redeclarar)")
    o.append(
        tabela(
            ["nome", "declarado", "usado nas linhas", "regioes que atravessa"],
            [
                [g["nome"], g["declarado"],
                 ", ".join(str(u) for u in g["usos"]),
                 ", ".join(g["regioes"] + (["(fora de regiao)"] if g["fora_de_regiao"] else []))]
                for g in m["globais_de_modulo"]
            ],
        )
    )
    o.append("")
    o.append("MODULOS DE TESTE DENTRO DO ARQUIVO")
    o.append(
        tabela(
            ["modulo", "ini", "fim", "linhas", "privados que alcanca"],
            [[t["nome"], t["ini"], t["fim"], t["linhas"], ", ".join(t["privados"]) or "-"]
             for t in m["mods_teste"]],
        )
    )
    return "\n".join(o)



# ---------------------------------------------------------------- os blocos
def md(cab, linhas):
    """Tabela em Markdown."""
    out = ["| " + " | ".join(cab) + " |", "|" + "|".join("---" for _ in cab) + "|"]
    for l in linhas:
        out.append("| " + " | ".join(str(x) for x in l) + " |")
    return "\n".join(out)


def n(x):
    """Numero com ponto de milhar, como o resto da documentacao escreve."""
    return f"{x:,}".replace(",", ".")


def dec(x):
    """Decimal com virgula -- o documento e em portugues."""
    return f"{x:.1f}".replace(".", ",")


def blocos(m: dict) -> dict:
    """O que o gerador escreve DENTRO das marcas do documento.

    Tudo o que e numero vive aqui. A prosa fica FORA das marcas -- a licao do
    `cognicao_editei-dentro-do-bloco-que-o-gerador-reescreve`: o gerador
    substitui TUDO entre as marcas, entao texto escrito la dentro morre no
    proximo `--escrever`.
    """
    prod = m["producao"]
    do_serv = [f for f in prod if f["dono"].startswith("impl Servidor")]
    b = {}

    b["carimbo"] = (
        f"**Medido em** {m['medido_em']} — `servidor.rs` com **{n(m['linhas'])}** "
        f"linhas, sha256 `{m['sha256'][:16]}`, árvore em `{m['git']}`.\n\n"
        f"> A árvore é compartilhada: este arquivo cresceu **{n(m['linhas'])} − 22.560 = "
        f"{n(m['linhas'] - 22560)}** linhas desde o número do roteiro da SP000005.\n"
        f"> Se o `wc -l` de hoje não for {n(m['linhas'])}, **esta página envelheceu** — "
        f"rode o gerador de novo."
    )

    b["mapa"] = md(
        ["medida", "valor"],
        [
            ["linhas totais", f"**{n(m['linhas'])}**"],
            ["linhas de código", f"**{n(m['linhas_de_codigo'])}**"],
            ["linhas de teste", f"**{n(m['linhas_de_teste'])}**, em **{len(m['mods_teste'])}** módulos `#[cfg(test)]`"],
            ["funções de produção", f"**{len(prod)}**"],
            ["blocos `impl`", f"**{len([x for x in m['impls'] if not x['de_teste']])}**"],
            ["campos do `struct Servidor`", f"**{len(m['campos_do_servidor'])}**"],
            ["fatia do crate `phxsql-server`", f"**{m['fatia_do_crate']}%** de {n(m['linhas_do_crate'])} linhas"],
        ],
    )

    b["impls"] = md(
        ["bloco", "ini", "fim", "linhas", "fns"],
        [
            [f"`{x['assinatura']}`", x["ini"], x["fim"], n(x["linhas"]),
             len([f for f in prod if f["dono"] == f"{x['assinatura']}@{x['ini']}"])]
            for x in m["impls"] if not x["de_teste"]
        ],
    )

    cont, lin = {}, {}
    for f in prod:
        for d in f["toca"]:
            cont[d] = cont.get(d, 0) + 1
            lin[d] = lin.get(d, 0) + f["linhas"]
    b["dominios"] = md(
        ["domínio", "funções", "linhas"],
        sorted([[d, c, n(lin[d])] for d, c in cont.items()], key=lambda r: -lin[r[0]]),
    )

    dist = {}
    for f in prod:
        dist[len(f["toca"])] = dist.get(len(f["toca"]), 0) + 1
    b["atravessa"] = md(
        ["domínios", "funções", "linhas"],
        [[k, v, n(sum(f["linhas"] for f in prod if len(f["toca"]) == k))]
         for k, v in sorted(dist.items())],
    )
    magras = [f for f in prod if len(f["toca"]) <= 1]
    gordas = [f for f in prod if len(f["toca"]) >= 4]
    b["atravessa-resumo"] = (
        f"**{len(magras)} das {len(prod)} funções ({round(100 * len(magras) / len(prod))}%) "
        f"tocam zero ou um domínio** — {n(sum(f['linhas'] for f in magras))} linhas que já "
        f"estão prontas para sair. As que atravessam quatro ou mais são **{len(gordas)}**, "
        f"e somam {n(sum(f['linhas'] for f in gordas))} linhas."
    )

    b["regioes"] = md(
        ["região", "linhas", "fns", "sai", "entra", "campos", "imports", "trava"],
        [[r["nome"], n(r["linhas"]), r["funcoes"], len(r["sai"]), len(r["entra"]),
          len(r["campos"]), len(r["imports"]), "sim" if r["trava_de_dados"] else "não"]
         for r in m["regioes"]],
    )
    b["regioes-cobertura"] = (
        f"As {len(m['regioes'])} regiões cobrem **{m['regioes_cobrem']} de {len(do_serv)}** métodos"
        + (f" — **FORA: {', '.join(m['fora_das_regioes'])}**" if m["fora_das_regioes"] else
           " (nenhum método num vão).")
    )

    # a fatia mais barata, escolhida pelo NUMERO e nao pela mao
    cand = [r for r in m["regioes"] if r["nome"] not in ("operacoes-de-dados", "portoes-e-despacho",
                                                          "erros-de-resposta", "arranque-e-identidade")]
    for r in cand:
        r["_trav"] = len(r["sai"]) + len(r["entra"])
        r["_razao"] = round(r["linhas"] / max(r["_trav"], 1), 1)
    ordem = sorted(cand, key=lambda r: -r["_razao"])
    b["custo-por-fatia"] = md(
        ["região", "linhas", "travessias (sai+entra)", "linhas por travessia", "campos", "imports"],
        [[("**" + r["nome"] + "**") if r is ordem[0] else r["nome"], n(r["linhas"]),
          r["_trav"], dec(r["_razao"]), len(r["campos"]), len(r["imports"])] for r in ordem],
    )
    v = ordem[0]
    b["fatia-mais-barata"] = (
        f"**`{v['nome']}` — {n(v['linhas'])} linhas, {v['funcoes']} funções, "
        f"`servidor.rs:{v['ini']}-{v['fim']}`.** Paga {v['_trav']} travessias "
        f"({len(v['sai'])} saindo, {len(v['entra'])} entrando), toca **{len(v['campos'])}** "
        f"dos {len(m['campos_do_servidor'])} campos do `Servidor` "
        f"(`{'`, `'.join(v['campos'])}`) e precisa de {len(v['imports'])} dos "
        f"{m['imports_no_topo']} imports do topo — a melhor razão "
        f"**{dec(v['_razao'])} linhas por travessia** de todas as regiões candidatas.\n\n"
        f"As travessias, nomeadas: ela chama `{'`, `'.join(v['sai'])}`; "
        f"é chamada em `{'`, `'.join(v['entra'])}`."
    )

    b["portas"] = md(
        ["porta", "linha", "linhas", "portão", "via"],
        [[f"`{x['nome']}`", x["linha"], x["linhas"], f"`{x['portao']}`" if x["portao"] else "**NENHUM**",
          " > ".join(f"`{v}`" for v in x["via"]) or "direto"] for x in m["portas"]],
    )

    reais = [x for x in m["ops_por_outro_campo"] if x["alvo"] == "tabela"]
    b["portao-numeros"] = md(
        ["", ""],
        [["operações `op_*`", f"**{m['ops']}**"],
         ["leem o campo `\"tabela\"` **do pedido**", f"**{m['ops_com_campo_tabela']}**"],
         ["nomeiam tabela por **outro** campo", f"**{len(m['ops_por_outro_campo'])}** "
          f"(**{len(reais)}** são tabela de verdade)"],
         ["carregam conferência **própria** de permissão", f"**{len(m['ops_com_conferencia_propria'])}**"]],
    )
    b["portao-furos"] = md(
        ["operação", "linha", "campo do pedido", "tabela aninhada em", "o que é", "confere por conta"],
        [[f"`{x['nome']}`", x["linha"], ", ".join(f"`{c}`" for c in x["campos"]) or "—",
          ", ".join(f"`{r}.tabela`" for r in x["aninhada_em"]) or "—",
          "tabela" if x["alvo"] == "tabela" else "**caminho de arquivo**",
          "sim" if x["confere_por_conta"] else "não precisa"]
         for x in m["ops_por_outro_campo"]],
    )

    b["globais"] = md(
        ["nome", "declarado", "usado nas linhas", "regiões que atravessa"],
        [[f"`{g['nome']}`", g["declarado"], ", ".join(str(u) for u in g["usos"]),
          ", ".join(g["regioes"] + (["**fora de toda região**"] if g["fora_de_regiao"] else [])) or "—"]
         for g in m["globais_de_modulo"] if not g["so_em_teste"]],
    )

    b["testes"] = md(
        ["módulo", "linhas", "privados que alcança"],
        [[f"`{t['nome']}`", n(t["linhas"]), ", ".join(f"`{x}`" for x in t["privados"]) or "—"]
         for t in m["mods_teste"]],
    )
    com_priv = [t for t in m["mods_teste"] if t["privados"]]
    b["testes-resumo"] = (
        f"**{n(m['linhas_de_teste'])} linhas em {len(m['mods_teste'])} módulos** vivem dentro do "
        f"arquivo — {round(100 * m['linhas_de_teste'] / m['linhas'])}% dele. E eles não são "
        f"caixa-preta: **{len(com_priv)} dos {len(m['mods_teste'])}** alcançam pelo menos um item "
        f"privado do módulo."
    )

    b["externos"] = md(
        ["arquivo", "usa"],
        [[f"`{x['arquivo']}:{x['linha']}`", f"`{x['item']}`"] for x in m["referencias_externas"]],
    )
    itens = sorted({x["item"] for x in m["referencias_externas"]})
    b["externos-resumo"] = (
        f"Apenas **{len({x['arquivo'] for x in m['referencias_externas']})} arquivos** fora do "
        f"módulo citam `servidor::`, e entre eles usam **{len(itens)}** itens: "
        f"`{'`, `'.join(itens)}`. A superfície pública real do `servidor.rs` são esses "
        f"{len(itens)} nomes — uma divisão em filhos preserva os três sem tocar em nenhum.\n\n"
        f"`crates/phxsql-server/tests/` tem **{m['baterias_de_fora']}** baterias com "
        f"**{n(m['linhas_de_teste_de_fora'])}** linhas ao todo, e "
        f"**{m['baterias_sem_o_modulo']} delas** falam com o servidor **pelo soquete** sem citar "
        f"o módulo. São a rede de segurança da sprint."
    )
    return b


MARCA_INI = "<!-- mapa:{}:inicio -->"
MARCA_FIM = "<!-- mapa:{}:fim -->"


def escrever(doc: Path, m: dict) -> list:
    """Substitui o conteudo entre as marcas. Prosa fora delas sobrevive."""
    txt = doc.read_text(encoding="utf-8")
    trocados = []
    for nome, corpo in blocos(m).items():
        ini, fim = MARCA_INI.format(nome), MARCA_FIM.format(nome)
        i, j = txt.find(ini), txt.find(fim)
        if i < 0 or j < 0:
            continue
        txt = txt[: i + len(ini)] + "\n" + corpo + "\n" + txt[j:]
        trocados.append(nome)
    doc.write_text(txt, encoding="utf-8")
    return trocados


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    alvo = Path(args[0]) if args else PADRAO
    med = medir(alvo)
    if "--json" in sys.argv:
        print(json.dumps(med, ensure_ascii=False, indent=1))
    elif "--escrever" in sys.argv:
        doc = Path(args[1]) if len(args) > 1 else RAIZ / "docs/FRONTEIRAS-DO-SERVIDOR.md"
        t = escrever(doc, med)
        print(f"{doc.name}: {len(t)} blocos reescritos ({', '.join(t)})")
    else:
        try:
            print(relatorio(med))
        except BrokenPipeError:
            # `| head` fecha o cano; nao e erro do gerador.
            sys.stdout = None
