#!/usr/bin/env python3
"""As quatro letras do ACID, uma a uma, provadas contra o `phxsqld` de pe.

    cargo build --release
    python3 bancada/acid/prova.py

# Por que esta bancada existe

O `CLAUDE.md` traz a lei em vigor, e ela esta DATADA: «sem transacao nao ha o A
nem o I do ACID». A premissa envelheceu -- ha transacao desde a SP000006, e ela
enxerga a propria escrita. A pergunta deixou de ser «ha transacao?» e passou a
ser «o que cada letra garante, medido?».

Este script nao decide a frase da marca. Ele MEDE, afirmacao por afirmacao, e
grava `resultado.json`. Quem escreve o `docs/ACID.md` e o `gerar-secoes.py`, a
partir desse arquivo -- nenhum numero do documento e digitado.

# O metodo, e a regra que ele obedece

Cada afirmacao tem um CONTROLE na mesma corrida. A casa ja publicou zero com um
medidor cego, e a licao ficou: «nao aconteceu» so vale quando o mesmo
instrumento, no mesmo servidor, na mesma corrida, ACUSA o caso oposto.

  - «leitura suja NAO acontece» so vale porque, na mesma corrida, o mesmo `ler`
    ACUSA a propria escrita da transacao (o read-your-own-writes). Instrumento
    que enxerga uma linha nao gravada enxergaria uma linha suja.
  - «uma instrucao le um estado consistente» so vale porque, na mesma corrida,
    o par de instrucoes SEPARADAS acusa a soma quebrada.
  - «o `.reg` nao vai ao disco em `por_lote`» so vale porque, na mesma corrida,
    `por_operacao` mostra o `.reg` indo.

# O que este script NAO mede, de proposito

TEMPO. Nenhum numero daqui e uma duracao -- sao contagens, vereditos e
`fsync` contados por `strace`, que sao determinsticos e imunes a maquina
ocupada. O portao `bancada/esta-medindo.sh` e consultado por cortesia e
registrado no resultado, mas nao decide nada: nao ha o que ele proteja aqui.

QUEDA DE ENERGIA. O `SIGKILL` mata o processo, e o nucleo fica com as paginas
sujas -- um `write` ja entregue sobrevive a morte de quem o escreveu. Por isso
a durabilidade e medida por CONTAGEM DE `fsync`, e o `SIGKILL` entra so para o
que ele realmente prova: que a marca `.tx` decide o desfecho. A secao D diz
isso com todas as letras.

Mata so os PIDs que ele mesmo subiu. Nunca `pkill -f`.
"""
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.environ.get("PHX_RAIZ", os.path.abspath(os.path.join(AQUI, "..", "..")))

# Reuso, e nao terceira copia: subir/matar/relatorio ja estao escritos na
# bancada de durabilidade, e este script so troca a porta e o diretorio.
sys.path.insert(0, os.path.join(RAIZ, "bancada", "durabilidade"))
import prova as dur  # noqa: E402

dur.PORTA = 7570
dur.BASE = os.path.join(AQUI, ".base-da-prova")
dur.DB = "acid"
PORTA, BASE, DB = dur.PORTA, dur.BASE, dur.DB
RESULTADO = os.path.join(AQUI, "resultado.json")
REGIMES = ["por_operacao", "por_lote", "sistema"]

# Atalho de DEPURACAO: pula as varreduras de `SIGKILL`, que sao a parte lenta.
# Nunca se publica numero de uma corrida rapida -- o resultado grava a marca.
RAPIDO = os.environ.get("PHX_ACID_RAPIDO") == "1"
SEM_QUEDA = {"corridas": []}  # o que uma varredura pulada devolve

AFIRMACOES = []


def afirmar(letra, chave, frase, esperado, medido, controle=None, nota=None):
    """Uma afirmacao do documento, com o que se esperava, o que se mediu, e o
    CONTROLE que prova que o instrumento nao estava cego."""
    ok = medido == esperado
    AFIRMACOES.append({
        "letra": letra, "chave": chave, "frase": frase,
        "esperado": esperado, "medido": medido, "ok": ok,
        "controle": controle, "nota": nota,
    })
    print("  %-4s %-46s %s" % ("ok" if ok else "NAO", chave, medido))
    if controle:
        print("       controle: %s" % (controle,))
    return ok


def liga(prazo=20):
    return dur.Ligacao(porta=PORTA, prazo=prazo)


def subir(regime="por_lote", limpar=True, **kw):
    cfg = dur.config(regime, **kw)
    cfg["max_linhas"] = 100_000
    return dur.subir(cfg, limpar=limpar)


def conta(c, tabela):
    """A contagem vem do `varrer`, que e a unica leitura de conjunto do
    protocolo -- nao ha `contar` na porta de dados."""
    r = c.fala({"op": "varrer", "database": DB, "tabela": tabela, "limite": 100_000})
    if not r.get("ok"):
        return -1
    return r["resultado"]["registros"]


def linhas(c, tabela):
    r = c.fala({"op": "varrer", "database": DB, "tabela": tabela, "limite": 100_000})
    return r["resultado"]["linhas"] if r.get("ok") else []


def tabela_simples(c, nome, colunas=None):
    c.ok({"op": "criar_tabela", "database": DB, "tabela": nome,
          "colunas": colunas or [
              {"nome": "id", "tipo": "Int8", "obrigatoria": True},
              {"nome": "v", "tipo": "Int8"}],
          "indices": [{"nome": "pk", "colunas": ["id"], "unico": True,
                       "primario": True}]})


def par_mae_filha(c, mae="maes", filha="filhas", ao_alterar="cascata",
                  verificar=None, indice_na_filha=True):
    c.ok({"op": "criar_tabela", "database": DB, "tabela": mae,
          "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                      {"nome": "n", "tipo": "Str(20)"}],
          "indices": [{"nome": "pk", "colunas": ["id"], "unico": True,
                       "primario": True}]})
    fk = {"nome": "fk", "colunas": ["mid"], "tabela_ref": mae,
          "colunas_ref": ["id"], "ao_excluir": "restringir",
          "ao_alterar": ao_alterar}
    if verificar is not None:
        fk["verificar"] = verificar
    idx = [{"nome": "pk", "colunas": ["id"], "unico": True, "primario": True}]
    if indice_na_filha:
        idx.append({"nome": "porMae", "colunas": ["mid"]})
    return c.fala({"op": "criar_tabela", "database": DB, "tabela": filha,
                   "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                               {"nome": "mid", "tipo": "Int8"}],
                   "indices": idx, "chaves_estrangeiras": [fk]})


def nome_do_erro(r):
    return r.get("nome") if not r.get("ok") else "ACEITOU"


# `dur.Ligacao` fixou a porta no DEFAULT do `__init__`, e o Python avalia
# default na DEFINICAO da classe: trocar `dur.PORTA` depois nao alcanca aquele
# valor. Esta subclasse existe so para isso, e e rebindada no modulo porque o
# proprio `dur.corrida()` chama `Ligacao()` sem argumento nenhum.
class _NaMinhaPorta(dur.Ligacao):
    def __init__(self, porta=None, prazo=20):
        super().__init__(porta or PORTA, prazo)


dur.Ligacao = _NaMinhaPorta


# =========================================================== A -- atomicidade

