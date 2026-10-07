#!/usr/bin/env python3
"""A catraca das LISTAS de dispensa -- pedido 654.

    python3 bancada/guardas/listas-de-dispensa.py --catraca
    python3 bancada/guardas/listas-de-dispensa.py --numeros
    python3 bancada/guardas/listas-de-dispensa.py --autoteste

# O defeito que motivou

O `TETO_BOTAO_SEM_PROVA` chegou a zero em 02/10/2026, e chegou em parte
porque o `DISPENSADOS` do `conferidor_botoes.rs` cresceu de 22 para 27. Cada
dispensa tem o motivo escrito -- mas a LISTA nao tinha teto nem piso: bastava
acrescentar uma linha para tirar um botao da conta sem tocar na catraca. E o
mesmo valia para o `ISENTOS` do `conferidor.rs` (textos que nao se traduzem),
estavel em 81 so porque ninguem tinha precisado dele ainda. A lei de G -- a
catraca so desce -- vale tambem para a lista que decide o que a catraca conta,
senao a lista e a porta dos fundos dela.

# O que esta regua conta

* `TETO_DISPENSADOS_DE_BOTAO` -- as entradas do `DISPENSADOS`. So desce.
* `TETO_ISENTOS_DE_TRADUCAO` -- as entradas do `ISENTOS`. So desce.
* `TETO_DISPENSA_SEM_MOTIVO` -- entradas das duas listas com o motivo vazio.
  Zero, porque dispensa sem motivo e dispensa silenciosa.
* `TETO_DISPENSA_VENCIDA` -- botao do `DISPENSADOS` cuja chave a bateria JA
  clica (`testes-web/botoes-exercitados.txt`): a razao da dispensa deixou de
  ser verdade, e a linha so serve para esconder que o botao entrou na conta.
  Zero.

# Por que em Python, lendo o fonte Rust, e nao um teste no conferidor

Porque a pergunta e sobre a LISTA escrita no fonte, e nao sobre o que o
conferidor MEDE -- esta regua nao decide quem e botao nem quem esta provado,
so conta as linhas que alguem digitou e confere o motivo delas. Nao ha
segunda receita de nada. E o custo decide o resto: a conferencia em Rust
pediria compilar o `phxsql-server` inteiro (medido na copia do provador:
2,3 GB de `target` frio), contra ~0,05 s aqui.

A conferencia da dispensa vencida compara a chave EXATA com a evidencia. O
`provado()` do conferidor tambem aceita `[data-x]` clicado por um
`[data-x="v"]` interpolado -- esta regua NAO repete essa decisao (seria a
segunda copia dela): ela mede menos que o conferidor nesse caso, e o diz aqui.
"""
import os
import re
import sys

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
EU = os.path.relpath(os.path.abspath(__file__), RAIZ)

BOTOES = "crates/phxsql-server/src/conferidor_botoes.rs"
TEXTOS = "crates/phxsql-server/src/conferidor.rs"
EVIDENCIA = "testes-web/botoes-exercitados.txt"

# Nasceram no numero medido do dia (07/10/2026). So descem: quem tirar uma
# dispensa baixa o teto no mesmo commit; quem acrescentar uma reprova, e o
# caminho honesto passa a ser escrever o caso que exercita o botao.
TETO_DISPENSADOS_DE_BOTAO = 27
TETO_ISENTOS_DE_TRADUCAO = 81
TETO_DISPENSA_SEM_MOTIVO = 0
TETO_DISPENSA_VENCIDA = 0


