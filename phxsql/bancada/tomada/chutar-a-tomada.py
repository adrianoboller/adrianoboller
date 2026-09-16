#!/usr/bin/env python3
"""Chutar a tomada: `SIGKILL` nos pontos que ninguem varria.

    cargo build --release
    python3 bancada/tomada/chutar-a-tomada.py

Pedido do dono, literal: «O teste de chutar a tomada e importante numa
transaction e num bulk-insert.»

# O que a casa JA tinha, e onde esta bancada entra

`bancada/transacoes/provar.py` SS7 mata no meio de UM commit; `bancada/acid/`
varre atrasos no meio de um commit de duas tabelas (A2) e no meio da cascata
(A4), e conta `fsync` (D). Nenhuma delas chuta a tomada:

  1. DENTRO de uma transacao ABERTA, antes do COMMIT -- inserir, atualizar,
     excluir, SAVEPOINT, ROLLBACK TO, com a queda no meio;
  2. no meio de um BULKINSERT (reserva, linhas, soltura);
  2c. no meio de um `reindexar` -- que e o conserto que a queda de (2) exige;
  3. num BULKINSERT dentro de uma transacao;
  4. logo DEPOIS do «ok» -- do COMMIT e do BULKINSERT(false) -- com a
     contagem de `fsync` antes do «ok», por `strace`.

Cada ponto tem o RESULTADO ESPERADO escrito antes de rodar (o dicionario
`ESPERADO` abaixo, copiado para o `resultados.json`), uma varredura de dezenas
de atrasos, e pelo menos tres rodadas por atraso. O que se mede e o DESFECHO,
nunca o tempo: a calibracao que decide a faixa de atrasos e meio, nao
resultado.

# O que o codigo PROMETE, lido antes de afirmar

* Transacao aberta: nada vai a disco antes do COMMIT (`transacao.rs` so toca o
  disco em `gravar_marca`, chamada pelo `op_commit`). Uma queda antes do
  COMMIT tem de deixar ZERO rastro: contagem, slots e linhas iguais as de
  antes do BEGIN, nenhuma marca `.tx`, e o arranque calado -- o relatorio
  `PHXSQL Recovery` so sai quando ha marca.
* BULKINSERT (`carga.rs`, `op_bulkinsert`): a reserva mora na MEMORIA
  (`Cargas`), e o que a reserva compra e a janela de durabilidade aberta: cada
  `inserir` continua escrevendo `.reg` e `.ndx` na hora, e o `fsync` fica
  para o `bulkinsert(false)`. Entao a semantica sob queda de PROCESSO e: as
  linhas confirmadas ficam (o `write` ja foi entregue ao nucleo, e SIGKILL nao
  esvazia o cache dele -- e a lei da secao D do ACID), mais no maximo UMA em
  voo; o `.ndx` fica com a marca de sujo (byte 52) e recusa toda operacao ate
  um `reindexar`; e o arranque NAO diz nada, porque nao ha marca `.tx` -- a
  recuperacao (`transacao::recuperar`) so reconstroi indice de tabela NOMEADA
  numa marca. A reserva volta solta por construcao: memoria de processo morto.
  Isso NAO e durabilidade: so o ponto 4 (fsync antes do «ok») e.
* `reindexar` (`table.rs`): `NdxFile::criar` trunca o `.ndx` e grava um
  cabecalho LIMPO com arvore vazia; depois varre o `.reg` inteiro; so entao
  `construir_em_lote` grava paginas (por `gravar_pagina`, que levanta a marca
  de sujo antes da primeira). HIPOTESE, escrita antes de medir: uma queda
  entre o `criar` e a primeira pagina deixa indice vazio e LIMPO -- `buscar`
  cala, `inserir` aceita chave repetida, e so `verificar` acusa («indice pk
  tem 0 chaves para N registros»). Se a varredura acertar essa janela, e
  achado; se nao acertar, fica registrado como nao alcancado.
* BULKINSERT dentro de transacao: `op_bulkinsert` nao olha a transacao da
  sessao, entao deve aceitar. Os `inserir` empilham; o COMMIT grava; como a
  tabela esta reservada a janela nao fecha e a marca `.tx` fica PENDENTE
  (`marcas_pendentes`). HIPOTESE: `bulkinsert(false)` sincroniza a tabela
  mas nao drena `marcas_pendentes` -- a marca de um commit ja duravel fica no
  disco ate a proxima drenagem, e uma queda depois do «ok» faz o arranque
  reportar «transacoes achadas 1 / ja aplicadas N». Nao perde dado; e ruido.
  CONFIRMADA em 16/09/2026 (3/3 corridas `APOS_BULKINSERT_FALSE_MARCA_REPORTADA`,
  a marca no disco 300 ms depois do «ok») e consertada no pedido 254: o
  `bulkinsert(false)` passou a chamar a MESMA drenagem do fecho da janela.
  Desde entao o ponto e VEREDITO, nao linha informativa: marca depois do «ok»
  final reprova, e `bancada/tomada/marca-apos-bulkinsert.py` mede so ele.
* Depois do «ok»: o COMMIT e duravel pela marca `.tx` (fsync incondicional em
  `gravar_marca`), e o `.reg`/`.ndx` nao vao ao disco com a janela aberta; o
  `bulkinsert(false)` e o contrario: `.reg` e `.ndx` sincronizados, marca
  nenhuma. A contagem de `fsync` por extensao e o que separa os dois.

# Portao, vizinhos e limpeza

Consulta `bancada/esta-medindo.sh` antes de comecar e ESPERA (ate 20 min, de
30 em 30 s) enquanto houver bancada de tempo em curso -- esta nao mede tempo,
mas a outra mede, e a tomada chutada aqui e carga la. Compilacao em curso e
registrada e nao bloqueia. Mata SO os PIDs que ela mesma subiu, nunca `pkill`.
E ela e detectavel pelo mesmo portao: o caminho do script casa `bancada/*.py`.

Reuso, e nao terceira copia: subir, matar, `Ligacao`, relatorio e marcas vem
de `bancada/durabilidade/prova.py`; o `strace` vem de `prova-do-fecho.py`.
"""
import importlib.util
import json
import os
import shutil
import socket
import subprocess
import sys
import threading
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.environ.get("PHX_RAIZ", os.path.abspath(os.path.join(AQUI, "..", "..")))

sys.path.insert(0, os.path.join(RAIZ, "bancada", "durabilidade"))
import prova as dur  # noqa: E402

# Porta propria, fora das que as outras bancadas usam (7100, 7320, 7450..7570,
# 7731, 7741, 6xxx, 5xxx): nenhuma esta na faixa 76xx.
dur.PORTA = int(os.environ.get("PHX_TOMADA_PORTA", "7610"))
dur.BASE = os.path.join(AQUI, ".base-da-prova")
dur.DB = "tomada"
PORTA, BASE, DB = dur.PORTA, dur.BASE, dur.DB
RESULTADO = os.path.join(AQUI, "resultados.json")

_spec = importlib.util.spec_from_file_location(
    "fecho", os.path.join(RAIZ, "bancada", "durabilidade", "prova-do-fecho.py"))
fecho = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(fecho)

# `dur.Ligacao` fixa a porta no DEFAULT do `__init__`, avaliado na definicao
# da classe -- a mesma armadilha que o ACID pagou. Rebindada no modulo.


class _NaMinhaPorta(dur.Ligacao):
    def __init__(self, porta=None, prazo=30):
        super().__init__(porta or PORTA, prazo)


dur.Ligacao = _NaMinhaPorta
Ligacao = _NaMinhaPorta

# Atalho de DEPURACAO: poucos atrasos, uma rodada, resultado NAO publicado.
RAPIDO = os.environ.get("PHX_TOMADA_RAPIDO") == "1"
N_ATRASOS = int(os.environ.get("PHX_TOMADA_ATRASOS", "4" if RAPIDO else "24"))
N_ATRASOS_REINDEX = int(os.environ.get("PHX_TOMADA_ATRASOS_REINDEX", "6" if RAPIDO else "40"))
RODADAS = int(os.environ.get("PHX_TOMADA_RODADAS", "1" if RAPIDO else "3"))

N_TX_BASE = 200      # linhas gravadas ANTES do BEGIN (o estado que tem de voltar)
N_BULK = 2000        # linhas de um BULKINSERT
N_BULK_TX = 800      # linhas do BULKINSERT dentro da transacao
N_REINDEX = 10_000   # linhas da tabela cujo `reindexar` leva a tomada
N_APOS = 500         # linhas dos controles «logo depois do ok»