def letra_a():
    print("\n=== A -- ATOMICIDADE ===")
    bruto = {}

    # ---- A1: o ROLLBACK nao queima slot, e o COMMIT queima -----------------
    # A regra petrea da casa e que o `.reg` nunca reaproveita slot excluido.
    # Dai o desenho de «nada vai a disco antes do COMMIT»: o rollback de um
    # insert e zero byte de trabalho. O controle e o COMMIT na mesma corrida --
    # sem ele, «slots nao mudou» poderia ser um `esquema` que nao conta nada.
    p, _ = subir("por_lote")
    try:
        c = liga()
        c.ok({"op": "criar_database", "database": DB})
        tabela_simples(c, "s")
        c.ok({"op": "inserir", "database": DB, "tabela": "s", "linha": {"id": 1, "v": 1}})
        antes = c.ok({"op": "esquema", "database": DB, "tabela": "s"})["slots"]
        c.ok({"op": "begin", "database": DB})
        for i in (2, 3, 4):
            c.ok({"op": "inserir", "database": DB, "tabela": "s", "linha": {"id": i, "v": i}})
        c.ok({"op": "rollback"})
        depois_rb = c.ok({"op": "esquema", "database": DB, "tabela": "s"})["slots"]
        c.ok({"op": "begin", "database": DB})
        for i in (2, 3, 4):
            c.ok({"op": "inserir", "database": DB, "tabela": "s", "linha": {"id": i, "v": i}})
        c.ok({"op": "commit"})
        depois_cm = c.ok({"op": "esquema", "database": DB, "tabela": "s"})["slots"]
        bruto["slots"] = {"antes": antes, "apos_rollback": depois_rb, "apos_commit": depois_cm}
        afirmar("A", "rollback_nao_queima_slot",
                "o ROLLBACK de tres INSERT nao consome slot nenhum do `.reg`",
                antes, depois_rb,
                controle="o COMMIT das mesmas tres consome 3 (%d -> %d)"
                         % (depois_rb, depois_cm))
        afirmar("A", "commit_consome_os_slots",
                "o controle: o COMMIT das mesmas tres linhas consome 3 slots",
                antes + 3, depois_cm)
        c.fechar()
    finally:
        dur.derrubar_limpo(p)

    # ---- A2: queda no meio de um COMMIT que toca DUAS tabelas --------------
    # `SIGKILL` de verdade, varredura de atrasos. A pergunta do contrato nao e
    # «as N linhas estao la?», e «o banco consegue dizer, sem ambiguidade,
    # qual dos dois desfechos aconteceu?» -- entao o que reprova e METADE.
    print("\n-- A2: SIGKILL no meio de um COMMIT que toca duas tabelas --")
    v = SEM_QUEDA if RAPIDO else dur.varredura_dois_tabelas("por_lote", n_a=400, n_b=400, passos=7)
    metades = []
    validas = 0
    for r in v["corridas"]:
        reg = r.get("registros", {})
        a_, b_ = reg.get("a", -1), reg.get("b", -1)
        if r.get("tarde_demais"):
            continue
        validas += 1
        if not ((a_, b_) == (0, 0) or (a_, b_) == (400, 400)):
            metades.append((r["rotulo"], a_, b_))
    bruto["a2"] = {"corridas": len(v["corridas"]), "validas": validas,
                   "metades": metades,
                   "desfechos": [[r["rotulo"], r.get("classe"),
                                  r.get("registros", {}).get("a"),
                                  r.get("registros", {}).get("b")]
                                 for r in v["corridas"]]}
    afirmar("A", "commit_multitabela_nunca_metade",
            "em %d quedas validas no meio de um COMMIT de duas tabelas, "
            "nenhuma deixou uma tabela com linha e a outra sem" % validas,
            0, len(metades),
            controle="a varredura pegou %s -- pontos de morte diferentes, e e "
                     "isso que prova que ela mirou dentro da janela"
                     % " · ".join(sorted({(r.get("classe") or "?").split(" ")[0]
                                          for r in v["corridas"]})))

    # ---- A3: a cascata recusa ANTES da primeira escrita --------------------
    # O pedido 163 escreveu «nao ha transacao: a cascata nao e atomica». O
    # pedido 169 mudou metade disso: `conferir_a_arvore` roda antes de gravar.
    # Aqui se mede a metade que FECHOU, por soquete e nao por teste unitario.
    p, _ = subir("por_lote")
    try:
        c = liga()
        c.ok({"op": "criar_database", "database": DB})
        # avo <- mae (cascata) <- neta (restringir).
        #
        # A CHAVE DA MONTAGEM, e ela custou uma corrida: a coluna que a mae
        # aponta para a avo tem de ser a MESMA que a neta aponta na mae. Numa
        # primeira versao a mae tinha `id` proprio e uma coluna `pai` separada;
        # a cascata mudava `mae.pai` e a neta, que apontava para `mae.id`, nao
        # era alcancada por nada -- o teste passava sem exercitar o caso, que e
        # a forma de prova que esta casa mais recusa. Aqui a chave da mae E o
        # `id` dela, entao a cascata desce de verdade dois niveis.
        c.ok({"op": "criar_tabela", "database": DB, "tabela": "avo",
              "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                          {"nome": "n", "tipo": "Str(20)"}],
              "indices": [{"nome": "pk", "colunas": ["id"], "unico": True,
                           "primario": True}]})
        c.ok({"op": "criar_tabela", "database": DB, "tabela": "mae",
              "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                          {"nome": "n", "tipo": "Str(20)"}],
              "indices": [{"nome": "pk", "colunas": ["id"], "unico": True,
                           "primario": True}],
              "chaves_estrangeiras": [
                  {"nome": "fk", "colunas": ["id"], "tabela_ref": "avo",
                   "colunas_ref": ["id"], "ao_excluir": "restringir",
                   "ao_alterar": "cascata"}]})
        c.ok({"op": "criar_tabela", "database": DB, "tabela": "neta",
              "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                          {"nome": "pai", "tipo": "Int8"}],
              "indices": [{"nome": "pk", "colunas": ["id"], "unico": True,
                           "primario": True},
                          {"nome": "porPai", "colunas": ["pai"]}],
              "chaves_estrangeiras": [
                  {"nome": "fk", "colunas": ["pai"], "tabela_ref": "mae",
                   "colunas_ref": ["id"], "ao_excluir": "restringir",
                   "ao_alterar": "restringir"}]})
        c.ok({"op": "inserir", "database": DB, "tabela": "avo", "linha": {"id": 1, "n": "a"}})
        c.ok({"op": "inserir", "database": DB, "tabela": "mae", "linha": {"id": 1, "n": "m"}})
        c.ok({"op": "inserir", "database": DB, "tabela": "neta", "linha": {"id": 1, "pai": 1}})
        r = c.fala({"op": "atualizar", "database": DB, "tabela": "avo",
                    "rowid": 1, "linha": {"id": 77, "n": "a"}})
        avo_depois = c.ok({"op": "ler", "database": DB, "tabela": "avo", "rowid": 1})["id"]
        mae_depois = c.ok({"op": "ler", "database": DB, "tabela": "mae", "rowid": 1})["id"]
        bruto["a3"] = {"erro": nome_do_erro(r), "avo_id": avo_depois, "mae_id": mae_depois}
        afirmar("A", "cascata_recusa_antes_de_gravar",
                "com avo<-mae(cascata)<-neta(restringir), trocar a chave da avo "
                "e recusado e a AVO fica no valor antigo",
                [1, 1], [avo_depois, mae_depois],
                controle="a recusa e nomeada: %s" % bruto["a3"]["erro"])
        # controle positivo: sem a neta restringindo, a mesma alteracao PASSA
        # De vez, e nao suave: filha logicamente morta CONTINUA restringindo a
        # mae (medido na secao C) -- apagar suave aqui deixaria o controle
        # medindo o mesmo caso do teste, com outro nome.
        c.ok({"op": "excluir", "database": DB, "tabela": "neta", "rowid": 1,
              "fisico": True})
        r2 = c.fala({"op": "atualizar", "database": DB, "tabela": "avo",
                     "rowid": 1, "linha": {"id": 77, "n": "a"}})
        avo2 = c.ok({"op": "ler", "database": DB, "tabela": "avo", "rowid": 1})["id"]
        mae2 = c.ok({"op": "ler", "database": DB, "tabela": "mae", "rowid": 1})["id"]
        bruto["a3"]["sem_neta"] = {"ok": r2.get("ok"), "avo_id": avo2, "mae_id": mae2}
        afirmar("A", "cascata_alcanca_a_filha_quando_nao_ha_restricao",
                "o controle: tirada a neta, a mesma alteracao passa e a MAE "
                "acompanha a avo",
                [77, 77], [avo2, mae2])
        c.fechar()
    finally:
        dur.derrubar_limpo(p)

    # ---- A4: a cascata NAO e atomica sob QUEDA -----------------------------
    # A outra metade do pedido 163, e ela continua aberta: `aplicar_ao_alterar`
    # reescreve as filhas uma a uma e sincroniza cada uma no fim do laco dela.
    # Uma queda no meio deixa a mae no valor novo e parte das filhas no velho.
    # O criterio nao e «isso e defeito», e «o relatorio DENUNCIOU?».
    print("\n-- A4: SIGKILL no meio da cascata do ao_alterar --")
    vc = SEM_QUEDA if RAPIDO else dur.varredura_cascata(
        # 1.200 filhas e 7 passos sao os MESMOS parametros da matriz publicada
        # em `docs/TRANSACOES.md` SS5.7, e de proposito: aquela matriz mediu 3 de
        # 7 PARCIAL_DENUNCIADO por regime, ANTES de o pedido 172 por a
        # recuperacao a reconstruir o indice da filha. Com outros parametros
        # esta corrida nao poderia ser comparada com aquela, e o que se quer
        # saber e exatamente se o numero mudou.
        "por_lote", filhas=1200, passos=7)
    vereditos = {}
    for r in vc["corridas"]:
        vereditos[r["veredito"]] = vereditos.get(r["veredito"], 0) + 1
    # `classe` e `indices_reconstruidos` entram porque o veredito sozinho NAO
    # distingue as duas maneiras de sair consistente: a queda pode ter caido
    # antes da passada (e ai a reaplicacao refaz tudo do zero) ou no meio do
    # laco que reescreve as filhas (e ai quem salva e a reconstrucao do `.ndx`
    # da filha, o conserto do pedido 172). Sem esses dois campos, «7 de 7
    # consistentes» e um numero que nao diz por que.
    bruto["a4"] = {"vereditos": vereditos,
                   "corridas": [[r["rotulo"], r["veredito"], r.get("classe"),
                                 (r.get("relatorio") or {}).get("indices_reconstruidos"),
                                 r.get("filhas_com_mae_nova"),
                                 r.get("filhas_com_mae_velha")] for r in vc["corridas"]]}
    parciais = vereditos.get("PARCIAL_DENUNCIADO", 0)
    silenciosas = vereditos.get("*** PARCIAL SEM AVISO ***", 0)
    afirmar("A", "cascata_parcial_nunca_em_silencio",
            "nenhuma queda no meio da cascata deixou mae e filhas divergentes "
            "SEM o relatorio do arranque denunciar",
            0, silenciosas,
            controle="a corrida achou %d cascata(s) parcial(is) denunciada(s) "
                     "em %d, e %d consistente(s) -- o instrumento distingue os dois"
                     % (parciais, len(vc["corridas"]),
                        vereditos.get("CONSISTENTE", 0)))
    return bruto


