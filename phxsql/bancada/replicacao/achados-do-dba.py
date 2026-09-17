#!/usr/bin/env python3
"""Os tres achados do papel C (DBA) de 17/09/2026, provados pelo SOQUETE.

    python3 bancada/replicacao/achados-do-dba.py [diretorio]
    python3 bancada/replicacao/achados-do-dba.py --so rownum|unico|recriada

O parecer `docs/propostas/parecer-dba-replicacao-2026-09-17.md` achou tres
defeitos LENDO o codigo. Ler o codigo nao prova defeito de replicacao: o que
depende de dois processos e de um soquete se prova contra dois processos e um
soquete. Esta bancada e a prova real de cada um, e ela e **nos dois sentidos**
por construcao -- cada estagio roda o cenario do defeito E o controle, que e o
MESMO roteiro com a unica linha do defeito retirada:

| estagio | cenario (tem de DIVERGIR/PARAR) | controle (tem de bater/andar) |
|---|---|---|
| `rownum` | 3 linhas, **1 insercao recusada**, 2 linhas | as mesmas 5 linhas, sem a recusa |
| `unico` | par bidirecional, unico secundario em **conflito** | o mesmo par, e-mails distintos |
| `recriada`| `excluir_tabela`+`criar_tabela` no source | o mesmo source, sem apagar a tabela |

Se o controle tambem falhar, a prova nao vale: o que ela mediu foi outra
coisa. E por isso que o controle roda SEMPRE, e no mesmo processo.

Portas 5910-5917, so desta bancada. Nenhum `pkill`: cada servidor morre pelo
PID que este script guardou, e o diretorio sai no fim.
"""
import hashlib
import json
import os
import shutil
import signal
import socket
import subprocess
import sys
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
PHXSQLD = os.path.join(RAIZ, "target", "release", "phxsqld")

TOKEN = "espelho"
USUARIO = "adm"
SENHA = "segredo1"
RECONECTAR_EM = 1

# Uma faixa so desta bancada, para nunca esbarrar na de replicacao (5800-5803),
# na de modos (5330-5339) nem na da trava (7050-7055).
P = {"fonte": 5910, "replica": 5911, "alfa": 5912, "beta": 5913,
     "fonte2": 5916, "replica2": 5917}

PROCESSOS = []
RESULTADO = {}


def hash_da_senha(senha):
    """O hash sai do proprio servidor -- nao ha uma segunda implementacao."""
    saida = subprocess.run([PHXSQLD, "--senha"], input=senha + "\n",
                           capture_output=True, text=True).stdout
    return saida.split('": "')[1].split('"')[0]


def permissoes():
    return {"*": {"ler": True, "inserir": True, "alterar": True,
                  "excluir": True, "criar": True, "administrar": True,
                  "diario": True, "verificar": True, "replicar": True}}


def config(porta, h, replicacao):
    return {"base": "base", "bind": f"127.0.0.1:{porta}", "token": TOKEN,
            "web": {"ligado": False}, "replicacao": replicacao,
            "usuarios": [{"login": USUARIO, "nome": "Adriano", "id": 10,
                          "senha_hash": h, "bases": permissoes()}]}


def origem(porta, nome, h):
    return {"nome": nome, "host": "127.0.0.1", "porta": porta, "token": TOKEN,
            "usuario": USUARIO, "senha_hash": h, "databases": ["loja"],
            "reconectar_em": RECONECTAR_EM}