ESPERADO = {
    "tx_aberta": (
        "zero rastro: registros e slots iguais aos de antes do BEGIN, as 20 "
        "linhas atualizadas com o valor antigo, as 10 excluidas ainda la, "
        "nenhuma chave da transacao encontravel, nenhuma marca .tx, arranque "
        "calado (sem bloco PHXSQL Recovery), verificar limpo -- em TODOS os "
        "atrasos, porque nada vai a disco antes do COMMIT"),
    "bulk": (
        "antes da reserva: 0 linhas e indice limpo. No meio: registros = "
        "confirmadas ou confirmadas+1 (uma em voo), nenhuma duplicata; o .ndx "
        "pode estar SUJO e recusar com «reparar indice» -- nunca atrasado em "
        "silencio --, e depois do reindexar toda linha visivel e achada pela "
        "chave; arranque calado; reserva solta. Depois do ok final: as "
        f"{N_BULK} linhas, indice limpo sem reindexar. Nuance lida no "
        "servidor: `abrir_travada` abre a tabela a CADA pedido e o `fechar` "
        "do fim do pedido leva as paginas ao nucleo, entao com `inserir` "
        "linha a linha a marca de sujo so fica levantada se a tomada cair "
        "DENTRO de um pedido -- janela de microssegundos"),
    "bulk_lote": (
        f"o mesmo BULKINSERT em 4 `inserir_lote` de {N_BULK // 4}: um pedido "
        "so segura a tabela aberta o lote inteiro, e e ai que o write-back "
        "do .ndx fica exposto. No meio: registros entre confirmadas e "
        f"confirmadas+{N_BULK // 4} (o lote NAO e atomico fora de transacao), "
        "nenhuma duplicata, .ndx SUJO detectado (byte 52 = 1) e recusando ate "
        "o reindexar, arranque calado, reserva solta"),
    "tx_em_bulk": (
        f"reserva primeiro, BEGIN dentro, {N_BULK_TX} inserir, COMMIT, "
        "bulkinsert(false) -- a ordem que o portao nao recusa. Registros 0 ou "
        f"{N_BULK_TX}, nunca no meio; indice de pe depois da recuperacao; "
        "nenhuma marca sobra. HIPOTESE: a tabela reservada nao fecha a janela "
        "no COMMIT, a marca fica PENDENTE, e bulkinsert(false) sincroniza a "
        "tabela sem drenar `marcas_pendentes` -- a marca de um commit ja "
        "duravel fica no disco, e a tomada depois do ok final faz o arranque "
        "reportar «achadas 1 / ja aplicadas N». Hipotese CONFIRMADA em "
        "16/09/2026 e consertada (pedido 254): hoje o esperado e marca "
        "NENHUMA depois do ok do bulkinsert(false), e o arranque calado"),
    "reindexar": (
        "ou o indice esta INTEIRO (queda antes de comecar ou depois de "
        "terminar), ou esta SUJO e recusa ate o reindexar. HIPOTESE de "
        "achado: uma queda entre o criar() e a primeira pagina deixa indice "
        "VAZIO e LIMPO, em que buscar cala e so verificar acusa"),
    "bulk_em_tx": (
        f"o motor aceita bulkinsert dentro da transacao; registros e 0 ou "
        f"{N_BULK_TX}, nunca no meio; o indice responde depois da recuperacao "
        "(a marca .tx nomeia a tabela e a recuperacao a reconstroi); nenhuma "
        "marca sobra. HIPOTESE: com bulkinsert(false) dado, a marca do commit "
        "ja sincronizado continua no disco e o arranque a reporta como "
        "«ja aplicadas»"),
    "apos_ok": (
        f"COMMIT de {N_APOS} linhas + SIGKILL logo apos o ok: as {N_APOS} la, "
        "com a marca .tx reportada pelo arranque (janela aberta = marca "
        f"pendente). BULKINSERT de {N_APOS} + SIGKILL logo apos o ok do "
        f"bulkinsert(false): as {N_APOS} la, indice limpo, arranque calado. "
        "fsync antes do ok: o COMMIT sincroniza o .tx e nao o .reg/.ndx; o "
        "bulkinsert(false) sincroniza o .reg e o .ndx e marca nenhuma"),
}

CONFERENCIAS = []
FALHAS = []


def ok(nome, condicao, detalhe=""):
    CONFERENCIAS.append({"nome": nome, "passou": bool(condicao), "detalhe": detalhe})
    if not condicao:
        FALHAS.append(f"{nome}: {detalhe}")
    print(("  OK    " if condicao else "  FALHA ") + nome + (f"  -- {detalhe}" if detalhe else ""))


# ----------------------------------------------------------------- servidor

def config():
    # Janela de durabilidade que NUNCA fecha sozinha (1 h / 1.000.000 op.):
    # e o que deixa o `.ndx` no write-back e a marca pendente -- o estado que
    # uma tomada chutada encontra numa carga longa de verdade.
    cfg = dur.config("por_lote", lote_ms=3_600_000, lote_op=1_000_000)
    cfg["max_linhas"] = 100_000
    return cfg


def subir(limpar):
    return dur.subir(config(), limpar=limpar)


def esperar_porta(prazo=20):
    fim = time.time() + prazo
    while time.time() < fim:
        try:
            socket.create_connection(("127.0.0.1", PORTA), 0.3).close()
            return True
        except OSError:
            time.sleep(0.05)
    return False


# ------------------------------------------------------------------ pedidos

def tabela(c, nome="t"):
    c.ok({"op": "criar_tabela", "database": DB, "tabela": nome,
          "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                      {"nome": "v", "tipo": "Str(20)"}],
          "indices": [{"nome": "pk", "colunas": ["id"], "unico": True,
                       "primario": True}]})


def inserir(c, i, v=None):
    return c.ok({"op": "inserir", "database": DB, "tabela": "t",
                 "linha": {"id": i, "v": v or f"v{i}"}})


def inserir_lote(c, de, ate):
    c.ok({"op": "inserir_lote", "database": DB, "tabela": "t",
          "linhas": [{"id": i, "v": f"v{i}"} for i in range(de, ate + 1)]})


def esquema(c):
    r = c.fala({"op": "esquema", "database": DB, "tabela": "t"})
    if not r.get("ok"):
        return {"registros": -1, "slots": -1, "erro": r.get("erro", "")[:120]}
    return {"registros": r["resultado"]["registros"], "slots": r["resultado"]["slots"]}


def buscar(c, i):
    """('ok', quantas) ou ('erro', texto INTEIRO).

    Inteiro, e nao recortado: a primeira versao devolvia 160 caracteres e o
    `indice_sujo()` procurava «reparar indice» no que sobrava -- o texto
    cortava em «nao e co», a marca nao era vista, e a passada rapida
    classificou como «indice limpo e vazio EM SILENCIO» um indice que estava
    recusando corretamente. O achado era do instrumento. Recorta-se so ao
    guardar."""
    r = c.fala({"op": "buscar", "database": DB, "tabela": "t", "indice": "pk",
                "chave": [i], "max": 5})
    if r.get("ok"):
        return "ok", len(r["resultado"]["linhas"])
    return "erro", r.get("erro", "")


def ler(c, rowid):
    r = c.fala({"op": "ler", "database": DB, "tabela": "t", "rowid": rowid})
    return r.get("resultado") if r.get("ok") else None


def verificar(c):
    r = c.fala({"op": "verificar", "database": DB, "tabela": "t"})
    if r.get("ok"):
        return "ok", {"registros": r["resultado"]["registros"],
                      "indices": r["resultado"]["indices"]}
    return "erro", r.get("erro", "")[:200]


def reindexar(c):
    r = c.fala({"op": "reindexar", "database": DB, "tabela": "t"})
    return r.get("ok"), (r.get("erro", "")[:160] if not r.get("ok") else "")


def cargas(c):
    r = c.fala({"op": "cargas"})
    return r["resultado"]["total"] if r.get("ok") else -1


def indice_sujo(texto):
    """A recusa do `.ndx` que ficou para tras: `NdxFile::conferir_confiavel`."""
    return ("ficou para tras numa queda" in texto or "reparar indice" in texto
            or "reconstru" in texto)