# =========================================================== C -- consistencia

def letra_c():
    """O que e IMPOSTO na gravacao, e o que e so DECLARADO.

    Cada guarda entra com o par: a violacao RECUSADA e o caso legitimo
    ACEITO, na mesma corrida e na mesma tabela. Uma guarda que recusa tudo
    protegeria o mesmo numero e nao serviria para nada -- e essa e a forma de
    teste que esta casa ja pagou uma vez (`ao_excluir_so_aceita_restringir` tem
    um irmao exatamente por isso)."""
    print("\n=== C -- CONSISTENCIA ===")
    bruto = {}
    p, _ = subir("por_lote")
    try:
        c = liga()
        c.ok({"op": "criar_database", "database": DB})
        par_mae_filha(c)
        c.ok({"op": "inserir", "database": DB, "tabela": "maes", "linha": {"id": 1, "n": "mae"}})

        def par(chave, frase, violacao, legitimo):
            """Aplica a violacao e o caso legitimo, e afirma os DOIS."""
            rv = c.fala(violacao)
            rl = c.fala(legitimo)
            bruto[chave] = {"violacao": nome_do_erro(rv), "legitimo": rl.get("ok")}
            return afirmar("C", chave, frase, ["RECUSADO", True],
                           ["RECUSADO" if not rv.get("ok") else "ACEITOU", rl.get("ok")],
                           controle="o caso legitimo passa na mesma tabela; a recusa "
                                    "e %s" % nome_do_erro(rv))

        par("unicidade",
            "indice unico recusa chave repetida",
            {"op": "inserir", "database": DB, "tabela": "maes", "linha": {"id": 1, "n": "x"}},
            {"op": "inserir", "database": DB, "tabela": "maes", "linha": {"id": 2, "n": "x"}})
        par("chave_estrangeira",
            "a filha sem mae e recusada na GRAVACAO, nao so declarada",
            {"op": "inserir", "database": DB, "tabela": "filhas", "linha": {"id": 1, "mid": 99}},
            {"op": "inserir", "database": DB, "tabela": "filhas", "linha": {"id": 1, "mid": 1}})
        par("obrigatoria",
            "coluna obrigatoria recusa NULL",
            {"op": "inserir", "database": DB, "tabela": "maes", "linha": {"n": "sem id"}},
            {"op": "inserir", "database": DB, "tabela": "maes", "linha": {"id": 3, "n": "com id"}})
        par("tipo",
            "texto numa coluna inteira e recusado",
            {"op": "inserir", "database": DB, "tabela": "maes", "linha": {"id": "abc", "n": "x"}},
            {"op": "inserir", "database": DB, "tabela": "maes", "linha": {"id": 4, "n": "x"}})
        par("tamanho",
            "texto que nao cabe em Str(20) e recusado, nunca truncado",
            {"op": "inserir", "database": DB, "tabela": "maes", "linha": {"id": 5, "n": "x" * 40}},
            {"op": "inserir", "database": DB, "tabela": "maes", "linha": {"id": 5, "n": "x" * 20}})

        # ---- a regra primordial, nos DOIS excluires -------------------------
        # «Nunca se mata o pai que tem filhos». O suave conta: pai logicamente
        # morto deixa filha apontando para linha que a tela nao mostra mais, e
        # orfa que ninguem ve e pior que orfa que da erro.
        # O `excluir` nasce SUAVE quando a tabela tem coluna de marca; o modo
        # fisico se pede com `"fisico": true`. Uma primeira versao deste teste
        # mandava `"modo": "suave"`, que o servidor nem le -- os dois casos
        # exercitavam o MESMO caminho, e o par «de vez E suave» era uma
        # afirmacao sobre uma coisa so.
        rv = c.fala({"op": "excluir", "database": DB, "tabela": "maes", "rowid": 1,
                     "fisico": True})
        rs = c.fala({"op": "excluir", "database": DB, "tabela": "maes", "rowid": 1,
                     "motivo": "prova"})
        rl = c.fala({"op": "excluir", "database": DB, "tabela": "maes", "rowid": 2,
                     "fisico": True})
        bruto["regra_primordial"] = {"de_vez": nome_do_erro(rv), "suave": nome_do_erro(rs),
                                     "sem_filha": rl.get("ok")}
        afirmar("C", "regra_primordial",
                "excluir a mae que tem filha e recusado -- de vez E suave",
                ["RECUSADO", "RECUSADO", True],
                ["RECUSADO" if not rv.get("ok") else "ACEITOU",
                 "RECUSADO" if not rs.get("ok") else "ACEITOU", rl.get("ok")],
                controle="a mae SEM filha (rowid 2) e excluida na mesma corrida")

        # ---- a filha logicamente morta CONTINUA sendo filha ------------------
        # Consequencia direta da petrea «orfa que ninguem ve e pior que orfa
        # que da erro»: o `conferir_filhas` conta a linha marcada. Isso NAO e
        # simetrico com o `conferir_fks` do INSERT, que pergunta «a mae esta
        # VIVA?» (pedido 171) -- e a assimetria e a resposta certa nos dois
        # lados, porque um deles protege quem nasce e o outro quem some.
        par_mae_filha(c, mae="ms", filha="fs")
        c.ok({"op": "inserir", "database": DB, "tabela": "ms", "linha": {"id": 1, "n": "m"}})
        c.ok({"op": "inserir", "database": DB, "tabela": "ms", "linha": {"id": 2, "n": "m2"}})
        c.ok({"op": "inserir", "database": DB, "tabela": "fs", "linha": {"id": 1, "mid": 1}})
        c.ok({"op": "excluir", "database": DB, "tabela": "fs", "rowid": 1})   # suave
        r_com_morta = c.fala({"op": "excluir", "database": DB, "tabela": "ms",
                              "rowid": 1, "fisico": True})
        r_sem_filha = c.fala({"op": "excluir", "database": DB, "tabela": "ms",
                              "rowid": 2, "fisico": True})
        # e a outra ponta da assimetria: mae marcada nao aceita filha nova
        c.ok({"op": "inserir", "database": DB, "tabela": "ms", "linha": {"id": 3, "n": "m3"}})
        c.ok({"op": "excluir", "database": DB, "tabela": "ms", "rowid": 3})   # suave
        r_mae_morta = c.fala({"op": "inserir", "database": DB, "tabela": "fs",
                              "linha": {"id": 2, "mid": 3}})
        bruto["marca_de_exclusao"] = {
            "mae_com_filha_marcada": nome_do_erro(r_com_morta),
            "mae_sem_filha": r_sem_filha.get("ok"),
            "filha_de_mae_marcada": nome_do_erro(r_mae_morta)}
        afirmar("C", "a_marca_de_exclusao_conta_dos_dois_lados",
                "filha marcada continua restringindo a mae, e mae marcada nao "
                "aceita filha nova",
                ["RECUSADO", True, "RECUSADO"],
                ["RECUSADO" if not r_com_morta.get("ok") else "ACEITOU",
                 r_sem_filha.get("ok"),
                 "RECUSADO" if not r_mae_morta.get("ok") else "ACEITOU"],
                controle="a mae SEM filha nenhuma sai de vez na mesma corrida")

        # ---- a chave DECLARADA nasce conferida ------------------------------
        # E o interruptor so existe para o lado contrario: quem QUER declarar
        # sem conferir manda `verificar: false`, e ai e escolha escrita.
        par_mae_filha(c, mae="m2", filha="f2")                       # sem `verificar`
        c.ok({"op": "inserir", "database": DB, "tabela": "m2", "linha": {"id": 1, "n": "m"}})
        r_nasce = c.fala({"op": "inserir", "database": DB, "tabela": "f2", "linha": {"id": 1, "mid": 99}})
        par_mae_filha(c, mae="m3", filha="f3", verificar=False)
        r_desligada = c.fala({"op": "inserir", "database": DB, "tabela": "f3", "linha": {"id": 1, "mid": 99}})
        bruto["nasce_conferida"] = {"sem_pedir": nome_do_erro(r_nasce),
                                    "verificar_false": r_desligada.get("ok")}
        afirmar("C", "chave_declarada_nasce_conferida",
                "chave declarada sem pedir `verificar` JA confere; com "
                "`verificar: false` a orfa entra",
                ["RECUSADO", True],
                ["RECUSADO" if not r_nasce.get("ok") else "ACEITOU", r_desligada.get("ok")],
                controle="o mesmo INSERT orfao, nas duas tabelas, da os dois "
                         "desfechos -- o instrumento nao esta recusando tudo")

        # ---- o indice dos dois lados ---------------------------------------
        # E aqui a recusa muda de lugar, e isso e o achado desta secao: a falta
        # do indice NAO e recusada na declaracao (ao contrario do `ao_excluir`)
        # -- ela e recusada na GRAVACAO, quando a mae tenta morrer. Um parecer
        # com as tres saidas esta em `docs/PARECER-175-INDICE-NA-DECLARACAO.md`.
        r_decl = par_mae_filha(c, mae="m4", filha="f4", indice_na_filha=False)
        c.ok({"op": "inserir", "database": DB, "tabela": "m4", "linha": {"id": 1, "n": "m"}})
        r_del = c.fala({"op": "excluir", "database": DB, "tabela": "m4", "rowid": 1})
        bruto["indice_dos_dois_lados"] = {
            "declaracao": "ACEITOU" if r_decl.get("ok") else nome_do_erro(r_decl),
            "excluir_a_mae": nome_do_erro(r_del),
            "erro": (r_del.get("erro") or "")[:200]}
        afirmar("C", "indice_faltando_recusa_na_gravacao",
                "chave sem indice na filha e ACEITA na declaracao e recusada no "
                "`excluir` da mae, nomeando o indice que falta",
                ["ACEITOU", "INTEGRIDADE"],
                ["ACEITOU" if r_decl.get("ok") else nome_do_erro(r_decl),
                 nome_do_erro(r_del)],
                controle="a mesma mae, com indice na filha (`maes`), recusa "
                         "com o texto da regra primordial e nao com este")

        # ---- `ao_excluir` so aceita `restringir`, e a recusa e na DECLARACAO -
        r_casc = par_mae_filha(c, mae="m5", filha="f5")
        r_casc_ruim = c.fala({"op": "criar_tabela", "database": DB, "tabela": "f6",
                              "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                                          {"nome": "mid", "tipo": "Int8"}],
                              "indices": [{"nome": "pk", "colunas": ["id"], "unico": True,
                                           "primario": True},
                                          {"nome": "porMae", "colunas": ["mid"]}],
                              "chaves_estrangeiras": [
                                  {"nome": "fk", "colunas": ["mid"], "tabela_ref": "m5",
                                   "colunas_ref": ["id"], "ao_excluir": "cascata",
                                   "ao_alterar": "cascata"}]})
        bruto["ao_excluir"] = {"cascata": nome_do_erro(r_casc_ruim),
                               "restringir": r_casc.get("ok")}
        afirmar("C", "ao_excluir_so_aceita_restringir",
                "`ao_excluir: cascata` e recusado na DECLARACAO; `restringir` passa",
                ["RECUSADO", True],
                ["RECUSADO" if not r_casc_ruim.get("ok") else "ACEITOU", r_casc.get("ok")],
                controle="a mesma tabela, so trocando a acao, nasce ou nao nasce")
        c.fechar()
    finally:
        dur.derrubar_limpo(p)
    return bruto