def subir(base, nome, cfg):
    d = os.path.join(base, nome)
    os.makedirs(d, exist_ok=True)
    with open(os.path.join(d, "config.json"), "w") as f:
        json.dump(cfg, f, indent=2)
    log = open(os.path.join(d, "servidor.log"), "a")
    p = subprocess.Popen([PHXSQLD], cwd=d, stdout=log,
                         stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
    PROCESSOS.append(p)
    return p


def derrubar(*procs):
    """Mata pelo PID guardado -- nunca por nome, nunca por varredura."""
    for p in procs or list(PROCESSOS):
        if p.poll() is None:
            p.send_signal(signal.SIGTERM)
    for p in procs or list(PROCESSOS):
        try:
            p.wait(timeout=15)
        except subprocess.TimeoutExpired:
            p.kill()
        if p in PROCESSOS:
            PROCESSOS.remove(p)


def liga(porta, segundos=25):
    """Conexao com prazo LONGO de leitura.

    Prazo de cliente menor que o defeito mede «nao respondeu» em vez de
    «esperou» -- a armadilha que o `trava.py` ja pagou.
    """
    fim = time.monotonic() + segundos
    erro = None
    while time.monotonic() < fim:
        try:
            s = socket.create_connection(("127.0.0.1", porta), timeout=120)
            s.settimeout(120)
            f = s.makefile("rwb")

            def fala(p, _f=f):
                p.setdefault("token", TOKEN)
                _f.write((json.dumps(p) + "\n").encode())
                _f.flush()
                return json.loads(_f.readline().decode())

            r = fala({"op": "login", "usuario": USUARIO, "senha": SENHA})
            if not r.get("ok"):
                raise SystemExit(f"login na porta {porta}: {r}")
            return fala
        except OSError as e:          # servidor ainda subindo
            erro = e
            time.sleep(0.3)
    raise SystemExit(f"nao consegui falar com 127.0.0.1:{porta}: {erro}")


def criar_tabela(fala, email=False):
    colunas = [{"nome": "id", "tipo": "Int4", "obrigatoria": True},
               {"nome": "nome", "tipo": "Str(40)", "obrigatoria": True}]
    indices = [{"nome": "porId", "colunas": ["id"], "unico": True,
                "primario": True}]
    if email:
        colunas.append({"nome": "email", "tipo": "Str(60)"})
        indices.append({"nome": "porEmail", "colunas": ["email"],
                        "unico": True})
    fala({"op": "criar_database", "database": "loja"})
    return fala({"op": "criar_tabela", "database": "loja", "tabela": "clientes",
                 "motivo_obrigatorio": False, "colunas": colunas,
                 "indices": indices})


def linhas_de(fala, com_email=False):
    """Todas as linhas pelo cursor, com as colunas de SISTEMA junto.

    O `rownum` e uma delas, e e o que este estagio mede: contar linhas nao
    acha divergencia nenhuma.
    """
    saida, depois = [], 0
    while True:
        d = fala({"op": "varrer", "database": "loja", "tabela": "clientes",
                  "max": 2000, "depois": depois, "visao": "todas"})
        if not d.get("ok", True):
            return saida
        d = d["resultado"]
        saida.extend(d["linhas"])
        if not d["ha_mais"] or not d["linhas"]:
            return saida
        depois = d["cursor_fim"]


def retrato(fala):
    """O MESMO retrato da bancada: SHA-256 da linha inteira, pelo cursor."""
    h = hashlib.sha256()
    n = 0
    for l in linhas_de(fala):
        h.update(json.dumps(l, sort_keys=True, ensure_ascii=False).encode())
        n += 1
    return n, h.hexdigest()[:16]


def eventos(fala):
    """Quantos eventos o diario DESTE servidor tem para `loja.clientes`.

    O campo mora em `resultado.tabelas.clientes.eventos`; pedi-lo com
    `"tabela"` no pedido devolve `None` calado -- foi assim que a primeira
    versao desta bancada esperou por uma condicao que nunca poderia ser
    verdadeira e seguiu em frente como se tivesse esperado.
    """
    r = fala({"op": "posicao", "database": "loja"})
    if not r.get("ok"):
        return None
    return r["resultado"]["tabelas"].get("clientes", {}).get("eventos")


def esperar(cond, segundos, passo=0.25):
    fim = time.monotonic() + segundos
    while time.monotonic() < fim:
        v = cond()
        if v:
            return v
        time.sleep(passo)
    return None


def cabecalho(nome, esperado):
    print(f"\n--- estagio ({nome})")
    print(f"    esperado: {esperado}")


def julgar(nome, ok, medido, extra=None):
    print(f"    medido:   {medido}  [{'ok' if ok else 'FALHA'}]")
    RESULTADO[nome] = {"ok": ok, "medido": medido, **(extra or {})}


# ----------------------------------------------------------- (1) o `rownum`


def par_classico(base, sufixo, h, pf, pr, email=False, somente_leitura=True):
    subir(base, f"fonte{sufixo}", config(pf, h, {
        "papel": "source", "id_servidor": f"fonte{sufixo}",
        "imagem_da_linha": True}))
    cfg_r = config(pr, h, {
        "papel": "replica", "id_servidor": f"replica{sufixo}",
        "imagem_da_linha": True,
        "origens": [origem(pf, f"fonte{sufixo}", h)]})
    cfg_r["somente_leitura"] = somente_leitura
    subir(base, f"replica{sufixo}", cfg_r)
    f, r = liga(pf), liga(pr)
    criar_tabela(f, email=email)
    return f, r


# As tres recusas que este estagio experimenta, e por que TRES.
#
# A primeira corrida usou so' a chave duplicada na PRIMARIA e nao reproduziu a
# divergencia: o contador do source nao foi queimado. Hipotese morta gera a
# proxima -- se o contador anda antes da conferencia de unicidade, entao o que
# importa e' QUAL conferencia recusa, porque elas nao estao todas no mesmo
# ponto do caminho. Cada linha desta tabela e' uma recusa em um ponto
# diferente, e a coluna medida e' uma so': o source queimou um `rownum`?
RECUSAS = {
    # unicidade da chave PRIMARIA
    "primaria": lambda i: {"id": i - 1, "nome": "repetida"},
    # unicidade de um indice unico SECUNDARIO
    "secundaria": lambda i: {"id": 900 + i, "nome": "repetida",
                             "email": "cliente1@x"},
    # coluna obrigatoria faltando (regra de esquema, nao de indice)
    "obrigatoria": lambda i: {"id": 900 + i},
}


def semear(fala, ids, recusa_em=None, tipo="primaria", email=False):
    """Insere `ids`, e -- se pedido -- tenta UMA linha que tem de ser recusada.

    A tentativa recusada e' a unica diferenca entre o cenario e o controle. Se
    o servidor a ACEITAR, a prova nao mediu o que promete, e o estagio diz
    isso em vez de seguir.
    """
    recusada = None
    for i in ids:
        if recusa_em is not None and i == recusa_em:
            recusada = fala({"op": "inserir", "database": "loja",
                             "tabela": "clientes",
                             "linha": RECUSAS[tipo](i)})
        linha = {"id": i, "nome": f"Cliente {i}"}
        if email:
            linha["email"] = f"cliente{i}@x"
        fala({"op": "inserir", "database": "loja", "tabela": "clientes",
              "linha": linha})
    return recusada


def compara_rownum(f, r):
    lf = {l["rowid"]: l for l in linhas_de(f)}
    lr = {l["rowid"]: l for l in linhas_de(r)}
    difs = [(k, lf[k].get("rownum"), lr[k].get("rownum"))
            for k in sorted(lf) if k in lr
            and lf[k].get("rownum") != lr[k].get("rownum")]
    return lf, lr, difs


def uma_corrida_de_rownum(base, sufixo, h, deslocamento, recusa=None):
    """Sobe um par, semeia 5 linhas (com ou sem uma recusa) e compara."""
    f, r = par_classico(base, sufixo, h, P["fonte"] + deslocamento,
                        P["replica"] + deslocamento, email=True)
    recusada = semear(f, [1, 2, 3, 4, 5], recusa_em=(4 if recusa else None),
                      tipo=(recusa or "primaria"), email=True)
    esperar(lambda: eventos(r) == 5, 30)
    lf, lr, difs = compara_rownum(f, r)
    ret = (retrato(f), retrato(r))
    derrubar()
    return {"recusa": recusa, "resposta": recusada, "lf": lf, "lr": lr,
            "difs": difs, "retratos": ret}


def corrida_de_rownum_em_lote(base, h, deslocamento):
    """A recusa DENTRO de um lote, com `parar_no_erro: false`.

    As tres recusas de cima sao operacoes separadas, e entre duas operacoes o
    servidor reabre a tabela -- o contador em memoria (`Reg::proximo_rownum`)
    nasce de novo do disco, e a queima se perde. Dentro de UM lote a tabela e'
    a mesma instancia do comeco ao fim: se a queima existe, e' aqui que ela
    sobrevive ate a linha seguinte.
    """
    f, r = par_classico(base, "-lote", h, P["fonte"] + deslocamento,
                        P["replica"] + deslocamento, email=True)
    linhas = [{"id": i, "nome": f"Cliente {i}", "email": f"cliente{i}@x"}
              for i in (1, 2, 3)]
    linhas.append({"id": 3, "nome": "repetida", "email": "repetida@x"})
    linhas += [{"id": i, "nome": f"Cliente {i}", "email": f"cliente{i}@x"}
               for i in (4, 5)]
    resp = f({"op": "inserir_lote", "database": "loja", "tabela": "clientes",
              "linhas": linhas, "parar_no_erro": False})
    esperar(lambda: eventos(r) == 5, 30)
    lf, lr, difs = compara_rownum(f, r)
    ret = (retrato(f), retrato(r))
    derrubar()
    return {"resposta": resp, "lf": lf, "lr": lr, "difs": difs,
            "retratos": ret}


def estagio_rownum(base, h):
    cabecalho("rownum",
              "uma insercao RECUSADA no source queima um rownum e nao gera "
              "evento: os rowids continuam iguais e o rownum DIVERGE -- e o "
              "retrato SHA-256, que inclui o rownum, acusa. O controle (as "
              "mesmas 5 linhas sem a recusa) tem de bater nos dois. Sao TRES "
              "recusas, em tres pontos diferentes do caminho de gravacao: "
              "unicidade da primaria, unicidade de um secundario, e coluna "
              "obrigatoria faltando")

    ctl = uma_corrida_de_rownum(base, "-ctl", h, 0)
    print(f"    controle: retrato source={ctl['retratos'][0]} "
          f"replica={ctl['retratos'][1]}; rownum diferentes: "
          f"{len(ctl['difs'])} de {len(ctl['lf'])}")

    corridas, desloc = {}, 20
    for tipo in RECUSAS:
        c = uma_corrida_de_rownum(base, f"-{tipo}", h, desloc, recusa=tipo)
        desloc += 20
        corridas[tipo] = c
        resp = c["resposta"] or {}
        recusou = not resp.get("ok", True)
        print(f"    recusa por {tipo:12} -> recusada={recusou} "
              f"('{str(resp.get('erro', resp))[:70]}')")
        print("      rowid |  id | rownum source | rownum replica")
        for k in sorted(c["lf"]):
            a = c["lf"][k].get("rownum")
            b = c["lr"].get(k, {}).get("rownum")
            print(f"      {k:5} | {c['lf'][k]['id']:3} | {a:13} | {b:14}"
                  + ("  <<< DIVERGIU" if a != b else ""))

    # A quarta: a recusa DENTRO de um lote, onde a tabela nao e' reaberta.
    c = corrida_de_rownum_em_lote(base, h, desloc)
    corridas["lote"] = {"resposta": c["resposta"], "lf": c["lf"],
                        "lr": c["lr"], "difs": c["difs"],
                        "retratos": c["retratos"]}
    res = c["resposta"].get("resultado", {})
    print(f"    recusa por {'lote':12} -> gravadas={res.get('gravadas')} "
          f"recusadas={res.get('recusadas', res.get('erros'))}")
    print("      rowid |  id | rownum source | rownum replica")
    for k in sorted(c["lf"]):
        a = c["lf"][k].get("rownum")
        b = c["lr"].get(k, {}).get("rownum")
        print(f"      {k:5} | {c['lf'][k]['id']:3} | {a:13} | {b:14}"
              + ("  <<< DIVERGIU" if a != b else ""))

    # O veredito: ALGUMA das recusas queimou o contador do source?
    queimaram = [t for t, c in corridas.items() if c["difs"]]
    # O lote NAO entra nesta conta: ele responde `ok` e recusa uma linha por
    # dentro -- quem diz que a recusa aconteceu ali e' o `gravadas` < enviadas.
    todas_recusadas = all(not (corridas[t]["resposta"] or {}).get("ok", True)
                          for t in RECUSAS)
    controle_limpo = not ctl["difs"] and ctl["retratos"][0] == ctl["retratos"][1]
    ok = bool(queimaram) and todas_recusadas and controle_limpo
    julgar("rownum", ok,
           f"controle SEM recusa: retrato igual="
           f"{ctl['retratos'][0] == ctl['retratos'][1]}, rownum "
           f"diferentes={len(ctl['difs'])}; as tres recusas aconteceram="
           f"{todas_recusadas}; QUEIMARAM o contador do source: "
           f"{queimaram or 'NENHUMA'}; rownum diferentes por tipo: "
           + ", ".join(f"{t}={len(c['difs'])}" for t, c in corridas.items()),
           {"controle": {"retratos": ctl["retratos"],
                         "rownum_diferentes": len(ctl["difs"])},
            "por_recusa": {t: {"erro": (c["resposta"] or {}).get("erro"),
                               "recusada": not (c["resposta"] or {}).get("ok", True),
                               "rownum_diferentes": c["difs"],
                               "retratos": c["retratos"]}
                           for t, c in corridas.items()},
            "queimaram_o_contador": queimaram})


# -------------------------------------------- (2) o unico secundario no par


def par_bidi(base, sufixo, h, pa, pb):
    subir(base, f"alfa{sufixo}", config(pa, h, {
        "papel": "multi", "id_servidor": f"alfa{sufixo}",
        "imagem_da_linha": True,
        "origens": [origem(pb, f"beta{sufixo}", h)]}))
    subir(base, f"beta{sufixo}", config(pb, h, {
        "papel": "multi", "id_servidor": f"beta{sufixo}",
        "imagem_da_linha": True,
        "origens": [origem(pa, f"alfa{sufixo}", h)]}))
    a, b = liga(pa), liga(pb)
    criar_tabela(a, email=True)
    criar_tabela(b, email=True)
    return a, b


def ids_em(fala):
    return sorted(l["id"] for l in linhas_de(fala))


def estagio_unico(base, h):
    cabecalho("unico",
              "num par bidirecional com um UNICO SECUNDARIO, uma linha que "
              "conflita nesse indice nao e uma linha perdida: a posicao nao "
              "anda e o par PARA -- a escrita SEGUINTE, que nao conflita com "
              "nada, tambem nao atravessa. O controle (mesmos ids, e-mails "
              "distintos) tem de atravessar os dois")

    # -------- controle: e-mails distintos
    a, b = par_bidi(base, "-ctl", h, P["alfa"], P["beta"])
    b({"op": "inserir", "database": "loja", "tabela": "clientes",
       "linha": {"id": 2, "nome": "de beta", "email": "beta@x"}})
    a({"op": "inserir", "database": "loja", "tabela": "clientes",
       "linha": {"id": 1, "nome": "de alfa", "email": "alfa@x"}})
    chegou1_ctl = esperar(lambda: 1 in ids_em(b), 20) is not None
    a({"op": "inserir", "database": "loja", "tabela": "clientes",
       "linha": {"id": 3, "nome": "depois", "email": "tres@x"}})
    chegou3_ctl = esperar(lambda: 3 in ids_em(b), 20) is not None
    ids_ctl = ids_em(b)
    derrubar()
    print(f"    controle: em beta {ids_ctl} -- a linha de alfa chegou="
          f"{chegou1_ctl}, a SEGUINTE chegou={chegou3_ctl}")

    # -------- cenario: o mesmo e-mail dos dois lados
    a, b = par_bidi(base, "-def", h, P["alfa"] + 20, P["beta"] + 20)
    b({"op": "inserir", "database": "loja", "tabela": "clientes",
       "linha": {"id": 2, "nome": "de beta", "email": "mesmo@x"}})
    a({"op": "inserir", "database": "loja", "tabela": "clientes",
       "linha": {"id": 1, "nome": "de alfa", "email": "mesmo@x"}})
    chegou1 = esperar(lambda: 1 in ids_em(b), 20) is not None
    # A SEGUNDA escrita e a que separa «uma linha perdida» de «o par parado».
    a({"op": "inserir", "database": "loja", "tabela": "clientes",
       "linha": {"id": 3, "nome": "depois", "email": "tres@x"}})
    chegou3 = esperar(lambda: 3 in ids_em(b), 20) is not None
    ids = ids_em(b)
    estado = b({"op": "replicacao_estado"}).get("resultado")
    log = os.path.join(base, "beta-def", "servidor.log")
    erro = ""
    if os.path.exists(log):
        linhas = [x for x in open(log, errors="replace").read().splitlines()
                  if "duplicada" in x or "porEmail" in x]
        erro = linhas[-1][-160:] if linhas else ""
    derrubar()

    ok = (chegou1_ctl and chegou3_ctl and not chegou1 and not chegou3)
    julgar("unico", ok,
           f"controle (e-mails distintos): em beta {ids_ctl}, a seguinte "
           f"chegou={chegou3_ctl}; CENARIO (mesmo e-mail): em beta {ids}, a "
           f"linha do conflito chegou={chegou1} e a SEGUINTE, que nao "
           f"conflita com nada, chegou={chegou3} -- o par parou. log: "
           f"'{erro}'",
           {"controle_ids": ids_ctl, "cenario_ids": ids,
            "chegou_conflitante": chegou1, "chegou_seguinte": chegou3,
            "estado_da_replica": estado, "log": erro})


# ------------------------------------------ (3) tabela apagada e recriada


def estagio_recriada(base, h):
    cabecalho("recriada",
              "`excluir_tabela`+`criar_tabela` no source zera o diario dele; "
              "a replica, com posicao MAIOR, devolve Ok(0) calada para "
              "sempre: fica com a tabela velha, sem erro em lugar nenhum. O "
              "controle (sem apagar) tem de receber as linhas novas")

    # -------- controle: sem apagar a tabela
    f, r = par_classico(base, "-r-ctl", h, P["fonte2"], P["replica2"])
    semear(f, [1, 2, 3, 4, 5])
    esperar(lambda: eventos(r) == 5, 30)
    semear(f, [6, 7, 8])
    chegou_ctl = esperar(lambda: len(linhas_de(r)) == 8, 25) is not None
    ids_ctl = ids_em(r)
    derrubar()
    print(f"    controle: na replica {ids_ctl} -- as 3 novas chegaram="
          f"{chegou_ctl}")

    # -------- cenario: apaga e recria no source
    f, r = par_classico(base, "-r-def", h, P["fonte2"] + 20, P["replica2"] + 20)
    semear(f, [1, 2, 3, 4, 5])
    esperar(lambda: eventos(r) == 5, 30)
    antes = ids_em(r)
    # `excluir_tabela` EXIGE `confirmar` com o nome da tabela. Sem ele a ordem
    # e' recusada -- e a primeira versao desta bancada nao conferia a resposta:
    # a tabela continuava de pe, as linhas novas chegavam, e o estagio
    # concluia «o defeito nao existe». Conferir o ESTRAGO antes de julgar o
    # veredito e' a mesma licao que a casa ja pagou uma vez.
    apagou = f({"op": "excluir_tabela", "database": "loja",
                "tabela": "clientes", "confirmar": "clientes"})
    recriou = criar_tabela(f)
    if not apagou.get("ok") or not recriou.get("ok"):
        julgar("recriada", False,
               f"a MONTAGEM falhou, entao nada foi medido: "
               f"excluir_tabela={apagou}, criar_tabela={recriou}")
        derrubar()
        return
    semear(f, [91, 92, 93])
    chegou = esperar(lambda: 91 in ids_em(r), 25) is not None
    ids_r, ids_f = ids_em(r), ids_em(f)
    ev_f, ev_r = eventos(f), eventos(r)
    estado = r({"op": "replicacao_estado"}).get("resultado")
    # A testemunha do silencio: a replica acusa alguma coisa?
    parada = (estado or {}).get("origens", [{}])
    parada = parada[0] if isinstance(parada, list) and parada else {}
    derrubar()

    ok = (chegou_ctl and not chegou and ids_r == antes
          and ids_f == [91, 92, 93])
    julgar("recriada", ok,
           f"controle (sem apagar): na replica {ids_ctl}, as novas "
           f"chegaram={chegou_ctl}; CENARIO: source apagou e recriou, tem "
           f"{ids_f} com {ev_f} evento(s); a replica ficou com {ids_r} "
           f"({ev_r} evento(s)), as linhas novas chegaram={chegou}, "
           f"ultimo_erro={parada.get('ultimo_erro')!r} "
           f"parada={parada.get('parada')!r}",
           {"controle_ids": ids_ctl, "replica_ids": ids_r, "source_ids": ids_f,
            "eventos_source": ev_f, "eventos_replica": ev_r,
            "chegou": chegou, "estado_da_replica": estado})


def main():
    if not os.path.exists(PHXSQLD):
        sys.exit(f"nao achei {PHXSQLD} -- rode `cargo build --release` antes")
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    base = args[0] if args else f"/tmp/phx-achados-dba-{os.getpid()}"
    so = None
    if "--so" in sys.argv:
        so = sys.argv[sys.argv.index("--so") + 1]
    os.makedirs(base, exist_ok=True)
    h = hash_da_senha(SENHA)
    print(f"binario {PHXSQLD}")
    print(f"pasta   {base}")
    t0 = time.perf_counter()
    try:
        if so in (None, "rownum"):
            estagio_rownum(base, h)
        if so in (None, "unico"):
            estagio_unico(base, h)
        if so in (None, "recriada"):
            estagio_recriada(base, h)
    finally:
        derrubar()
    RESULTADO["minutos"] = round((time.perf_counter() - t0) / 60, 1)
    RESULTADO["medido_em"] = time.strftime("%Y-%m-%d %H:%M", time.gmtime()) + " UTC"
    RESULTADO["binario"] = os.path.relpath(PHXSQLD, RAIZ)
    falhas = [k for k, v in RESULTADO.items()
              if isinstance(v, dict) and not v.get("ok")]
    RESULTADO["confirmados"] = [k for k, v in RESULTADO.items()
                                if isinstance(v, dict) and v.get("ok")]
    print("\nRESULTADO " + json.dumps(RESULTADO, ensure_ascii=False))
    # MESCLA por nome, e nao sobrescreve: rodar `--so rownum` apagava os
    # outros dois estagios e deixava um arquivo que parecia a corrida inteira.
    # E' o mesmo defeito que o `medir.py` ja pagou, e a cura e' a mesma --
    # chave preservada e' chave NOMEADA, e o arquivo diz o que veio de onde.
    alvo = os.path.join(AQUI, "achados-do-dba.json")
    saida = {}
    if os.path.exists(alvo):
        try:
            saida = json.load(open(alvo, encoding="utf-8"))
        except ValueError:
            saida = {}
    preservados = [k for k in ("rownum", "unico", "recriada")
                   if k in saida and k not in RESULTADO]
    saida.update(RESULTADO)
    if preservados:
        saida["preservados_de_corrida_anterior"] = preservados
    elif "preservados_de_corrida_anterior" in saida:
        del saida["preservados_de_corrida_anterior"]
    with open(alvo, "w", encoding="utf-8") as g:
        json.dump(saida, g, ensure_ascii=False, indent=1)
    print(f"gravado em {alvo}"
          + (f" (preservados de corrida anterior: {preservados})"
             if preservados else ""))
    if not args:
        shutil.rmtree(base, ignore_errors=True)
    print("\nconfirmados (o defeito aconteceu E o controle passou): "
          + ", ".join(RESULTADO["confirmados"] or ["nenhum"]))
    if falhas:
        print("NAO confirmados: " + ", ".join(falhas))


if __name__ == "__main__":
    main()