def byte52():
    """A marca de sujo do `.ndx` (`ndx.rs`, byte 52 do cabecalho), lida direto
    do arquivo -- ANTES de reabrir, para que nenhum `fechar` a mexa."""
    caminho = os.path.join(dur.caminho_db(), "t.ndx")
    try:
        with open(caminho, "rb") as f:
            f.seek(52)
            b = f.read(1)
        return b[0] if b else None
    except OSError:
        return None


def chaves_achadas(c, ids):
    """Quantas das chaves pedidas o `buscar` acha EXATAMENTE uma vez, quantas
    acha em dobro, e o primeiro erro (se o indice recusar)."""
    achadas, dobradas, erro = 0, 0, ""
    for i in ids:
        estado, q = buscar(c, i)
        if estado == "erro":
            return achadas, dobradas, q
        if q == 1:
            achadas += 1
        elif q > 1:
            dobradas += 1
    return achadas, dobradas, erro


def amostra(n, teto=2000):
    """Todas as chaves ate `teto`; acima, uma amostra espalhada de `teto`."""
    if n <= teto:
        return list(range(1, n + 1))
    passo = n / teto
    return sorted({int(1 + k * passo) for k in range(teto)} | {n})


# ------------------------------------------------------------ uma corrida

def corrida(rotulo, montar, roteiro, atraso_s, julgar):
    """Sobe, monta, dispara o `roteiro` numa thread, mata `atraso_s` depois do
    instante de referencia que o roteiro sinaliza, reabre e chama `julgar`.

    O roteiro recebe `(c, comecou, prog)`: sinaliza `comecou` no instante a
    partir do qual o atraso conta, e vai anotando em `prog` o que ja recebeu
    «ok» -- e essa anotacao que diz ONDE a tomada caiu."""
    p, _off = subir(limpar=True)
    resultado = {"rotulo": rotulo, "atraso_ms": round(atraso_s * 1000, 3)}
    c = Ligacao()
    estado = montar(c)
    comecou = threading.Event()
    prog = {"confirmadas": 0}
    fio = threading.Thread(target=roteiro, args=(c, comecou, prog), daemon=True)
    t0 = None
    fio.start()
    if comecou.wait(30):
        t0 = time.time()
        if atraso_s > 0:
            time.sleep(atraso_s)
    dur.matar_de_verdade(p)
    t_kill = time.time()
    fio.join(30)
    try:
        c.fechar()
    except OSError:
        pass
    resultado["atraso_medido_ms"] = None if t0 is None else round((t_kill - t0) * 1000, 3)
    resultado["progresso"] = dict(prog)
    dur.esperar_porta_fechar()
    resultado["marcas_antes_de_reabrir"] = dur.marcas()
    resultado["ndx_byte52_apos_queda"] = byte52()

    p2, off2 = subir(limpar=False)
    try:
        c2 = Ligacao()
        rel = dur.ler_relatorio(off2)
        resultado["relatorio"] = rel
        resultado.update(julgar(c2, estado, prog, rel))
        c2.fechar()
    finally:
        dur.derrubar_limpo(p2)
    resultado["marcas_depois"] = dur.marcas()
    return resultado


def calibrar(montar, roteiro):
    """O roteiro inteiro SEM matar ninguem: quanto ele leva nesta maquina,
    agora, com este binario. Decide a faixa de atrasos -- nao se chuta."""
    p, _ = subir(limpar=True)
    try:
        c = Ligacao()
        montar(c)
        comecou = threading.Event()
        prog = {"confirmadas": 0}
        t0 = [None]

        def marcar_e_rodar():
            roteiro(c, comecou, prog)

        fio = threading.Thread(target=marcar_e_rodar, daemon=True)
        fio.start()
        comecou.wait(30)
        t0[0] = time.time()
        fio.join(120)
        dt = time.time() - t0[0]
        c.fechar()
        return dt, dict(prog)
    finally:
        dur.derrubar_limpo(p)


def atrasos_de(t_limpo, n, folga=1.05):
    return [t_limpo * folga * i / (n - 1) for i in range(n)]


def varredura(nome, montar, roteiro, julgar, n_atrasos):
    t_limpo, prog_limpo = calibrar(montar, roteiro)
    print(f"    calibracao ({nome}, sem matar): {t_limpo * 1000:.1f} ms  progresso={prog_limpo}")
    corridas = []
    for rodada in range(1, RODADAS + 1):
        for i, atraso in enumerate(atrasos_de(t_limpo, n_atrasos)):
            r = corrida(f"{nome}#{rodada}.{i}", montar, roteiro, atraso, julgar)
            r["rodada"] = rodada
            corridas.append(r)
            print("      r%d atraso %7.2f ms -> %-22s %s%s"
                  % (rodada, r["atraso_ms"], r["classe"],
                     "" if r["valido"] else "*** INVALIDO *** ",
                     r.get("detalhe", "")))
    classes = {}
    for r in corridas:
        classes[r["classe"]] = classes.get(r["classe"], 0) + 1
    invalidas = [r for r in corridas if not r["valido"]]
    return {"esperado": ESPERADO[nome], "t_calibracao_ms": round(t_limpo * 1000, 2),
            "progresso_sem_queda": prog_limpo, "atrasos": n_atrasos, "rodadas": RODADAS,
            "corridas": corridas, "classes": classes,
            "invalidas": [{"rotulo": r["rotulo"], "atraso_ms": r["atraso_ms"],
                           "classe": r["classe"], "detalhe": r.get("detalhe", ""),
                           "relatorio": r.get("relatorio"), "progresso": r["progresso"]}
                          for r in invalidas]}


# ============================================ 1. transacao ABERTA, sem COMMIT

def montar_tx(c):
    c.ok({"op": "criar_database", "database": DB})
    tabela(c)
    for i in range(1, N_TX_BASE + 1):
        inserir(c, i)
    return esquema(c)


def roteiro_tx(c, comecou, prog):
    """Inserir, atualizar, excluir, SAVEPOINT, inserir, ROLLBACK TO, inserir --
    e nunca COMMIT. `confirmadas` conta instrucoes que receberam «ok»."""
    try:
        c.ok({"op": "begin", "database": DB})
        comecou.set()
        for i in range(1001, 1151):
            inserir(c, i)
            prog["confirmadas"] += 1
        for rowid in range(1, 21):
            c.ok({"op": "atualizar", "database": DB, "tabela": "t", "rowid": rowid,
                  "linha": {"id": rowid, "v": "alterado"}})
            prog["confirmadas"] += 1
        for rowid in range(21, 31):
            c.ok({"op": "excluir", "database": DB, "tabela": "t", "rowid": rowid})
            prog["confirmadas"] += 1
        c.ok({"op": "savepoint", "database": DB, "nome": "s1"})
        prog["confirmadas"] += 1
        for i in range(1151, 1251):
            inserir(c, i)
            prog["confirmadas"] += 1
        c.ok({"op": "rollback_to_savepoint", "database": DB, "nome": "s1"})
        prog["confirmadas"] += 1
        for i in range(1251, 1301):
            inserir(c, i)
            prog["confirmadas"] += 1
        prog["acabou"] = True
    except (OSError, ValueError, SystemExit) as e:
        prog["parou"] = str(e)[:100]
        comecou.set()


def julgar_tx(c, antes, prog, rel):
    e = esquema(c)
    problemas = []
    if rel is not None:
        problemas.append(f"o arranque imprimiu Recovery: {rel}")
    if dur.marcas():
        problemas.append(f"marca .tx viva: {dur.marcas()}")
    if e != antes:
        problemas.append(f"esquema {e} != antes {antes}")
    for rowid in range(1, 21):
        linha = ler(c, rowid)
        if not linha or linha.get("v") != f"v{rowid}":
            problemas.append(f"rowid {rowid} = {linha}")
            break
    for rowid in range(21, 31):
        linha = ler(c, rowid)
        if not linha or linha.get("id") != rowid:
            problemas.append(f"rowid {rowid} (excluido na tx) = {linha}")
            break
    for i in (1001, 1151, 1251):
        estado, q = buscar(c, i)
        if estado == "erro" or q != 0:
            problemas.append(f"chave {i} da transacao: {estado} {q}")
    v = verificar(c)
    if v[0] != "ok" or v[1]["registros"] != antes["registros"]:
        problemas.append(f"verificar: {v}")
    n = prog.get("confirmadas", 0)
    if prog.get("acabou"):
        classe = "TODAS_AS_OPS_E_SEM_COMMIT"
    elif n == 0:
        classe = "LOGO_APOS_O_BEGIN"
    elif n <= 150:
        classe = "NO_MEIO_DOS_INSERTS"
    elif n <= 180:
        classe = "NO_MEIO_DE_ATUALIZAR_EXCLUIR"
    elif n <= 281:
        classe = "ENTRE_SAVEPOINT_E_ROLLBACK_TO"
    else:
        classe = "DEPOIS_DO_ROLLBACK_TO"
    return {"classe": classe, "valido": not problemas,
            "detalhe": "; ".join(problemas) if problemas else f"ops={n}"}