# ============================================================= I -- isolamento

def letra_i():
    """Os fenomenos da norma, nomeados e provados um a um, por soquete.

    Um fenomeno so se prova ACONTECENDO. O que nao acontece se prova pelo
    controle: o mesmo instrumento, no mesmo servidor, na mesma corrida,
    mostrando o caso oposto."""
    print("\n=== I -- ISOLAMENTO ===")
    bruto = {}
    p, _ = subir("por_lote", lote_ms=3_600_000)
    try:
        a = liga()
        b = liga()
        a.ok({"op": "criar_database", "database": DB})
        tabela_simples(a, "c")
        for i in (1, 2):
            a.ok({"op": "inserir", "database": DB, "tabela": "c", "linha": {"id": i, "v": 50}})

        # ---- I1: leitura suja -- NAO acontece -------------------------------
        # E o CONTROLE mora dentro do mesmo teste: a propria transacao ENXERGA
        # o valor novo (read-your-own-writes, pedido 162). Um `ler` cego a uma
        # linha nao gravada seria cego a uma linha suja tambem, e o «nao
        # acontece» nao valeria nada.
        a.ok({"op": "begin", "database": DB})
        a.ok({"op": "atualizar", "database": DB, "tabela": "c", "rowid": 1,
              "linha": {"id": 1, "v": 999}})
        dentro = a.ok({"op": "ler", "database": DB, "tabela": "c", "rowid": 1})["v"]
        fora = b.ok({"op": "ler", "database": DB, "tabela": "c", "rowid": 1})["v"]
        a.ok({"op": "rollback"})
        depois = b.ok({"op": "ler", "database": DB, "tabela": "c", "rowid": 1})["v"]
        bruto["leitura_suja"] = {"dentro": dentro, "outra_sessao": fora, "apos_rollback": depois}
        afirmar("I", "leitura_suja",
                "outra sessao NAO ve a escrita nao confirmada",
                [999, 50, 50], [dentro, fora, depois],
                controle="a PROPRIA transacao ve 999 na mesma corrida -- o "
                         "instrumento enxerga escrita nao gravada quando ela e dela")

        # ---- I2: leitura nao repetivel -- ACONTECE ---------------------------
        a.ok({"op": "begin", "database": DB})
        um = a.ok({"op": "ler", "database": DB, "tabela": "c", "rowid": 2})["v"]
        b.ok({"op": "atualizar", "database": DB, "tabela": "c", "rowid": 2,
              "linha": {"id": 2, "v": 77}})
        dois = a.ok({"op": "ler", "database": DB, "tabela": "c", "rowid": 2})["v"]
        a.ok({"op": "rollback"})
        bruto["leitura_nao_repetivel"] = {"primeira": um, "segunda": dois}
        afirmar("I", "leitura_nao_repetivel",
                "duas leituras da MESMA linha dentro da mesma transacao "
                "devolvem valores diferentes",
                True, um != dois,
                controle="a primeira leu %s e a segunda %s, com um COMMIT de "
                         "outra sessao no meio" % (um, dois))

        # ---- I3: fantasma -- ACONTECE ----------------------------------------
        a.ok({"op": "begin", "database": DB})
        n1 = conta(a, "c")
        b.ok({"op": "inserir", "database": DB, "tabela": "c", "linha": {"id": 3, "v": 1}})
        n2 = conta(a, "c")
        a.ok({"op": "rollback"})
        bruto["fantasma"] = {"primeira": n1, "segunda": n2}
        afirmar("I", "fantasma",
                "a mesma varredura, repetida dentro da transacao, devolve uma "
                "linha que nao existia na primeira",
                True, n2 > n1,
                controle="%d -> %d linhas, com um INSERT de outra sessao no meio"
                         % (n1, n2))

        # ---- I4: perda de atualizacao ---------------------------------------
        # Tres regimes na mesma corrida, e os tres desfechos sao diferentes:
        # sem nada, ACONTECE; com `versao`, o servidor recusa; com transacao, a
        # trava de linha faz o segundo esperar o LOCK TIMEOUT e desistir.
        a.ok({"op": "atualizar", "database": DB, "tabela": "c", "rowid": 1, "linha": {"id": 1, "v": 10}})
        va = a.ok({"op": "ler", "database": DB, "tabela": "c", "rowid": 1})["v"]
        vb = b.ok({"op": "ler", "database": DB, "tabela": "c", "rowid": 1})["v"]
        a.ok({"op": "atualizar", "database": DB, "tabela": "c", "rowid": 1,
              "linha": {"id": 1, "v": va + 1}})
        b.ok({"op": "atualizar", "database": DB, "tabela": "c", "rowid": 1,
              "linha": {"id": 1, "v": vb + 1}})
        final_solto = a.ok({"op": "ler", "database": DB, "tabela": "c", "rowid": 1})["v"]

        # A versao NAO vem na linha crua -- e preciso pedir `com_versao`, e o
        # teste `a_versao_vazou_na_linha_crua` do servidor cobra isso. Uma
        # primeira versao desta prova lia sem pedir, mandava `versao` ausente, e
        # a segunda gravacao passava: o controle dizia ACEITOU e eu quase
        # publiquei «o otimista nao protege».
        r_ver = a.ok({"op": "ler", "database": DB, "tabela": "c", "rowid": 1,
                      "com_versao": True})
        versao = r_ver["versao"]
        a.ok({"op": "atualizar", "database": DB, "tabela": "c", "rowid": 1,
              "linha": {"id": 1, "v": 100}, "versao": versao})
        r_obsoleta = b.fala({"op": "atualizar", "database": DB, "tabela": "c", "rowid": 1,
                             "linha": {"id": 1, "v": 200}, "versao": versao})

        a.ok({"op": "begin", "database": DB})
        a.ok({"op": "atualizar", "database": DB, "tabela": "c", "rowid": 1, "linha": {"id": 1, "v": 300}})
        r_travada = b.fala({"op": "atualizar", "database": DB, "tabela": "c", "rowid": 1,
                            "linha": {"id": 1, "v": 400}})
        r_outra_linha = b.fala({"op": "atualizar", "database": DB, "tabela": "c", "rowid": 2,
                                "linha": {"id": 2, "v": 5}})
        a.ok({"op": "rollback"})
        bruto["perda_de_atualizacao"] = {
            "sem_nada": {"lido_por_dois": [va, vb], "final": final_solto,
                         "esperado_se_nao_houvesse_perda": va + 2},
            "com_versao": nome_do_erro(r_obsoleta),
            "com_trava": nome_do_erro(r_travada),
            "outra_linha": r_outra_linha.get("ok")}
        afirmar("I", "perda_de_atualizacao_entre_escritas_soltas",
                "duas escritas SEM `versao` e SEM transacao: as duas leram %d e "
                "somaram 1, e o final e %d em vez de %d" % (va, final_solto, va + 2),
                True, final_solto == va + 1,
                controle="a MESMA sequencia com `versao` e recusada (%s), e com "
                         "transacao a segunda espera o LOCK TIMEOUT e desiste (%s)"
                         % (nome_do_erro(r_obsoleta), nome_do_erro(r_travada)))
        afirmar("I", "trava_de_linha_nao_pega_a_linha_ao_lado",
                "o controle da trava: a linha 2 continua gravavel enquanto a "
                "transacao segura a linha 1",
                True, bool(r_outra_linha.get("ok")))

        # ---- I5: skew de escrita -- ACONTECE ---------------------------------
        # O fenomeno que separa REPEATABLE READ de SERIALIZABLE. Duas
        # transacoes leem o mesmo conjunto, cada uma escreve numa LINHA
        # DIFERENTE, e nenhuma trava conflita -- o invariante «pelo menos um de
        # plantao» morre sem nenhuma das duas ter feito nada de errado sozinha.
        tabela_simples(a, "plantao")
        for i in (1, 2):
            a.ok({"op": "inserir", "database": DB, "tabela": "plantao", "linha": {"id": i, "v": 1}})
        a.ok({"op": "begin", "database": DB})
        b.ok({"op": "begin", "database": DB})
        de_plantao_a = sum(1 for l in linhas(a, "plantao") if l["v"] == 1)
        de_plantao_b = sum(1 for l in linhas(b, "plantao") if l["v"] == 1)
        a.ok({"op": "atualizar", "database": DB, "tabela": "plantao", "rowid": 1, "linha": {"id": 1, "v": 0}})
        b.ok({"op": "atualizar", "database": DB, "tabela": "plantao", "rowid": 2, "linha": {"id": 2, "v": 0}})
        a.ok({"op": "commit"})
        b.ok({"op": "commit"})
        sobraram = sum(1 for l in linhas(a, "plantao") if l["v"] == 1)
        bruto["skew_de_escrita"] = {"viu_a": de_plantao_a, "viu_b": de_plantao_b,
                                    "de_plantao_no_fim": sobraram}
        afirmar("I", "skew_de_escrita",
                "as duas viram %d de plantao, cada uma tirou a sua, e no fim "
                "sobraram %d" % (de_plantao_a, sobraram),
                0, sobraram,
                controle="as duas transacoes CONFIRMARAM (nenhuma foi recusada) "
                         "-- as travas sao por linha e as linhas eram diferentes")

        # ---- I6: a cascata nao entra no read-your-own-writes ------------------
        # Imprecisao nomeada, e ela e do I: a sobreposicao cobre o conjunto de
        # escrita, e a cascata NAO vira `Escrita` -- ela so acontece na passada
        # do COMMIT.
        par_mae_filha(a, mae="mm", filha="ff")
        a.ok({"op": "inserir", "database": DB, "tabela": "mm", "linha": {"id": 1, "n": "m"}})
        a.ok({"op": "inserir", "database": DB, "tabela": "ff", "linha": {"id": 1, "mid": 1}})
        a.ok({"op": "begin", "database": DB})
        a.ok({"op": "atualizar", "database": DB, "tabela": "mm", "rowid": 1, "linha": {"id": 42, "n": "m"}})
        mae_dentro = a.ok({"op": "ler", "database": DB, "tabela": "mm", "rowid": 1})["id"]
        filha_dentro = a.ok({"op": "ler", "database": DB, "tabela": "ff", "rowid": 1})["mid"]
        a.ok({"op": "commit"})
        filha_depois = a.ok({"op": "ler", "database": DB, "tabela": "ff", "rowid": 1})["mid"]
        bruto["cascata_no_ryow"] = {"mae_dentro": mae_dentro, "filha_dentro": filha_dentro,
                                    "filha_depois_do_commit": filha_depois}
        afirmar("I", "cascata_fora_do_read_your_own_writes",
                "dentro da transacao a MAE ja aparece com a chave nova e a "
                "FILHA ainda aponta para a antiga; o COMMIT acerta as duas",
                [42, 1, 42], [mae_dentro, filha_dentro, filha_depois],
                controle="a mae muda dentro (42) e a filha nao (1) -- se a "
                         "sobreposicao estivesse desligada, a mae tambem nao mudaria")

        # ---- I7: a matriz da leitura consistente ------------------------------
        # Duas rodadas do MESMO transferidor, so mudando se ele usa transacao:
        # 100 saem de X e entram em Y, e a soma tem de ser sempre 100. O leitor
        # pergunta de duas formas, UMA instrucao (`varrer`, que devolve as duas
        # linhas) e DUAS (`ler` + `ler`).
        #
        # As quatro celulas se controlam entre si, e por isso nao ha aqui um
        # «nao aconteceu» apoiado em nada: a celula «duas instrucoes, escritor
        # SEM transacao» e a que TEM de quebrar. Se ela nao quebrar, o medidor
        # esta cego e nenhuma das outras tres vale.
        #
        # A primeira versao deste teste tinha so o escritor COM transacao, e as
        # duas celulas deram zero -- o que parecia um resultado bonito e era um
        # medidor sem controle. O motivo esta na propria resposta: o COMMIT
        # aplica as duas alteracoes numa tomada so da trava, entao nem o par de
        # leituras separadas alcanca o meio dele.
        tabela_simples(a, "conta")
        for i, val in ((1, 50), (2, 50)):
            a.ok({"op": "inserir", "database": DB, "tabela": "conta", "linha": {"id": i, "v": val}})
        import threading

        def transferir(com_transacao, voltas, min_escritas=40):
            """Roda o escritor numa thread e o leitor aqui, e devolve quantas
            vezes cada instrumento viu a soma quebrada.

            A ARMADILHA QUE ESTA FUNCAO PAGOU, e ela e o motivo de `escritas`
            estar no resultado: na primeira versao o leitor terminava as 400
            voltas em 225 ms e o escritor levava 296 ms so para conectar e
            logar -- ele NAO rodava uma unica volta, e os dois instrumentos
            devolviam ZERO. Zero de uma corrida em que o outro lado nao existiu
            nao e «nao acontece»: e nada. Hoje o leitor ESPERA o escritor
            entrar no laco e so para quando ele ja deu `min_escritas` voltas, e
            o numero de voltas dele vai para o resultado, para que uma corrida
            vazia apareca como vazia."""
            parar = threading.Event()
            rodando = threading.Event()
            estado = {"escritas": 0, "recusas": 0, "erros": []}

            def escritor():
                w = liga()
                try:
                    sinal = 1
                    while not parar.is_set():
                        x = w.ok({"op": "ler", "database": DB, "tabela": "conta", "rowid": 1})["v"]
                        y = w.ok({"op": "ler", "database": DB, "tabela": "conta", "rowid": 2})["v"]
                        if com_transacao:
                            w.ok({"op": "begin", "database": DB})
                        r1 = w.fala({"op": "atualizar", "database": DB, "tabela": "conta",
                                     "rowid": 1, "linha": {"id": 1, "v": x - sinal}})
                        r2 = w.fala({"op": "atualizar", "database": DB, "tabela": "conta",
                                     "rowid": 2, "linha": {"id": 2, "v": y + sinal}})
                        if com_transacao:
                            w.ok({"op": "commit"})
                        if not (r1.get("ok") and r2.get("ok")):
                            estado["recusas"] += 1
                            if len(estado["erros"]) < 3:
                                estado["erros"].append(r1.get("erro") or r2.get("erro"))
                        estado["escritas"] += 1
                        rodando.set()
                        sinal = -sinal
                except (SystemExit, OSError, ValueError) as e:
                    estado["erros"].append(repr(e)[:200])
                    rodando.set()
                finally:
                    w.fechar()

            t = threading.Thread(target=escritor)
            t.start()
            rodando.wait(timeout=30)
            uma, duas, voltas_feitas = 0, 0, 0
            # A DISTRIBUICAO dos estados, e nao so a contagem de quebras: e ela
            # que separa «o leitor rasgou a leitura» de «o banco estava mesmo
            # inconsistente». Um escritor solto passa 1 ida-e-volta em cada
            # estado intermediario e 3 em cada estado em acordo, entao um leitor
            # que amostra o tempo uniformemente tem de ver 3:1:3:1. Se a
            # frequencia bate com a duracao, quem esta inconsistente e o banco.
            estados = {}
            limite = time.time() + 120
            try:
                while (voltas_feitas < voltas or estado["escritas"] < min_escritas) \
                        and time.time() < limite:
                    ls = linhas(a, "conta")
                    if len(ls) == 2:
                        chave = ",".join(str(l["v"]) for l in
                                         sorted(ls, key=lambda l: l["id"]))
                        estados[chave] = estados.get(chave, 0) + 1
                        if sum(l["v"] for l in ls) != 100:
                            uma += 1
                    x = a.ok({"op": "ler", "database": DB, "tabela": "conta", "rowid": 1})["v"]
                    y = a.ok({"op": "ler", "database": DB, "tabela": "conta", "rowid": 2})["v"]
                    if x + y != 100:
                        duas += 1
                    voltas_feitas += 1
            finally:
                parar.set()
                t.join(timeout=30)
            return {"voltas_do_leitor": voltas_feitas, "escritas": estado["escritas"],
                    "recusas": estado["recusas"], "uma_instrucao": uma,
                    "duas_instrucoes": duas, "erros_do_escritor": estado["erros"][:3],
                    "estados_vistos_por_uma_instrucao":
                        dict(sorted(estados.items(), key=lambda kv: -kv[1]))}

        VOLTAS = 400
        solto = transferir(com_transacao=False, voltas=VOLTAS)
        com_tx = transferir(com_transacao=True, voltas=VOLTAS)
        bruto["leitura_consistente"] = {"escritor_solto": solto, "escritor_em_transacao": com_tx}
        afirmar("I", "os_dois_lados_rodaram_juntos",
                "a corrida nao foi vazia: o escritor deu voltas enquanto o "
                "leitor perguntava",
                [True, True], [solto["escritas"] > 0, com_tx["escritas"] > 0],
                controle="escritor solto %d voltas / leitor %d; escritor em "
                         "transacao %d / leitor %d"
                         % (solto["escritas"], solto["voltas_do_leitor"],
                            com_tx["escritas"], com_tx["voltas_do_leitor"]))
        afirmar("I", "uma_instrucao_ve_o_banco_inconsistente_sem_transacao",
                "uma varredura unica ENXERGA o estado entre as duas escritas "
                "quando o escritor nao usa transacao",
                True, solto["uma_instrucao"] > 0,
                controle="%d de %d voltas -- e nao e defeito do leitor: o banco "
                         "esta mesmo inconsistente ali, porque o escritor "
                         "deixou as duas linhas fora de acordo"
                         % (solto["uma_instrucao"], solto["voltas_do_leitor"]))
        afirmar("I", "a_transacao_conserta_a_leitura_de_uma_instrucao",
                "com o escritor em transacao, a MESMA varredura nunca mais ve o "
                "estado intermediario",
                0, com_tx["uma_instrucao"],
                controle="o mesmo instrumento, na mesma tabela, viu %d vez(es) "
                         "contra o escritor solto -- a diferenca e a transacao, "
                         "e nada mais" % solto["uma_instrucao"])
        afirmar("I", "duas_instrucoes_nao_tem_consistencia_nenhuma",
                "duas leituras separadas veem o par inconsistente MESMO contra "
                "um escritor em transacao -- e a leitura repetivel que falta",
                True, com_tx["duas_instrucoes"] > 0,
                controle="%d de %d voltas; o COMMIT e atomico, mas ele acontece "
                         "INTEIRO entre a primeira leitura e a segunda"
                         % (com_tx["duas_instrucoes"], com_tx["voltas_do_leitor"]),
                nota="a celula «escritor solto x duas instrucoes» deu %d, e isso "
                     "NAO e garantia nenhuma: o mesmo par quebra %d vez(es) na "
                     "outra coluna, entao o instrumento enxerga. O zero ali e o "
                     "ciclo curto do escritor solto somado ao passo alternado "
                     "dos pedidos, e esta escrito para ninguem o ler como "
                     "protecao." % (solto["duas_instrucoes"], com_tx["duas_instrucoes"]))
        a.fechar()
        b.fechar()
    finally:
        dur.derrubar_limpo(p)
    return bruto