def lista_rust(texto, nome):
    """As tuplas de literais de `pub const <nome>: ... = &[ ... ];`.

    Um analisador de verdade, e nao um `re.findall` de aspas: o motivo das
    dispensas atravessa linhas com `\\` + quebra, traz `\\"` dentro, e a
    lista tem comentarios `//` com aspas no meio. Recortar por padrao de texto
    contaria a aspa do comentario como um motivo. Devolve `None` quando a
    constante nao existe -- regua que nao acha a lista nao pode dizer zero."""
    m = re.search(r"\bconst\s+%s\s*:[^=]*=\s*&\[" % re.escape(nome), texto)
    if not m:
        return None
    i, n = m.end(), len(texto)
    tuplas, atual, prof = [], None, 0
    while i < n:
        c = texto[i]
        if texto.startswith("//", i):
            fim = texto.find("\n", i)
            i = n if fim == -1 else fim + 1
            continue
        if texto.startswith("/*", i):
            fim = texto.find("*/", i)
            i = n if fim == -1 else fim + 2
            continue
        if c == '"':
            i += 1
            pedacos = []
            while i < n and texto[i] != '"':
                if texto[i] == "\\":
                    prox = texto[i + 1] if i + 1 < n else ""
                    if prox == "\n":
                        # continuacao: o Rust come a quebra e o recuo
                        i += 2
                        while i < n and texto[i] in " \t\r\n":
                            i += 1
                        continue
                    pedacos.append({"n": "\n", "t": "\t", "0": "\0"}.get(prox, prox))
                    i += 2
                    continue
                pedacos.append(texto[i])
                i += 1
            i += 1
            if atual is not None:
                atual.append("".join(pedacos))
            continue
        if c == "(":
            prof += 1
            if prof == 1:
                atual = []
        elif c == ")":
            if prof == 1 and atual is not None:
                tuplas.append(tuple(atual))
                atual = None
            prof -= 1
        elif c == "]" and prof == 0:
            return tuplas
        i += 1
    return tuplas


def ler(rel):
    with open(os.path.join(RAIZ, rel), encoding="utf-8", errors="replace") as f:
        return f.read()


def exercitados(texto):
    """O mesmo crivo do `exercitados()` do conferidor: linha nao vazia que
    nao comeca por `//`. E leitura do arquivo, nao decisao sobre ele."""
    return {l.strip() for l in texto.splitlines()
            if l.strip() and not l.strip().startswith("//")}


def medir(fonte_botoes, fonte_textos, evidencia):
    """Os quatro numeros e as listas que os explicam. Recebe TEXTO, e nao
    caminho, para que o autoteste o alimente com fonte sintetica."""
    dispensados = lista_rust(fonte_botoes, "DISPENSADOS")
    isentos = lista_rust(fonte_textos, "ISENTOS")
    avisos = []
    if dispensados is None:
        avisos.append("nao achei `const DISPENSADOS` em %s" % BOTOES)
        dispensados = []
    if isentos is None:
        avisos.append("nao achei `const ISENTOS` em %s" % TEXTOS)
        isentos = []
    if evidencia is None:
        avisos.append("%s nao existe: sem evidencia nao da para dizer se a "
                      "razao de uma dispensa ainda vale" % EVIDENCIA)
        clicados = set()
    else:
        clicados = exercitados(evidencia)
    sem_motivo = []
    for t in dispensados:
        if len(t) < 3 or not t[2].strip():
            sem_motivo.append("DISPENSADOS %s" % (t[1] if len(t) > 1 else t))
    for t in isentos:
        if len(t) < 2 or not t[1].strip():
            sem_motivo.append("ISENTOS %r" % (t[0] if t else t))
    vencidas = [t[1] for t in dispensados
                if len(t) > 1 and not t[1].startswith("fn:") and t[1] in clicados]
    return {
        "TETO_DISPENSADOS_DE_BOTAO": len(dispensados),
        "TETO_ISENTOS_DE_TRADUCAO": len(isentos),
        "TETO_DISPENSA_SEM_MOTIVO": len(sem_motivo),
        "TETO_DISPENSA_VENCIDA": len(vencidas),
    }, {"sem_motivo": sem_motivo, "vencidas": vencidas, "avisos": avisos}


def medir_a_arvore():
    try:
        evid = ler(EVIDENCIA)
    except OSError:
        evid = None
    return medir(ler(BOTOES), ler(TEXTOS), evid)