# =============================================== 2. BULKINSERT sem transacao

def montar_bulk(c):
    c.ok({"op": "criar_database", "database": DB})
    tabela(c)
    return {}


def roteiro_bulk(c, comecou, prog):
    try:
        comecou.set()
        r = c.fala({"op": "bulkinsert", "database": DB, "tabela": "t", "ligado": True})
        prog["reservou"] = bool(r.get("ok"))
        for i in range(1, N_BULK + 1):
            inserir(c, i)
            prog["confirmadas"] += 1
        r = c.fala({"op": "bulkinsert", "database": DB, "tabela": "t", "ligado": False})
        prog["soltou"] = bool(r.get("ok"))
        prog["acabou"] = True
    except (OSError, ValueError, SystemExit) as e:
        prog["parou"] = str(e)[:100]
        comecou.set()


def roteiro_bulk_lote(c, comecou, prog):
    """O mesmo BULKINSERT, em `inserir_lote`: e o que a carga de verdade manda
    (`bancada/carga/bulkinsert.py` usa lotes de 5.000), e e o pedido que
    segura a tabela aberta tempo bastante para o write-back do `.ndx` ficar
    exposto a tomada. `confirmadas` conta LINHAS de lotes que responderam."""
    lote = N_BULK // 4
    try:
        comecou.set()
        r = c.fala({"op": "bulkinsert", "database": DB, "tabela": "t", "ligado": True})
        prog["reservou"] = bool(r.get("ok"))
        for de in range(1, N_BULK + 1, lote):
            inserir_lote(c, de, de + lote - 1)
            prog["confirmadas"] += lote
        r = c.fala({"op": "bulkinsert", "database": DB, "tabela": "t", "ligado": False})
        prog["soltou"] = bool(r.get("ok"))
        prog["acabou"] = True
    except (OSError, ValueError, SystemExit) as e:
        prog["parou"] = str(e)[:100]
        comecou.set()


def fazer_julgar_bulk(lote):
    """`lote` = quantas linhas um pedido confirma de uma vez: 1 no `inserir`,
    N_BULK/4 no `inserir_lote`. Fora de transacao o lote NAO e atomico, entao
    a tomada no meio dele deixa de 0 a `lote` linhas alem das confirmadas."""

    def julgar_bulk(c, _estado, prog, rel):
        problemas = []
        if rel is not None:
            problemas.append(f"o arranque imprimiu Recovery sem marca: {rel}")
        if dur.marcas():
            problemas.append(f"marca .tx num BULKINSERT sem transacao: {dur.marcas()}")
        reserva = cargas(c)
        if reserva != 0:
            problemas.append(f"reserva sobrou depois da queda: {reserva}")
        e = esquema(c)
        conf = prog.get("confirmadas", 0)
        r_ = e["registros"]
        sujo_antes = buscar(c, 1)
        indice_sujo_ = sujo_antes[0] == "erro" and indice_sujo(sujo_antes[1])
        if sujo_antes[0] == "erro" and not indice_sujo_:
            problemas.append(f"buscar recusou por outro motivo: {sujo_antes[1][:160]}")
        v_antes = verificar(c)
        reindexou = False
        if indice_sujo_:
            reindexou, erro = reindexar(c)
            if not reindexou:
                problemas.append(f"reindexar falhou: {erro}")
        achadas, dobradas, erro = chaves_achadas(c, amostra(r_))
        v = verificar(c)
        if prog.get("soltou"):
            classe = "APOS_O_OK_FINAL"
            if r_ != N_BULK:
                problemas.append(f"registros {r_} != {N_BULK} depois do ok final")
            if indice_sujo_:
                problemas.append("indice sujo depois de bulkinsert(false) ter respondido ok")
        elif prog.get("reservou"):
            classe = "NO_MEIO_DAS_LINHAS" + ("_INDICE_SUJO" if indice_sujo_ else "_INDICE_LIMPO")
            if not (conf <= r_ <= conf + lote):
                problemas.append(f"registros {r_} fora de [{conf}, {conf + lote}]")
        else:
            classe = "ANTES_DA_RESERVA"
            if r_ != 0:
                problemas.append(f"registros {r_} sem reserva nenhuma")
        if r_ >= 0 and achadas != len(amostra(r_)):
            problemas.append(f"chaves achadas {achadas} de {len(amostra(r_))} (erro: {erro[:160]})")
        if dobradas:
            problemas.append(f"{dobradas} chave(s) em dobro")
        if v[0] != "ok" or v[1]["registros"] != r_ or v[1]["indices"].get("pk") != r_:
            problemas.append(f"verificar depois: {v}")
        if not indice_sujo_ and v_antes[0] == "erro":
            # Indice que se diz limpo e nao confere e o pior dos dois mundos.
            problemas.append(f"indice LIMPO que nao confere: {v_antes[1][:160]}")
        return {"classe": classe, "valido": not problemas,
                "indice_sujo": indice_sujo_, "reindexou": reindexou,
                "registros": r_, "confirmadas": conf,
                "verificar_antes": v_antes if v_antes[0] == "ok" else ("erro", v_antes[1][:160]),
                "verificar_depois": v if v[0] == "ok" else ("erro", v[1][:160]),
                "detalhe": "; ".join(problemas) if problemas else
                f"conf={conf} reg={r_} sujo={indice_sujo_} achadas={achadas}"}

    return julgar_bulk