# =========================================================== D -- durabilidade

# O anexador de `strace` ja esta escrito na prova do fecho da janela; aqui so
# se reusa. A CLASSIFICACAO e propria, porque aquela filtra as oito extensoes
# de tabela e o que interessa nesta secao inclui a marca `.tx`.
import importlib.util  # noqa: E402

_spec = importlib.util.spec_from_file_location(
    "fecho", os.path.join(RAIZ, "bancada", "durabilidade", "prova-do-fecho.py"))
fecho = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(fecho)


def fsync_por_extensao(regime, acao, lote_ms=3_600_000, lote_op=1_000_000):
    """Conta os `fsync` que UMA acao provoca, por extensao de arquivo.

    Determinstico: e contagem de chamada de sistema, nao tempo. O `strace` e
    ANEXADO ao PID que ja esta de pe (nunca substitui o processo), e so depois
    de o esquema estar criado -- assim o que se conta e a acao, e nao a subida
    do servidor."""
    p, _ = subir(regime, lote_ms=lote_ms, lote_op=lote_op)
    saida = os.path.join(BASE, "strace-%s.txt" % regime)
    st = None
    try:
        c = liga()
        c.ok({"op": "criar_database", "database": DB})
        tabela_simples(c, "d")
        c.ok({"op": "inserir", "database": DB, "tabela": "d", "linha": {"id": 1, "v": 1}})
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
    for _ts, caminho, ok in fecho.eventos_fsync(saida):
        if not ok:
            continue
        base = os.path.basename(caminho)
        ext = base.rsplit(".", 1)[-1] if "." in base else "(sem)"
        por_ext[ext] = por_ext.get(ext, 0) + 1
    return por_ext