AS_CATRACAS = [
    ("TETO_DISPENSADOS_DE_BOTAO", TETO_DISPENSADOS_DE_BOTAO,
     "botoes dispensados de prova no DISPENSADOS do conferidor de botoes",
     "a lista de dispensas CRESCEU: e a porta dos fundos do "
     "TETO_BOTAO_SEM_PROVA. Escreva o caso que exercita o botao; se ele "
     "provadamente nao se exercita, a decisao de subir a lista e do QA com o "
     "numero, e entao a catraca se APOSENTA e nasce outra -- nunca sobe."),
    ("TETO_ISENTOS_DE_TRADUCAO", TETO_ISENTOS_DE_TRADUCAO,
     "textos isentos de traducao no ISENTOS do conferidor de idiomas",
     "a lista de isentos CRESCEU: texto que so ainda nao foi traduzido nao "
     "entra ali, entra na fabrica."),
    ("TETO_DISPENSA_SEM_MOTIVO", TETO_DISPENSA_SEM_MOTIVO,
     "entradas do DISPENSADOS e do ISENTOS com o motivo vazio",
     "dispensa sem motivo e dispensa silenciosa: escreva por que."),
    ("TETO_DISPENSA_VENCIDA", TETO_DISPENSA_VENCIDA,
     "botoes dispensados cuja chave a bateria ja clica",
     "a razao da dispensa deixou de ser verdade -- a bateria clica o botao. "
     "Tire a linha do DISPENSADOS e baixe o TETO_DISPENSADOS_DE_BOTAO."),
]


def julgar(agora, extra, catracas=None):
    """Imprime e devolve 1 se alguma nao segura. Teto frouxo tambem reprova:
    catraca frouxa nao segura nada."""
    ruim = 0
    for nome, valor, _mede, recado in (catracas or AS_CATRACAS):
        atual = agora[nome]
        if atual > valor:
            print(f"   SUBIU  {nome}: {atual} (teto {valor})")
            print(f"   Reprovado: {recado}")
            ruim = 1
        elif atual < valor:
            print(f"   DESCEU -- BAIXE O TETO  {nome}: {atual} (teto {valor})")
            print(f"   Melhorou. Ponha {atual} em {nome}, no mesmo commit.")
            ruim = 1
        else:
            print(f"   ok  {nome}: {atual} (teto {valor})")
    for s in extra["sem_motivo"]:
        print(f"      sem motivo: {s}")
    for v in extra["vencidas"]:
        print(f"      dispensa vencida: {v} esta em {EVIDENCIA}")
    for a in extra["avisos"]:
        print(f"   NAO DA PARA MEDIR  {a}")
        ruim = 1
    return ruim


def catraca():
    print("=== a catraca das listas de dispensa (pedido 654) ===")
    agora, extra = medir_a_arvore()
    return julgar(agora, extra)


def numeros():
    agora, _ = medir_a_arvore()
    for nome, valor, mede, _r in AS_CATRACAS:
        print(f"catraca:nome={nome};onde={EU};valor={valor};"
              f"medido={agora[nome]};tipo=teto;mede={mede}")
    return 0