julgar_bulk = fazer_julgar_bulk(1)
julgar_bulk_lote = fazer_julgar_bulk(N_BULK // 4)


# ====================================================== 2c. no meio do reindexar

def montar_reindex(c):
    c.ok({"op": "criar_database", "database": DB})
    tabela(c)
    for de in range(1, N_REINDEX + 1, 1000):
        inserir_lote(c, de, min(de + 999, N_REINDEX))
    return {}


def roteiro_reindex(c, comecou, prog):
    try:
        comecou.set()
        r = c.fala({"op": "reindexar", "database": DB, "tabela": "t"})
        prog["reindexou"] = bool(r.get("ok"))
        prog["acabou"] = True
    except (OSError, ValueError, SystemExit) as e:
        prog["parou"] = str(e)[:100]
        comecou.set()


def julgar_reindex(c, _estado, prog, rel):
    problemas = []
    if rel is not None:
        problemas.append(f"Recovery sem marca: {rel}")
    e = esquema(c)
    r_ = e["registros"]
    primeira = buscar(c, 1)
    sujo = primeira[0] == "erro" and indice_sujo(primeira[1])
    v_antes = verificar(c)
    achadas = dobradas = 0
    erro = ""
    reindexou = False
    if sujo:
        classe = "SUJO_DETECTADO"
        reindexou, erro_r = reindexar(c)
        if not reindexou:
            problemas.append(f"reindexar depois da queda falhou: {erro_r}")
        achadas, dobradas, erro = chaves_achadas(c, amostra(r_))
    else:
        achadas, dobradas, erro = chaves_achadas(c, amostra(r_))
        if achadas == len(amostra(r_)) and v_antes[0] == "ok":
            classe = "INTEIRO_" + ("DEPOIS_DE_TERMINAR" if prog.get("reindexou") else "ANTES_DE_COMECAR")
        else:
            # O caso da hipotese: indice que se diz limpo e nao tem as chaves.
            classe = "*** VAZIO_OU_PARCIAL_EM_SILENCIO ***"
            problemas.append(f"indice limpo com {achadas}/{len(amostra(r_))} chaves; "
                             f"verificar: {v_antes[0]} {str(v_antes[1])[:160]}")
    v = verificar(c)
    if r_ != N_REINDEX:
        problemas.append(f"registros {r_} != {N_REINDEX}")
    if achadas != len(amostra(r_)) or dobradas:
        problemas.append(f"achadas {achadas}/{len(amostra(r_))} dobradas {dobradas} {erro[:160]}")
    if v[0] != "ok":
        problemas.append(f"verificar no fim: {v[0]} {str(v[1])[:160]}")
    return {"classe": classe, "valido": not problemas, "indice_sujo": sujo,
            "reindexou_depois": reindexou,
            "verificar_antes": v_antes if v_antes[0] == "ok" else ("erro", v_antes[1][:160]),
            "detalhe": "; ".join(problemas) if problemas else
            f"reindexar_ok={prog.get('reindexou')} sujo={sujo} achadas={achadas}"}


# ============================================ 3. BULKINSERT dentro da transacao

def montar_bulk_tx(c):
    c.ok({"op": "criar_database", "database": DB})
    tabela(c)
    return {}


def roteiro_bulk_tx(c, comecou, prog):
    try:
        c.ok({"op": "begin", "database": DB})
        comecou.set()
        r = c.fala({"op": "bulkinsert", "database": DB, "tabela": "t", "ligado": True})
        prog["reservou"] = bool(r.get("ok"))
        if not r.get("ok"):
            prog["recusa"] = r.get("erro", "")[:200]
            prog["acabou"] = True
            return
        for i in range(1, N_BULK_TX + 1):
            inserir(c, i)
            prog["confirmadas"] += 1
        r = c.fala({"op": "commit"})
        prog["commit"] = bool(r.get("ok"))
        r = c.fala({"op": "bulkinsert", "database": DB, "tabela": "t", "ligado": False})
        prog["soltou"] = bool(r.get("ok"))
        prog["marcas_apos_bulkinsert_false"] = dur.marcas()
        prog["acabou"] = True
    except (OSError, ValueError, SystemExit) as e:
        prog["parou"] = str(e)[:100]
        comecou.set()


def julgar_bulk_tx(c, _estado, prog, rel):
    problemas = []
    e = esquema(c)
    r_ = e["registros"]
    if r_ not in (0, N_BULK_TX):
        problemas.append(f"registros {r_}: nem 0 nem {N_BULK_TX} -- METADE")
    if dur.marcas():
        problemas.append(f"marca sobrou depois da recuperacao: {dur.marcas()}")
    if cargas(c) != 0:
        problemas.append("reserva sobrou")
    primeira = buscar(c, 1)
    if primeira[0] == "erro":
        problemas.append(f"buscar recusou depois da recuperacao: {primeira[1]}")
    achadas, dobradas, erro = chaves_achadas(c, amostra(r_))
    if achadas != len(amostra(r_)) or dobradas:
        problemas.append(f"achadas {achadas}/{len(amostra(r_))} dobradas {dobradas} {erro}")
    v = verificar(c)
    if v[0] != "ok" or v[1]["registros"] != r_:
        problemas.append(f"verificar: {v}")
    falso = {"relatorio": rel, "marca_apareceu": True, "atraso_ms": 1}
    if prog.get("soltou"):
        classe = "APOS_BULKINSERT_FALSE"
        if r_ != N_BULK_TX:
            problemas.append(f"depois do ok final, registros {r_}")
    elif prog.get("commit"):
        classe = "APOS_COMMIT_OK_ANTES_DE_SOLTAR"
        if r_ != N_BULK_TX:
            problemas.append(f"COMMIT respondeu ok e registros {r_}")
    elif prog.get("reservou"):
        classe = "NO_MEIO: " + (dur.classificar(falso, N_BULK_TX) if rel is not None
                                else "sem marca (antes do commit)")
        if rel is None and r_ != 0:
            problemas.append(f"sem marca e registros {r_}")
    else:
        classe = "ANTES_DA_RESERVA"
    return {"classe": classe, "valido": not problemas, "registros": r_,
            "detalhe": "; ".join(problemas) if problemas else
            f"conf={prog.get('confirmadas')} reg={r_} rel={None if rel is None else {k: v for k, v in rel.items() if k != 'impossiveis_linhas'}}"}


def bulk_em_tx_e_aceito():
    """Sem matar ninguem: o motor aceita BULKINSERT dentro da transacao? E a
    marca do commit fica no disco depois do bulkinsert(false)?"""
    p, _ = subir(limpar=True)
    try:
        c = Ligacao()
        montar_bulk_tx(c)
        comecou = threading.Event()
        prog = {"confirmadas": 0}
        roteiro_bulk_tx(c, comecou, prog)
        marcas_depois = dur.marcas()
        time.sleep(0.3)
        marcas_tarde = dur.marcas()
        c.fechar()
        return {"aceito": bool(prog.get("reservou")), "recusa": prog.get("recusa"),
                "commit": prog.get("commit"), "soltou": prog.get("soltou"),
                "marcas_logo_apos_bulkinsert_false": marcas_depois,
                "marcas_300ms_depois": marcas_tarde}
    finally:
        dur.derrubar_limpo(p)


# ======================================= 3b. transacao DENTRO de um BULKINSERT

def roteiro_tx_em_bulk(c, comecou, prog):
    """A ordem inversa: reserva primeiro, transacao dentro. `op_bulkinsert`
    recusa quando ja ha transacao aberta; `begin` com a tabela reservada por
    esta mesma ligacao nao passa por portao nenhum que o recuse."""
    try:
        r = c.fala({"op": "bulkinsert", "database": DB, "tabela": "t", "ligado": True})
        prog["reservou"] = bool(r.get("ok"))
        if not r.get("ok"):
            prog["recusa"] = r.get("erro", "")[:300]
            prog["acabou"] = True
            comecou.set()
            return
        r = c.fala({"op": "begin", "database": DB})
        prog["begin"] = bool(r.get("ok"))
        if not r.get("ok"):
            prog["recusa"] = r.get("erro", "")[:300]
            prog["acabou"] = True
            comecou.set()
            return
        comecou.set()
        for i in range(1, N_BULK_TX + 1):
            inserir(c, i)
            prog["confirmadas"] += 1
        r = c.fala({"op": "commit"})
        prog["commit"] = bool(r.get("ok"))
        if not r.get("ok"):
            prog["recusa"] = r.get("erro", "")[:300]
        prog["marcas_apos_commit"] = dur.marcas()
        r = c.fala({"op": "bulkinsert", "database": DB, "tabela": "t", "ligado": False})
        prog["soltou"] = bool(r.get("ok"))
        prog["marcas_apos_bulkinsert_false"] = dur.marcas()
        prog["acabou"] = True
    except (OSError, ValueError, SystemExit) as e:
        prog["parou"] = str(e)[:100]
        comecou.set()


def julgar_tx_em_bulk(c, _estado, prog, rel):
    problemas = []
    e = esquema(c)
    r_ = e["registros"]
    if r_ not in (0, N_BULK_TX):
        problemas.append(f"registros {r_}: nem 0 nem {N_BULK_TX} -- METADE")
    if dur.marcas():
        problemas.append(f"marca sobrou depois da recuperacao: {dur.marcas()}")
    if cargas(c) != 0:
        problemas.append("reserva sobrou")
    primeira = buscar(c, 1)
    if primeira[0] == "erro":
        problemas.append(f"buscar recusou depois da recuperacao: {primeira[1][:160]}")
    achadas, dobradas, erro = chaves_achadas(c, amostra(r_))
    if achadas != len(amostra(r_)) or dobradas:
        problemas.append(f"achadas {achadas}/{len(amostra(r_))} dobradas {dobradas} {erro[:160]}")
    v = verificar(c)
    if v[0] != "ok" or v[1]["registros"] != r_:
        problemas.append(f"verificar: {v}")
    falso = {"relatorio": rel, "marca_apareceu": True, "atraso_ms": 1}
    if prog.get("soltou"):
        classe = "APOS_BULKINSERT_FALSE" + ("_MARCA_REPORTADA" if rel is not None else "_SEM_MARCA")
        if r_ != N_BULK_TX:
            problemas.append(f"depois do ok final, registros {r_}")
        # Pedido 254: o «ok» do bulkinsert(false) so sai depois de a marca do
        # COMMIT ter sido drenada. Marca reportada aqui e o defeito de volta.
        if rel is not None:
            problemas.append("marca .tx do COMMIT sobreviveu ao ok do bulkinsert(false) (pedido 254)")
    elif prog.get("commit"):
        classe = "APOS_COMMIT_OK_ANTES_DE_SOLTAR"
        if r_ != N_BULK_TX:
            problemas.append(f"COMMIT respondeu ok e registros {r_}")
    elif prog.get("begin"):
        classe = "NO_MEIO: " + (dur.classificar(falso, N_BULK_TX) if rel is not None
                                else "sem marca (antes do commit)")
        if rel is None and r_ != 0:
            problemas.append(f"sem marca e registros {r_}")
    else:
        classe = "ANTES_DO_BEGIN"
    rel_curto = None if rel is None else {k: v for k, v in rel.items() if k != "impossiveis_linhas"}
    return {"classe": classe, "valido": not problemas, "registros": r_,
            "detalhe": "; ".join(problemas) if problemas else
            f"conf={prog.get('confirmadas')} reg={r_} rel={rel_curto}"}


def tx_em_bulk_sem_queda():
    """Sem matar ninguem: a transacao entra na tabela reservada? E a marca do
    COMMIT fica no disco depois do bulkinsert(false)? (a hipotese)"""
    p, _ = subir(limpar=True)
    try:
        c = Ligacao()
        montar_bulk_tx(c)
        comecou = threading.Event()
        prog = {"confirmadas": 0}
        roteiro_tx_em_bulk(c, comecou, prog)
        time.sleep(0.3)
        marcas_tarde = dur.marcas()
        c.fechar()
        return {"aceito": bool(prog.get("begin")) and bool(prog.get("commit")),
                "recusa": prog.get("recusa"), "reservou": prog.get("reservou"),
                "commit": prog.get("commit"), "soltou": prog.get("soltou"),
                "marcas_apos_commit": prog.get("marcas_apos_commit"),
                "marcas_apos_bulkinsert_false": prog.get("marcas_apos_bulkinsert_false"),
                "marcas_300ms_depois": marcas_tarde}
    finally:
        dur.derrubar_limpo(p)


# ============================================ 4. logo depois do «ok» + fsync

def queda_apos(acao, julgar):
    p, _ = subir(limpar=True)
    try:
        c = Ligacao()
        c.ok({"op": "criar_database", "database": DB})
        tabela(c)
        acao(c)
        dur.matar_de_verdade(p)
        c.fechar()
    except OSError:
        pass
    dur.esperar_porta_fechar()
    marcas_antes = dur.marcas()
    p2, off = subir(limpar=False)
    try:
        c2 = Ligacao()
        rel = dur.ler_relatorio(off)
        r = julgar(c2, rel)
        c2.fechar()
    finally:
        dur.derrubar_limpo(p2)
    r["marcas_antes_de_reabrir"] = marcas_antes
    r["marcas_depois"] = dur.marcas()
    r["relatorio"] = rel
    return r


def commit_n(c):
    c.ok({"op": "begin", "database": DB})
    for i in range(1, N_APOS + 1):
        inserir(c, i)
    c.ok({"op": "commit"})


def bulk_n(c):
    c.ok({"op": "bulkinsert", "database": DB, "tabela": "t", "ligado": True})
    for i in range(1, N_APOS + 1):
        inserir(c, i)
    c.ok({"op": "bulkinsert", "database": DB, "tabela": "t", "ligado": False})


def julgar_apos(c, rel):
    e = esquema(c)
    primeira = buscar(c, 1)
    sujo = primeira[0] == "erro" and indice_sujo(primeira[1])
    achadas, dobradas, erro = chaves_achadas(c, amostra(N_APOS))
    v = verificar(c)
    return {"registros": e["registros"], "indice_sujo": sujo, "achadas": achadas,
            "dobradas": dobradas, "erro_buscar": erro, "verificar": v}


def fsync_por_extensao(acao):
    """Conta os `fsync` que a acao provoca, por extensao, com o `strace`
    ANEXADO ao servidor de pe (nunca substituindo o processo) e so depois do
    esquema criado. So `fsync`: nenhum lugar do motor chama `sync_data`
    (`grep -rn sync_data crates/*/src` vazio), entao `fdatasync` nao
    aconteceria e rastrea-lo so acrescentaria uma linha zerada."""
    p, _ = subir(limpar=True)
    saida = os.path.join(BASE, "strace.txt")
    st = None
    try:
        c = Ligacao()
        c.ok({"op": "criar_database", "database": DB})
        tabela(c)
        st = fecho.anexar(p.pid, saida)
        acao(c)
        time.sleep(0.4)
        fecho.soltar(st)
        st = None
        c.fechar()
    finally:
        if st is not None:
            fecho.soltar(st)
        dur.derrubar_limpo(p)
    por_ext = {}
    for _ts, caminho, ok_ in fecho.eventos_fsync(saida):
        if not ok_:
            continue
        base = os.path.basename(caminho)
        ext = base.rsplit(".", 1)[-1] if "." in base else "(sem)"
        por_ext[ext] = por_ext.get(ext, 0) + 1
    return por_ext


# ----------------------------------------------------------------- o portao

def portao(esperar=True):
    """Consulta `esta-medindo.sh`; ESPERA bancada de tempo, REGISTRA compilacao."""
    gate = os.path.join(RAIZ, "bancada", "esta-medindo.sh")
    inicio = time.time()
    esperou = 0
    while True:
        r = subprocess.run([gate], capture_output=True, text=True)
        linhas = [l for l in r.stdout.splitlines() if l.strip()]
        bancadas = [l for l in linhas if "bancada em python" in l or "exemplo de medicao" in l]
        compilacoes = [l for l in linhas if "compilacao" in l]
        if not bancadas or not esperar or time.time() - inicio > 1200:
            return {"havia_bancada_em_curso": bool(bancadas), "esperou_s": round(esperou),
                    "esperar_estava_ligado": esperar,
                    "desistiu_de_esperar": bool(bancadas) and esperar,
                    "compilacao_em_curso": [l[:100] for l in compilacoes][:4],
                    "quem": [l[:100] for l in bancadas][:6]}
        print("  portao: ha bancada de tempo em curso -- esperando 30 s  (%s)"
              % bancadas[0][:80])
        time.sleep(30)
        esperou = time.time() - inicio


def binario():
    """Congela UMA copia do `phxsqld` para a corrida inteira.

    Numa maquina com outras frentes compilando, `target/release/phxsqld` pode
    ser trocado por baixo no meio das centenas de corridas -- e ai metade da
    varredura mede um binario e metade outro, com um so rotulo de versao. A
    copia fica ao lado da base de prova e e apagada no fim; o SHA-256 e o
    mtime do original vao para o resultado, que e o que diz O QUE foi medido."""
    import hashlib
    original = dur.PHXSQLD
    os.makedirs(BASE, exist_ok=True)
    copia = os.path.join(AQUI, ".phxsqld-desta-corrida")
    shutil.copy2(original, copia)
    os.chmod(copia, 0o755)
    with open(copia, "rb") as f:
        sha = hashlib.sha256(f.read()).hexdigest()
    dur.PHXSQLD = copia
    versao = subprocess.run([copia, "--version"], capture_output=True,
                            text=True).stdout.strip()
    mtime = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(os.path.getmtime(original)))
    head = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=RAIZ,
                          capture_output=True, text=True).stdout.strip()
    sujos = subprocess.run(["git", "status", "--short", "crates"], cwd=RAIZ,
                           capture_output=True, text=True).stdout.split("\n")
    mais_novos = subprocess.run(
        ["find", "crates", "-name", "*.rs", "-newer", original],
        cwd=RAIZ, capture_output=True, text=True).stdout.split()
    return {"versao": versao, "compilado_em_utc": mtime, "sha256": sha,
            "original": original, "copia_usada": copia, "head_no_inicio": head,
            "crates_sujos_no_inicio": [s for s in sujos if s.strip()],
            "fontes_mais_novos_que_o_binario": sorted(mais_novos)}