def queda_apos(regime, acao, pergunta):
    """Sobe, roda `acao`, mata com `SIGKILL` assim que ela devolve OK, reabre e
    pergunta. Nao ha corrida nenhuma aqui: o que se mede e o estado DEPOIS de
    uma resposta bem-sucedida, e nao um instante no meio dela."""
    p, _ = subir(regime)
    try:
        c = liga()
        c.ok({"op": "criar_database", "database": DB})
        tabela_simples(c, "d")
        acao(c)
        dur.matar_de_verdade(p)
        c.fechar()
    except OSError:
        pass
    p2, off = subir(regime, limpar=False)
    try:
        c2 = liga()
        r = pergunta(c2)
        rel = dur.ler_relatorio(off)
        c2.fechar()
    finally:
        dur.derrubar_limpo(p2)
    return r, rel


def letra_d():
    print("\n=== D -- DURABILIDADE ===")
    bruto = {"fsync_insercao": {}, "fsync_commit": {}, "queda": {}}

    # ---- D1: o que um INSERT comum manda ao disco, por regime ---------------
    # O controle e o proprio cruzamento: `por_operacao` mostra o `.reg` indo, e
    # e isso que prova que o cano strace -> regex -> contador ENXERGA um `.reg`
    # sincronizado quando existe um. Sem essa celula, «zero fsync no `.reg`»
    # poderia ser o instrumento surdo.
    def um_insert(c):
        c.ok({"op": "inserir", "database": DB, "tabela": "d", "linha": {"id": 2, "v": 2}})

    for regime in REGIMES:
        bruto["fsync_insercao"][regime] = fsync_por_extensao(regime, um_insert)
        print("  insert  %-13s %s" % (regime, bruto["fsync_insercao"][regime]))
    reg_op = bruto["fsync_insercao"]["por_operacao"].get("reg", 0)
    reg_lote = bruto["fsync_insercao"]["por_lote"].get("reg", 0)
    reg_sist = bruto["fsync_insercao"]["sistema"].get("reg", 0)
    afirmar("D", "por_operacao_sincroniza_o_reg",
            "em `por_operacao`, um INSERT que respondeu OK ja mandou o `.reg` "
            "ao disco",
            True, reg_op > 0,
            controle="na mesma medicao, `por_lote` da %d e `sistema` da %d -- o "
                     "contador distingue os regimes" % (reg_lote, reg_sist))
    afirmar("D", "por_lote_nao_sincroniza_no_insert",
            "em `por_lote` (o padrao) e em `sistema`, o mesmo INSERT responde "
            "OK sem nenhum `fsync` no `.reg`",
            [0, 0], [reg_lote, reg_sist],
            controle="`por_operacao` deu %d na mesma corrida" % reg_op)

    # ---- D2: a marca `.tx` e o ponto de compromisso, nos TRES regimes -------
    # `gravar_marca` chama `sync_all` INCONDICIONAL: o regime decide quando a
    # TABELA sincroniza, nunca se a transacao aconteceu.
    def um_commit(c):
        c.ok({"op": "begin", "database": DB})
        c.ok({"op": "inserir", "database": DB, "tabela": "d", "linha": {"id": 3, "v": 3}})
        c.ok({"op": "commit"})

    for regime in REGIMES:
        bruto["fsync_commit"][regime] = fsync_por_extensao(regime, um_commit)
        print("  commit  %-13s %s" % (regime, bruto["fsync_commit"][regime]))
    tx = [bruto["fsync_commit"][r].get("tx", 0) for r in REGIMES]
    afirmar("D", "marca_tx_sincroniza_nos_tres_regimes",
            "o `fsync` da marca `.tx` acontece nos tres regimes -- ele nao "
            "olha `recursos.durabilidade`",
            [True, True, True], [n > 0 for n in tx],
            controle="contados %s (%s); no MESMO commit o `.reg` sai %s -- e a "
                     "diferenca entre os dois que mostra que o regime so decide "
                     "a tabela"
                     % (tx, ", ".join(REGIMES),
                        [bruto["fsync_commit"][r].get("reg", 0) for r in REGIMES]))

    # ---- D3: uma transacao confirmada volta depois da queda ----------------
    def commit_de_tres(c):
        c.ok({"op": "begin", "database": DB})
        for i in (10, 11, 12):
            c.ok({"op": "inserir", "database": DB, "tabela": "d", "linha": {"id": i, "v": i}})
        c.ok({"op": "commit"})

    def quantas(c):
        return conta(c, "d")

    for regime in REGIMES:
        n, rel = queda_apos(regime, commit_de_tres, quantas)
        bruto["queda"][regime] = {"linhas_depois": n, "relatorio": rel}
        print("  queda apos COMMIT  %-13s linhas=%s relatorio=%s"
              % (regime, n, None if rel is None else
                 {k: v for k, v in rel.items() if k != "impossiveis_linhas"}))
    afirmar("D", "commit_sobrevive_a_queda_nos_tres_regimes",
            "matar o processo logo depois de um COMMIT que respondeu OK deixa "
            "as tres linhas la, nos tres regimes",
            [3, 3, 3], [bruto["queda"][r]["linhas_depois"] for r in REGIMES],
            controle="em `sistema` nenhum `fsync` de tabela aconteceu, e as "
                     "linhas voltaram assim mesmo -- pela marca, e nao pelo disco "
                     "da tabela")

    # ---- D4: o que o SIGKILL NAO consegue provar ---------------------------
    # Um INSERT comum em `por_lote` responde OK sem nenhum `fsync` no `.reg`
    # (D1) e MESMO ASSIM sobrevive ao `SIGKILL`. Nao e durabilidade: e o cache
    # do nucleo, que a morte do processo nao esvazia. So queda de energia
    # separaria as duas coisas, e nenhum processo em espaco de usuario provoca
    # uma. Esta celula existe para que ninguem leia a de cima como prova.
    def insert_solto(c):
        for i in (20, 21):
            c.ok({"op": "inserir", "database": DB, "tabela": "d", "linha": {"id": i, "v": i}})

    n, _rel = queda_apos("por_lote", insert_solto, quantas)
    bruto["queda"]["insert_solto_por_lote"] = n
    afirmar("D", "sigkill_nao_separa_cache_do_nucleo_de_disco",
            "duas insercoes comuns em `por_lote`, sem nenhum `fsync` no `.reg`, "
            "sobrevivem ao SIGKILL",
            2, n,
            controle="a contagem de `fsync` da mesma configuracao (D1) e ZERO "
                     "no `.reg` -- as linhas voltaram do cache do nucleo, nao do "
                     "disco. O SIGKILL nao distingue os dois; so queda de energia "
                     "distinguiria")
    return bruto