def autoteste():
    """Prova real nos dois sentidos, com fonte sintetica e com a de verdade.

    O defeito reposto e o que o pedido nomeia: uma dispensa nova sem baixar o
    teto, um motivo removido, e a razao que deixou de valer."""
    falhas = []

    def conferir(nome, cond, detalhe=""):
        print("   %s  %s%s" % ("ok  " if cond else "FALHOU", nome,
                               "" if cond else "  -- " + detalhe))
        if not cond:
            falhas.append(nome)

    real_b, real_t = ler(BOTOES), ler(TEXTOS)
    try:
        evid = ler(EVIDENCIA)
    except OSError:
        evid = ""
    agora, extra = medir(real_b, real_t, evid)
    conferir("a arvore de hoje mede o que os tetos dizem",
             all(agora[n] == v for n, v, _m, _r in AS_CATRACAS)
             and not extra["avisos"], str(agora) + str(extra["avisos"]))

    # 1. Dispensa NOVA sem baixar o teto: a lista cresce, a catraca reprova.
    nova = real_b.replace(
        "pub const DISPENSADOS: &[(&str, &str, &str)] = &[",
        "pub const DISPENSADOS: &[(&str, &str, &str)] = &[\n"
        "    (\"ui/index.html\", \"#btNovo\", \"chato de clicar\"),", 1)
    a, _ = medir(nova, real_t, evid)
    conferir("dispensa nova sem baixar o teto: SUBIU",
             a["TETO_DISPENSADOS_DE_BOTAO"] == TETO_DISPENSADOS_DE_BOTAO + 1,
             str(a))
    import io
    import contextlib
    with contextlib.redirect_stdout(io.StringIO()):
        conferir("e o julgamento reprova", julgar(a, {"sem_motivo": [],
                 "vencidas": [], "avisos": []}) == 1)

    # 2. Motivo removido: a entrada continua, o motivo some.
    sem = real_b.replace(
        "\"#btClAdd\",\n        \"irmao do `#btClVer`: so existe com cluster "
        "configurado. Exercitado em \\\n",
        "\"#btClAdd\",\n        \"\",\n        \"", 1)
    conferir("o recorte do motivo achou o lugar (controle do proprio teste)",
             sem != real_b)
    a, e = medir(sem, real_t, evid)
    conferir("motivo removido: TETO_DISPENSA_SEM_MOTIVO acusa",
             a["TETO_DISPENSA_SEM_MOTIVO"] >= 1, str(e["sem_motivo"]))
    isento_vazio = real_t.replace('("PhxSql", "a marca")', '("PhxSql", "  ")', 1)
    conferir("o recorte do isento achou o lugar", isento_vazio != real_t)
    a, e = medir(real_b, isento_vazio, evid)
    conferir("isento com motivo em branco: acusa",
             a["TETO_DISPENSA_SEM_MOTIVO"] == 1, str(e["sem_motivo"]))

    # 3. A razao que deixou de ser verdade: a bateria passou a clicar.
    a, e = medir(real_b, real_t, evid + "\n#rzIr3\n")
    conferir("botao dispensado que a bateria clica: dispensa vencida",
             e["vencidas"] == ["#rzIr3"], str(e["vencidas"]))
    # controle: o comentario da evidencia nao e clique
    a, e = medir(real_b, real_t, evid + "\n// #rzIr3\n")
    conferir("linha comentada na evidencia nao vence a dispensa",
             e["vencidas"] == [], str(e["vencidas"]))
    # controle: `fn:` nao tem chave, nunca casa com a evidencia
    a, e = medir(real_b, real_t, evid + "\nfn:desenharCartao\n")
    conferir("a dispensa `fn:` (sem chave) nao se da por vencida",
             e["vencidas"] == [], str(e["vencidas"]))

    # 4. O analisador: aspa no comentario nao vira motivo, `\"` nao corta.
    sint = ('pub const DISPENSADOS: &[(&str, &str, &str)] = &[\n'
            '    // um comentario com "aspas" no meio\n'
            '    ("a", "#x", "com \\"aspa\\" e \\\n          continuacao"),\n'
            '];\n')
    t = lista_rust(sint, "DISPENSADOS")
    conferir("o analisador le uma tupla so, com a aspa escapada e a continuacao",
             t == [("a", "#x", 'com "aspa" e continuacao')], str(t))
    a, e = medir("nada aqui", real_t, evid)
    conferir("lista ausente: aviso escrito, nunca um zero calado",
             len(e["avisos"]) == 1, str(e["avisos"]))

    print("   %s" % ("todos passaram" if not falhas
                     else "FALHOU: " + ", ".join(falhas)))
    return 1 if falhas else 0


def principal():
    if "--autoteste" in sys.argv:
        print("=== autoteste da catraca das listas de dispensa (pedido 654) ===")
        return autoteste()
    if "--catraca" in sys.argv:
        return catraca()
    if "--numeros" in sys.argv:
        return numeros()
    agora, extra = medir_a_arvore()
    for nome, valor in agora.items():
        print(f"{nome} = {valor}")
    return 0


if __name__ == "__main__":
    sys.exit(principal())