def apagar_copia_do_binario():
    try:
        os.remove(os.path.join(AQUI, ".phxsqld-desta-corrida"))
    except OSError:
        pass


# --------------------------------------------------------------------- main

def main():
    if not os.path.exists(dur.PHXSQLD):
        print("falta %s -- rode `cargo build --release`" % dur.PHXSQLD)
        return 2
    bruto = {"medido_em_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
             "binario": binario(), "rapido": RAPIDO,
             "parametros": {"atrasos": N_ATRASOS, "atrasos_reindex": N_ATRASOS_REINDEX,
                            "rodadas": RODADAS, "n_tx_base": N_TX_BASE, "n_bulk": N_BULK,
                            "n_bulk_tx": N_BULK_TX, "n_reindex": N_REINDEX, "n_apos": N_APOS,
                            "porta": PORTA}}
    print("== chutar a tomada, contra %s ==" % bruto["binario"]["versao"])
    if bruto["binario"]["fontes_mais_novos_que_o_binario"]:
        print("  aviso: fontes mais novos que o binario: %s"
              % ", ".join(bruto["binario"]["fontes_mais_novos_que_o_binario"]))
    if RAPIDO:
        print("!! PHX_TOMADA_RAPIDO=1: poucos atrasos, uma rodada. NAO se publica.")
    bruto["maquina"] = portao(esperar=os.environ.get("PHX_TOMADA_SEM_ESPERA") != "1")
    print("  portao: %s" % bruto["maquina"])
    inicio = time.time()

    print("\n== 1. SIGKILL dentro de uma transacao ABERTA, antes do COMMIT ==")
    print("   esperado: " + ESPERADO["tx_aberta"])
    p1 = varredura("tx_aberta", montar_tx, roteiro_tx, julgar_tx, N_ATRASOS)
    ok("tx aberta: zero rastro em todas as corridas", not p1["invalidas"],
       f"{len(p1['invalidas'])} invalida(s) de {len(p1['corridas'])}; classes {p1['classes']}")
    ok("tx aberta: a varredura acertou pontos DIFERENTES da sequencia",
       len(p1["classes"]) >= 3, f"{p1['classes']}")
    ok("tx aberta: o arranque ficou calado em todas",
       all(r["relatorio"] is None for r in p1["corridas"]))

    print("\n== 2. SIGKILL no meio de um BULKINSERT ==")
    print("   esperado: " + ESPERADO["bulk"])
    p2 = varredura("bulk", montar_bulk, roteiro_bulk, julgar_bulk, N_ATRASOS)
    ok("bulk: nenhuma corrida perdeu linha confirmada, duplicou ou atrasou o indice em silencio",
       not p2["invalidas"],
       f"{len(p2['invalidas'])} invalida(s) de {len(p2['corridas'])}; classes {p2['classes']}")
    sujas = [r for r in p2["corridas"] if r.get("indice_sujo")]
    # Informativo, nao veredito: com `inserir` linha a linha a janela em que a
    # marca fica levantada e o proprio pedido (microssegundos). O controle de
    # que o instrumento VE a marca esta no `bulk_lote`, logo abaixo.
    print("   informativo: %d corrida(s) de %d acharam o indice sujo (byte 52 = 1 em %d)"
          % (len(sujas), len(p2["corridas"]),
             sum(1 for r in p2["corridas"] if r.get("ndx_byte52_apos_queda") == 1)))
    ok("bulk: a reserva voltou solta em todas",
       all("reserva sobrou" not in r.get("detalhe", "") for r in p2["corridas"]))

    print("\n== 2b. SIGKILL no meio de um BULKINSERT em LOTES (inserir_lote) ==")
    print("   esperado: " + ESPERADO["bulk_lote"])
    p2b = varredura("bulk_lote", montar_bulk, roteiro_bulk_lote, julgar_bulk_lote, N_ATRASOS)
    ok("bulk_lote: nenhuma corrida perdeu linha confirmada, duplicou ou atrasou o indice em silencio",
       not p2b["invalidas"],
       f"{len(p2b['invalidas'])} invalida(s) de {len(p2b['corridas'])}; classes {p2b['classes']}")
    sujas_lote = [r for r in p2b["corridas"] if r.get("indice_sujo")]
    ok("bulk_lote: a varredura pegou o indice SUJO (byte 52 = 1) pelo menos uma vez -- o instrumento ve a marca",
       bool(sujas_lote) and all(r.get("ndx_byte52_apos_queda") == 1 for r in sujas_lote),
       "%d corrida(s) com indice sujo, byte52=%s" % (
           len(sujas_lote), sorted({r.get("ndx_byte52_apos_queda") for r in sujas_lote})))
    ok("bulk_lote: toda corrida com byte 52 = 1 foi RECUSADA pelo buscar (nunca em silencio)",
       all(r.get("indice_sujo") for r in p2b["corridas"]
           if r.get("ndx_byte52_apos_queda") == 1 and not r["progresso"].get("soltou")),
       str([(r["atraso_ms"], r.get("indice_sujo")) for r in p2b["corridas"]
            if r.get("ndx_byte52_apos_queda") == 1 and not r.get("indice_sujo")]))

    print("\n== 2c. SIGKILL no meio de um reindexar ==")
    print("   esperado: " + ESPERADO["reindexar"])
    p2c = varredura("reindexar", montar_reindex, roteiro_reindex, julgar_reindex,
                    N_ATRASOS_REINDEX)
    silenciosas = [r for r in p2c["corridas"] if "SILENCIO" in r["classe"]]
    ok("reindexar: nenhuma queda deixou indice vazio/parcial em SILENCIO",
       not silenciosas,
       f"{len(silenciosas)} silenciosa(s) de {len(p2c['corridas'])}; classes {p2c['classes']}")
    ok("reindexar: a varredura acertou o meio (sujo detectado) ou os dois extremos",
       len(p2c["classes"]) >= 2, f"{p2c['classes']}")
    ok("reindexar: fora as silenciosas, todo desfecho foi valido",
       all(r["valido"] for r in p2c["corridas"] if "SILENCIO" not in r["classe"]))
    # A hipotese, julgada pelo byte 52 lido ANTES de reabrir: se toda queda no
    # meio (nem antes de comecar, nem depois de terminar) deixou a marca
    # levantada, a janela «criar() limpo -> primeira pagina» nao existe -- e
    # a razao esta no `criar`, que grava a raiz por `gravar_pagina`.
    no_meio = [r for r in p2c["corridas"]
               if not r["classe"].startswith("INTEIRO_")]
    p2c["hipotese_indice_vazio_e_limpo"] = {
        "corridas_no_meio": len(no_meio),
        "byte52_nas_corridas_no_meio": sorted({r.get("ndx_byte52_apos_queda") for r in no_meio},
                                              key=lambda x: (x is None, x)),
        "veredito": ("MORTA: toda queda no meio deixou byte 52 = 1 -- o `criar` ja grava a "
                     "raiz por `gravar_pagina`, que levanta a marca antes de a arvore existir"
                     if no_meio and all(r.get("ndx_byte52_apos_queda") == 1 for r in no_meio)
                     else "CONFIRMADA ou nao alcancada: ver corridas")}
    print("   hipotese «indice vazio e limpo»: %s" % p2c["hipotese_indice_vazio_e_limpo"]["veredito"])

    print("\n== 3. BULKINSERT dentro de uma transacao ==")
    print("   esperado: " + ESPERADO["bulk_em_tx"])
    aceite = bulk_em_tx_e_aceito()
    print("   sem queda: %s" % aceite)
    bruto["bulk_em_tx_sem_queda"] = aceite
    ok("bulk em tx: o motor aceita (ou a recusa esta registrada)",
       aceite["aceito"] or bool(aceite["recusa"]), str(aceite)[:200])
    if aceite["aceito"]:
        p3 = varredura("bulk_em_tx", montar_bulk_tx, roteiro_bulk_tx, julgar_bulk_tx, N_ATRASOS)
        ok("bulk em tx: nunca METADE, indice de pe, marca nenhuma no fim",
           not p3["invalidas"],
           f"{len(p3['invalidas'])} invalida(s) de {len(p3['corridas'])}; classes {p3['classes']}")
        ok("bulk em tx: a varredura mirou dentro da janela (mais de uma classe)",
           len(p3["classes"]) >= 2, f"{p3['classes']}")
        bruto["marca_fica_apos_bulkinsert_false"] = bool(aceite["marcas_logo_apos_bulkinsert_false"])
        ok("bulk em tx: HIPOTESE «bulkinsert(false) deixa a marca .tx do commit no disco»",
           True, "medido: marcas logo apos = %s, 300 ms depois = %s -- hipotese %s"
           % (aceite["marcas_logo_apos_bulkinsert_false"], aceite["marcas_300ms_depois"],
              "CONFIRMADA" if aceite["marcas_logo_apos_bulkinsert_false"] else "MORTA"))
    else:
        p3 = {"esperado": ESPERADO["bulk_em_tx"], "recusa": aceite["recusa"], "corridas": [],
              "classes": {}, "invalidas": []}

    print("\n== 3b. transacao DENTRO de um BULKINSERT (reserva primeiro) ==")
    print("   esperado: " + ESPERADO["tx_em_bulk"])
    aceite_b = tx_em_bulk_sem_queda()
    print("   sem queda: %s" % aceite_b)
    bruto["tx_em_bulk_sem_queda"] = aceite_b
    ok("tx em bulk: o motor aceita (ou a recusa esta registrada)",
       aceite_b["aceito"] or bool(aceite_b["recusa"]), str(aceite_b)[:300])
    if aceite_b["aceito"]:
        # Era linha informativa (a hipotese, sempre True); desde o pedido 254 e
        # veredito: a marca fica pendente no COMMIT (a janela nao fecha na
        # reserva) e SAI no bulkinsert(false), que chama a drenagem do fecho.
        ok("tx em bulk: a marca do COMMIT fica pendente ate o bulkinsert(false) e sai nele (pedido 254)",
           bool(aceite_b["marcas_apos_commit"]) and not aceite_b["marcas_apos_bulkinsert_false"]
           and not aceite_b["marcas_300ms_depois"],
           "medido: apos COMMIT = %s, apos bulkinsert(false) = %s, 300 ms depois = %s"
           % (aceite_b["marcas_apos_commit"], aceite_b["marcas_apos_bulkinsert_false"],
              aceite_b["marcas_300ms_depois"]))
        p3b = varredura("tx_em_bulk", montar_bulk_tx, roteiro_tx_em_bulk, julgar_tx_em_bulk, N_ATRASOS)
        ok("tx em bulk: nunca METADE, indice de pe, marca nenhuma no fim",
           not p3b["invalidas"],
           f"{len(p3b['invalidas'])} invalida(s) de {len(p3b['corridas'])}; classes {p3b['classes']}")
        ok("tx em bulk: a varredura mirou dentro da janela (mais de uma classe)",
           len(p3b["classes"]) >= 2, f"{p3b['classes']}")
    else:
        p3b = {"esperado": ESPERADO["tx_em_bulk"], "recusa": aceite_b["recusa"], "corridas": [],
               "classes": {}, "invalidas": []}

    print("\n== 4. logo DEPOIS do «ok» -- o controle positivo, e o fsync antes dele ==")
    print("   esperado: " + ESPERADO["apos_ok"])
    apos = {"commit": [], "bulk": []}
    for rodada in range(RODADAS):
        apos["commit"].append(queda_apos(commit_n, julgar_apos))
        apos["bulk"].append(queda_apos(bulk_n, julgar_apos))
        print("   r%d commit: %s | bulk: %s" % (
            rodada + 1,
            {k: apos["commit"][-1][k] for k in ("registros", "indice_sujo", "achadas")},
            {k: apos["bulk"][-1][k] for k in ("registros", "indice_sujo", "achadas")}))
    ok("apos COMMIT ok + SIGKILL: as %d linhas la, achadas pela chave, em todas as rodadas" % N_APOS,
       all(r["registros"] == N_APOS and r["achadas"] == N_APOS and r["verificar"][0] == "ok"
           for r in apos["commit"]),
       str([(r["registros"], r["achadas"], r["indice_sujo"]) for r in apos["commit"]]))
    ok("apos COMMIT ok: o arranque REPORTA a marca pendente (janela aberta) e a apaga",
       all(r["relatorio"] is not None and r["relatorio"]["ja_aplicadas"] + r["relatorio"]["reaplicadas"] == N_APOS
           and not r["marcas_depois"] for r in apos["commit"]),
       str([(r["marcas_antes_de_reabrir"], r["relatorio"] and
             {k: v for k, v in r["relatorio"].items() if k != "impossiveis_linhas"}) for r in apos["commit"]]))
    ok("apos BULKINSERT(false) ok + SIGKILL: as %d linhas la, indice LIMPO sem reindexar, arranque calado" % N_APOS,
       all(r["registros"] == N_APOS and r["achadas"] == N_APOS and not r["indice_sujo"]
           and r["relatorio"] is None and r["verificar"][0] == "ok" for r in apos["bulk"]),
       str([(r["registros"], r["achadas"], r["indice_sujo"], r["relatorio"]) for r in apos["bulk"]]))

    fs = {"ferramenta": "strace %s" % (subprocess.run(["strace", "-V"], capture_output=True,
                                                     text=True).stdout.split("\n")[0]
                                       if shutil.which("strace") else "AUSENTE")}
    if shutil.which("strace"):
        fs["commit"] = fsync_por_extensao(commit_n)
        fs["bulkinsert"] = fsync_por_extensao(bulk_n)
        print("   fsync antes do ok -- commit: %s | bulkinsert(false): %s" % (fs["commit"], fs["bulkinsert"]))
        ok("fsync: o COMMIT sincroniza a marca .tx antes do ok",
           fs["commit"].get("tx", 0) >= 1, str(fs["commit"]))
        ok("fsync: o COMMIT com a janela aberta NAO sincroniza .reg nem .ndx (a marca e o bilhete)",
           fs["commit"].get("reg", 0) == 0 and fs["commit"].get("ndx", 0) == 0, str(fs["commit"]))
        ok("fsync: o bulkinsert(false) sincroniza .reg e .ndx antes do ok, e marca nenhuma",
           fs["bulkinsert"].get("reg", 0) >= 1 and fs["bulkinsert"].get("ndx", 0) >= 1
           and fs["bulkinsert"].get("tx", 0) == 0, str(fs["bulkinsert"]))
    else:
        fs["nao_medido"] = "strace ausente nesta maquina"
        print("   fsync NAO medido: strace ausente")
    apos["fsync"] = fs

    bruto["pontos"] = {"tx_aberta": p1, "bulk": p2, "bulk_lote": p2b, "reindexar": p2c,
                       "bulk_em_tx": p3, "tx_em_bulk": p3b,
                       "apos_ok": {"esperado": ESPERADO["apos_ok"], "rodadas": RODADAS, **apos}}
    bruto["conferencias"] = len(CONFERENCIAS)
    bruto["falhas"] = FALHAS
    bruto["detalhe"] = CONFERENCIAS
    bruto["duracao_s"] = round(time.time() - inicio, 1)
    bruto["tabela"] = tabela_resumo(bruto["pontos"])

    shutil.rmtree(BASE, ignore_errors=True)
    apagar_copia_do_binario()
    print("\n== ponto x atrasos x desfechos ==")
    for linha in bruto["tabela"]:
        print("  " + linha)
    if RAPIDO:
        print("\n[rapido: resultado NAO gravado]")
    else:
        with open(RESULTADO, "w", encoding="utf-8") as f:
            json.dump(bruto, f, indent=1, ensure_ascii=False)
        print("\nresultado medido gravado em %s" % RESULTADO)
    print("\n%d conferencias, %d falha(s), %.0f s" % (len(CONFERENCIAS), len(FALHAS), bruto["duracao_s"]))
    for f_ in FALHAS:
        print("  - " + f_)
    return 1 if FALHAS else 0


def tabela_resumo(pontos):
    linhas = []
    for nome, p in pontos.items():
        if nome == "apos_ok":
            continue
        if not p.get("corridas"):
            linhas.append("%-12s recusado: %s" % (nome, p.get("recusa")))
            continue
        classes = " · ".join("%s=%d" % (k, v) for k, v in sorted(p["classes"].items()))
        linhas.append("%-12s %3d atrasos x %d rodadas = %3d corridas, calibracao %7.1f ms, invalidas %d | %s"
                      % (nome, p["atrasos"], p["rodadas"], len(p["corridas"]),
                         p["t_calibracao_ms"], len(p["invalidas"]), classes))
    a = pontos["apos_ok"]
    linhas.append("apos_ok      commit x%d: %s | bulk x%d: %s | fsync: %s"
                  % (len(a["commit"]), [r["registros"] for r in a["commit"]],
                     len(a["bulk"]), [r["registros"] for r in a["bulk"]],
                     {k: v for k, v in a["fsync"].items() if k != "ferramenta"}))
    return linhas


if __name__ == "__main__":
    sys.exit(main())