# ==================================================================== veredito

def nivel_ansi(bruto_i):
    """O nivel da norma sai dos fenomenos MEDIDOS, nunca de uma opiniao.

    ANSI SQL define os niveis pelo que cada um PROIBE:
      READ UNCOMMITTED  -- permite os tres
      READ COMMITTED    -- proibe leitura suja
      REPEATABLE READ   -- proibe tambem a leitura nao repetivel
      SERIALIZABLE      -- proibe tambem o fantasma (e, na leitura moderna, o
                           skew de escrita)
    """
    suja = bruto_i["leitura_suja"]["outra_sessao"] != 999
    nao_rep = bruto_i["leitura_nao_repetivel"]["primeira"] != bruto_i["leitura_nao_repetivel"]["segunda"]
    fant = bruto_i["fantasma"]["segunda"] > bruto_i["fantasma"]["primeira"]
    if not suja:
        return "READ UNCOMMITTED", ["leitura suja"]
    if nao_rep:
        return "READ COMMITTED", ["leitura nao repetivel", "fantasma"] if fant else ["leitura nao repetivel"]
    if fant:
        return "REPEATABLE READ", ["fantasma"]
    return "SERIALIZABLE", []


def portao_da_maquina():
    """Registrado, nao obedecido: nenhum numero desta bancada e uma duracao.
    Fica no resultado para quem for ler saber em que estado a maquina estava."""
    r = subprocess.run([os.path.join(RAIZ, "bancada", "esta-medindo.sh")],
                       capture_output=True, text=True)
    return {"havia_medicao_em_curso": r.returncode == 0,
            "quem": [l for l in r.stdout.strip().split("\n") if l][:6]}


def main():
    if not os.path.exists(fecho.PHXSQLD):
        print("falta %s -- rode `cargo build --release`" % fecho.PHXSQLD)
        return 2
    versao = subprocess.run([fecho.PHXSQLD, "--version"], capture_output=True,
                            text=True).stdout.strip()
    print("== ACID, letra por letra, contra %s ==" % versao)
    if RAPIDO:
        print("!! PHX_ACID_RAPIDO=1: as varreduras de SIGKILL foram PULADAS. "
              "Este resultado NAO se publica.")

    bruto = {"versao": versao, "rapido": RAPIDO, "maquina": portao_da_maquina()}
    bruto["A"] = letra_a()
    bruto["C"] = letra_c()
    bruto["I"] = letra_i()
    bruto["D"] = letra_d()
    nivel, acima_disso = nivel_ansi(bruto["I"])
    bruto["nivel_ansi"] = {"nivel": nivel, "fenomenos_que_acontecem": acima_disso,
                           "skew_de_escrita_acontece":
                               bruto["I"]["skew_de_escrita"]["de_plantao_no_fim"] == 0}
    bruto["afirmacoes"] = AFIRMACOES

    shutil.rmtree(BASE, ignore_errors=True)
    with open(RESULTADO, "w") as f:
        json.dump(bruto, f, indent=2, ensure_ascii=False)

    falharam = [a["chave"] for a in AFIRMACOES if not a["ok"]]
    print("\n== veredito ==")
    print("  nivel da norma, pelos fenomenos medidos: %s" % nivel)
    print("  afirmacoes: %d, das quais %d nao confirmaram"
          % (len(AFIRMACOES), len(falharam)))
    for ch in falharam:
        print("    ! %s" % ch)
    print("\nresultado medido gravado em %s" % RESULTADO)
    return 1 if falharam else 0


if __name__ == "__main__":
    sys.exit(main())
