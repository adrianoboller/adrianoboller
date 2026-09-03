#!/usr/bin/env python3
"""A matriz real de durabilidade -- SP000010, ponto de morte x regime.

    cargo build --release
    python3 bancada/durabilidade/prova.py

# O que este script prova, e por que ele mata processos de verdade

`docs/TRANSACOES.md` SS5 desenha o protocolo: a marca `transacao_<id>.tx` e o
PONTO DE COMPROMISSO (antes do `fsync` dela a transacao nao aconteceu; depois,
aconteceu, mesmo sem nenhum byte nas tabelas), e a recuperacao anda para a
frente e completa o que faltar. O que faltava era a MATRIZ: cruzar os cinco
pontos de morte do sprint com os tres regimes de `recursos.durabilidade`, e
provar cada celula com um `SIGKILL` de verdade -- nao com um teste unitario
que fecha uma `Table` e chama isso de queda.

A licao do proprio SS5.6 e o metodo inteiro deste script: matar o processo no
instante certo e uma CORRIDA, e os dois desfechos (ABORTED / COMMITTED) sao
legitimos. A pergunta nunca e "as N linhas estao la?" -- e "o relatorio do
arranque consegue dizer, sem ambiguidade, qual das duas aconteceu?". Por isso
cada ponto abaixo roda VARIAS vezes com o atraso do SIGKILL variando (uma
varredura, nao uma tentativa so), e o que se mede e a DISTRIBUICAO dos
desfechos -- nunca um numero digitado.

# O achado central, medido nesta rodada (nao suposto lendo o codigo)

A marca `.tx` e sincronizada de forma INCONDICIONAL em `gravar_marca` -- os
tres regimes de `recursos.durabilidade` NAO entram nessa chamada. O que o
regime decide e so o `fsync` da TABELA, depois da passada. E como a queda de
PROCESSO (ao contrario da queda de ENERGIA) nunca perde um `write` que ja foi
entregue ao sistema operacional -- e a mesma lei que `bancada/exclusao/
prova-da-queda.py` ja mediu para a exclusao --, os quatro primeiros pontos de
morte deste script tem a MESMA garantia nos tres regimes. Isso e testado
diretamente: a varredura roda para os tres e a distribuicao dos desfechos e
comparada.

Onde o regime REALMENTE muda o que se ve e outro eixo, tambem medido aqui:
quanto tempo a marca FICA no disco depois de um commit que NAO caiu. Ver
`marcador_por_regime()`.

# O que fica de fora, e por que

A queda de ENERGIA (nao de processo) e o unico jeito de ver um `.tx`
REALMENTE truncado (CRC que nao confere por escrita cortada no meio). Nenhum
processo em espaco de usuario provoca isso -- e a mesma linha que
`docs/DESEMPENHO.md` SS4.12 ja escreveu para a exclusao. O que se testa aqui em
vez disso, honestamente rotulado, e a fronteira que o SIGKILL alcanca: o
arquivo nunca chega a existir (antes do primeiro `write`).

Mata so os PIDs que ele mesmo subiu. Nunca `pkill -f`.
"""
import json
import os
import re
import shutil
import signal
import socket
import subprocess
import sys
import threading
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.environ.get("PHX_RAIZ", os.path.abspath(os.path.join(AQUI, "..", "..")))
sys.path.insert(0, os.path.join(RAIZ, "bancada", "profiler"))
from comum import PHXSQLD, TOKEN, hash_da_senha  # noqa: E402

PORTA = 7530
BASE = os.path.join(RAIZ, "bancada", "durabilidade", ".base-da-prova")
DB = "durab"
REGIMES = ["por_operacao", "por_lote", "sistema"]
RESULTADO = os.path.join(AQUI, "resultado.json")


# ----------------------------------------------------------------- subir/matar

def config(regime, lote_ms=3_600_000, lote_op=1_000_000):
    return {
        "base": "base", "bind": "127.0.0.1:%d" % PORTA, "token": TOKEN,
        # O padrao (1.000) truncava o "buscar" da cascata em silencio -- 1200
        # filhas voltavam como 1000 e a verificacao de consistencia acusava
        # "parcial" num caso que so estava CORTADO pela paginacao. Medido
        # neste mesmo script antes deste ajuste.
        "max_linhas": 10_000,
        "web": {"ligado": False},
        "recursos": {
            "durabilidade": regime,
            "lote_operacoes": lote_op,
            "lote_milissegundos": lote_ms,
        },
        "usuarios": [
            {"login": "adm", "nome": "Adriano", "id": 10, "nivel": "admin",
             "senha_hash": hash_da_senha("senha-do-adm"),
             "bases": {"*": {"ler": True, "inserir": True, "alterar": True,
                             "excluir": True, "criar": True,
                             "administrar": True, "verificar": True,
                             "reindexar": True}}},
        ],
    }


def subir(cfg, limpar):
    if limpar:
        shutil.rmtree(BASE, ignore_errors=True)
    os.makedirs(BASE, exist_ok=True)
    with open(os.path.join(BASE, "config.json"), "w") as f:
        json.dump(cfg, f, indent=2)
    log = os.path.join(BASE, "servidor.log")
    off = os.path.getsize(log) if os.path.exists(log) else 0
    saida = open(log, "a")
    p = subprocess.Popen([PHXSQLD], cwd=BASE, stdout=saida,
                         stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
    for _ in range(120):
        time.sleep(0.1)
        try:
            socket.create_connection(("127.0.0.1", PORTA), 0.3).close()
            return p, off
        except OSError:
            if p.poll() is not None:
                raise SystemExit("o servidor morreu ao subir -- veja %s" % log)
            continue
    p.kill()
    raise SystemExit("o servidor nao subiu na porta %d" % PORTA)


def esperar_porta_fechar(prazo=15):
    fim = time.time() + prazo
    while time.time() < fim:
        try:
            socket.create_connection(("127.0.0.1", PORTA), 0.2).close()
            time.sleep(0.05)
        except OSError:
            return True
    return False


def matar_de_verdade(p):
    """SIGKILL. O nucleo derruba o processo onde ele estiver -- sem tratar,
    sem fechar janela, sem sincronizar."""
    try:
        os.kill(p.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    p.wait(10)


def derrubar_limpo(p):
    if p is None or p.poll() is not None:
        return
    try:
        p.send_signal(signal.SIGTERM)
        p.wait(10)
    except (ProcessLookupError, subprocess.TimeoutExpired):
        try:
            p.kill()
            p.wait(5)
        except ProcessLookupError:
            pass


# ------------------------------------------------------------------- conexao

class Ligacao:
    """Copia da de `bancada/transacoes/provar.py`: o `makefile()` do Python
    segura o descritor, e fechar so o soquete deixa o fd aberto -- por isso
    esta classe fecha os DOIS quando quem chama pede para morrer de proposito.
    Aqui ela so e usada para desligar CONEXOES QUE O PROPRIO SCRIPT abriu; a
    queda que interessa e sempre a do SERVIDOR, via SIGKILL."""

    def __init__(self, porta=PORTA, prazo=20):
        self.s = socket.create_connection(("127.0.0.1", porta))
        # Sem isto, Nagle + ACK adiado do outro lado troca cada ida-e-volta
        # por ~40 ms parados -- 60 linhas levavam 1,5 s por causa do SOQUETE,
        # nao do servidor. Medido nesta mesma rodada.
        self.s.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        self.s.settimeout(prazo)
        self.f = self.s.makefile("rwb")
        r = self.fala({"op": "login", "usuario": "adm", "senha": "senha-do-adm"})
        if not r.get("ok"):
            raise SystemExit("login: %s" % r)

    def fala(self, p):
        p.setdefault("token", TOKEN)
        self.f.write((json.dumps(p) + "\n").encode())
        self.f.flush()
        return json.loads(self.f.readline().decode())

    def ok(self, p):
        r = self.fala(p)
        if not r.get("ok"):
            raise SystemExit("%s: %s" % (p.get("op"), r.get("erro")))
        return r["resultado"]

    def fechar(self):
        for c in (self.f, self.s):
            try:
                c.close()
            except OSError:
                pass


# --------------------------------------------------------------------- marca

def caminho_db():
    return os.path.join(BASE, "base", DB)


def marcas():
    d = caminho_db()
    if not os.path.isdir(d):
        return []
    return sorted(n for n in os.listdir(d) if n.endswith(".tx"))


# ----------------------------------------------------------- montar esquema

def montar_duas_tabelas(c):
    c.ok({"op": "criar_database", "database": DB})
    for t in ("a", "b"):
        c.ok({"op": "criar_tabela", "database": DB, "tabela": t,
              "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                          {"nome": "v", "tipo": "Str(20)"}],
              "indices": [{"nome": "pk", "colunas": ["id"], "unico": True,
                           "primario": True}]})


def montar_cascata(c, filhas):
    c.ok({"op": "criar_database", "database": DB})
    c.ok({"op": "criar_tabela", "database": DB, "tabela": "maes",
          "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                      {"nome": "nome", "tipo": "Str(20)"}],
          "indices": [{"nome": "pk", "colunas": ["id"], "unico": True,
                       "primario": True}]})
    c.ok({"op": "criar_tabela", "database": DB, "tabela": "filhas",
          "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                      {"nome": "mae_id", "tipo": "Int8"}],
          "indices": [{"nome": "pk", "colunas": ["id"], "unico": True,
                       "primario": True},
                      {"nome": "porMae", "colunas": ["mae_id"]}],
          "chaves_estrangeiras": [
              {"nome": "fk_mae", "colunas": ["mae_id"], "tabela_ref": "maes",
               "colunas_ref": ["id"], "ao_excluir": "restringir",
               "ao_alterar": "cascata"}]})
    c.ok({"op": "inserir", "database": DB, "tabela": "maes",
          "linha": {"id": 1, "nome": "original"}})
    for i in range(1, filhas + 1):
        c.ok({"op": "inserir", "database": DB, "tabela": "filhas",
              "linha": {"id": i, "mae_id": 1}})


# ------------------------------------------------------------ ler o relatorio

_CAMPOS = {
    "achadas": r"transacoes achadas \.+ (\d+)",
    "descartadas": r"marcas ilegiveis descartadas \.+ (\d+)",
    "completadas": r"transacoes completadas \.+ (\d+)",
    "reaplicadas": r"operacoes reaplicadas \.+ (\d+)",
    "ja_aplicadas": r"operacoes ja aplicadas \.+ (\d+)",
    "indices_reconstruidos": r"indices reconstruidos \.+ (\d+)",
    "impossiveis": r"operacoes IMPOSSIVEIS \.+ (\d+)",
}


def ler_relatorio(offset):
    """O bloco 'PHXSQL Recovery' escrito DEPOIS do offset dado (o tamanho do
    log antes deste `subir()`). Devolve None quando nao ha bloco -- que e a
    resposta certa quando `achadas == 0` (SS5.3: ele nao sai sem marca nenhuma)."""
    caminho = os.path.join(BASE, "servidor.log")
    with open(caminho, "rb") as f:
        f.seek(offset)
        trecho = f.read().decode("utf-8", "replace")
    if "PHXSQL Recovery" not in trecho:
        return None
    bloco = trecho[trecho.index("PHXSQL Recovery"):]
    r = {"impossiveis_linhas": []}
    for chave, regex in _CAMPOS.items():
        m = re.search(regex, bloco)
        r[chave] = int(m.group(1)) if m else 0
    for m in re.finditer(r"^\s*! (.+)$", bloco, re.MULTILINE):
        r["impossiveis_linhas"].append(m.group(1))
    return r


def registros(c, tabela):
    r = c.fala({"op": "esquema", "database": DB, "tabela": tabela})
    return r["resultado"]["registros"] if r.get("ok") else -1


# --------------------------------------------------------- a corrida em si

def calibrar(cfg, montar, n_ops):
    """Um commit LIMPO, sem matar ninguem -- so para medir quanto ele demora
    nesta maquina, agora, com este binario. E o numero que decide a varredura
    de atrasos: nao se chuta a largura da janela, se mede."""
    shutil.rmtree(BASE, ignore_errors=True)
    p, _ = subir(cfg, limpar=False)
    try:
        c = Ligacao()
        montar(c)
        c.ok({"op": "begin"})
        n_ops(c)
        t0 = time.time()
        c.ok({"op": "commit"})
        dt = time.time() - t0
        c.fechar()
        return dt
    finally:
        derrubar_limpo(p)


def corrida(cfg, montar, n_ops, atraso_s, rotulo, deixar_no_ar=False):
    """Uma rodada: sobe, monta o esquema, BEGIN + as operacoes, dispara o
    COMMIT numa thread e mata o processo `atraso_s` depois de a marca `.tx`
    aparecer no disco. `atraso_s` pode ser None -- mata assim que a marca
    aparecer, sem esperar mais nada (a fronteira mais cedo que da para mirar
    de fora do processo).

    `deixar_no_ar=True` devolve `(resultado, p2)` com o servidor RECEM-
    REABERTO ainda de pe, para quem chama inspecionar mais (a cascata precisa
    ler as filhas depois da recuperacao). Quem chama e responsavel por
    `derrubar_limpo(p2)`."""
    shutil.rmtree(BASE, ignore_errors=True)
    p, off = subir(cfg, limpar=False)
    resultado = {"rotulo": rotulo, "atraso_ms": None if atraso_s is None else round(atraso_s * 1000, 3)}
    try:
        c = Ligacao()
        montar(c)
        c.ok({"op": "begin"})
        n_ops(c)

        matou = threading.Event()

        def matador():
            fim = time.time() + 20
            while time.time() < fim:
                if marcas():
                    if atraso_s:
                        time.sleep(atraso_s)
                    matar_de_verdade(p)
                    matou.set()
                    return
                # poll bem apertado: e a marca que decide o instante, nao o
                # sono deste laco.
            # nunca achou marca -- o commit deve ter terminado sozinho antes.

        def enviar_commit():
            try:
                c.fala({"op": "commit"})
            except (OSError, ValueError, json.JSONDecodeError):
                pass  # o servidor morreu com a resposta no meio -- esperado

        t_matador = threading.Thread(target=matador)
        t_commit = threading.Thread(target=enviar_commit)
        t_matador.start()
        t_commit.start()
        t_commit.join(timeout=25)
        t_matador.join(timeout=25)
        resultado["marca_apareceu"] = matou.is_set()
        if not matou.is_set():
            # O commit terminou rapido demais para o matador alcancar --
            # mata mesmo assim, para nao deixar o processo pendurado, e marca
            # a rodada como "tarde demais" em vez de contar como um dos cinco
            # pontos.
            resultado["tarde_demais"] = True
            try:
                matar_de_verdade(p)
            except Exception:
                pass
        try:
            c.fechar()
        except OSError:
            pass
    finally:
        pass

    p2, off2 = subir(cfg, limpar=False)
    try:
        for _ in range(60):
            time.sleep(0.05)
            try:
                socket.create_connection(("127.0.0.1", PORTA), 0.2).close()
                break
            except OSError:
                continue
        c2 = Ligacao()
        resultado["relatorio"] = ler_relatorio(off2)
        resultado["registros"] = {t: registros(c2, t) for t in ("a", "b", "maes", "filhas")
                                   if os.path.exists(os.path.join(caminho_db(), t + ".reg"))}
        resultado["marca_sobrou"] = marcas()
        c2.fechar()
    except Exception:
        derrubar_limpo(p2)
        raise
    if deixar_no_ar:
        return resultado, p2
    derrubar_limpo(p2)
    return resultado


def classificar(r, n_total):
    rel = r.get("relatorio")
    if rel is None:
        if r.get("marca_apareceu") and (r.get("atraso_ms") or 0) > 0:
            # A marca chegou a existir, mas quando o SIGKILL saiu o commit ja
            # tinha se completado e se limpado sozinho -- o atraso mirou
            # tarde demais, e nao e o ponto 1 (nunca houve marca nenhuma).
            return "TARDE_DEMAIS (concluiu sozinho antes do kill)"
        return "P1 sem marca nenhuma (achadas=0)"
    if rel["descartadas"] >= 1:
        return "MARCA_INVALIDA (descartada)"
    if rel["impossiveis"] > 0:
        return "IMPOSSIVEL (%d op.)" % rel["impossiveis"]
    reap, ja = rel["reaplicadas"], rel["ja_aplicadas"]
    if reap == n_total and ja == 0:
        return "P2 nada aplicado (reaplicadas=%d)" % reap
    if reap == 0 and ja == n_total:
        return "P4 tudo aplicado, marca pendente (ja_aplicadas=%d)" % ja
    if 0 < reap < n_total:
        return "P3 parcial (reaplicadas=%d ja_aplicadas=%d)" % (reap, ja)
    return "outro (reaplicadas=%d ja_aplicadas=%d)" % (reap, ja)


# --------------------------------------------------- ponto 1: antes da marca

def ponto1_sem_commit(cfg, montar, n_ops):
    """Mata NO MEIO da transacao, antes de o cliente sequer mandar COMMIT.
    Deterministico -- nao e corrida nenhuma: nao ha `write` de marca porque
    `gravar_marca` so roda dentro do `op_commit`."""
    shutil.rmtree(BASE, ignore_errors=True)
    p, off = subir(cfg, limpar=False)
    try:
        c = Ligacao()
        montar(c)
        c.ok({"op": "begin"})
        n_ops(c)
        antes = marcas()
        matar_de_verdade(p)
    finally:
        pass
    p2, off2 = subir(cfg, limpar=False)
    try:
        for _ in range(60):
            time.sleep(0.05)
            try:
                socket.create_connection(("127.0.0.1", PORTA), 0.2).close()
                break
            except OSError:
                continue
        c2 = Ligacao()
        rel = ler_relatorio(off2)
        regs = {t: registros(c2, t) for t in ("a", "b")}
        c2.fechar()
    finally:
        derrubar_limpo(p2)
    return {"marca_antes_de_matar": antes, "relatorio": rel, "registros": regs}


# ------------------------------------------------- varredura ponto 2/3/4

def varredura_dois_tabelas(regime, n_a, n_b, passos):
    cfg = config(regime)

    def montar(c):
        montar_duas_tabelas(c)

    def n_ops(c):
        for i in range(1, n_a + 1):
            c.ok({"op": "inserir", "database": DB, "tabela": "a",
                  "linha": {"id": i, "v": "x"}})
        for i in range(1, n_b + 1):
            c.ok({"op": "inserir", "database": DB, "tabela": "b",
                  "linha": {"id": i, "v": "y"}})

    n_total = n_a + n_b
    t_limpo = calibrar(cfg, montar, n_ops)
    print("    calibracao (%s, %d+%d linhas, sem matar): %.1f ms"
          % (regime, n_a, n_b, t_limpo * 1000))

    atrasos = [0.0] + [t_limpo * 1.35 * (i / (passos - 1)) for i in range(1, passos)]
    corridas = []
    for i, atraso in enumerate(atrasos):
        r = corrida(cfg, montar, n_ops, atraso, "%s#%d" % (regime, i))
        r["classe"] = ("TARDE_DEMAIS" if r.get("tarde_demais")
                       else classificar(r, n_total))
        corridas.append(r)
        print("      atraso %6.2f ms -> %s" % (r["atraso_ms"] or 0.0, r["classe"]))
    return {"regime": regime, "n_a": n_a, "n_b": n_b, "t_calibracao_ms": t_limpo * 1000,
            "corridas": corridas}


# ---------------------------------------------------- varredura da cascata

def varredura_cascata(regime, filhas, passos):
    cfg = config(regime)

    def montar(c):
        montar_cascata(c, filhas)

    def n_ops(c):
        c.ok({"op": "atualizar", "database": DB, "tabela": "maes",
              "rowid": 1, "linha": {"id": 999, "nome": "trocado"}})

    t_limpo = calibrar(cfg, montar, n_ops)
    print("    calibracao cascata (%s, %d filhas, sem matar): %.1f ms"
          % (regime, filhas, t_limpo * 1000))

    atrasos = [0.0] + [t_limpo * 1.35 * (i / (passos - 1)) for i in range(1, passos)]
    corridas = []
    for i, atraso in enumerate(atrasos):
        r, p2 = corrida(cfg, montar, n_ops, atraso, "%s-cascata#%d" % (regime, i),
                        deixar_no_ar=True)
        rel = r.get("relatorio")
        r["classe"] = "TARDE_DEMAIS" if r.get("tarde_demais") else (
            "SEM_MARCA" if rel is None else
            "MARCA_INVALIDA" if rel["descartadas"] >= 1 else
            "COMPLETADA")
        corridas.append(r)

        c = Ligacao()
        try:
            mae_final = c.ok({"op": "ler", "database": DB, "tabela": "maes", "rowid": 1})

            # O buscar por "porMae" pode recusar: a queda pode ter deixado o
            # INDICE da FILHA sujo -- um mecanismo geral do write-back
            # (`docs/DESEMPENHO.md` SS4.8), independente da marca `.tx`, porque
            # a cascata sincroniza CADA FILHA por conta propria
            # (`Table::aplicar_ao_alterar`) fora da janela de durabilidade do
            # servidor. Isso NAO e silencioso: vira uma linha em
            # `operacoes IMPOSSIVEIS`, e e isto que se mede aqui.
            def buscar_com_reindex(valor):
                resp = c.fala({"op": "buscar", "database": DB, "tabela": "filhas",
                               "indice": "porMae", "chave": [valor], "max": filhas + 10})
                if resp.get("ok"):
                    return len(resp["resultado"]["linhas"]), False
                rr = c.fala({"op": "reindexar", "database": DB, "tabela": "filhas"})
                if not rr.get("ok"):
                    raise SystemExit("reindexar: %s" % rr.get("erro"))
                resp2 = c.ok({"op": "buscar", "database": DB, "tabela": "filhas",
                             "indice": "porMae", "chave": [valor], "max": filhas + 10})
                return len(resp2["linhas"]), True

            n_novas, precisou_reindex_novas = buscar_com_reindex(999)
            n_velhas, precisou_reindex_velhas = buscar_com_reindex(1)
        finally:
            c.fechar()
            derrubar_limpo(p2)
        precisou_reindex = precisou_reindex_novas or precisou_reindex_velhas
        r["indice_da_filha_precisou_reindexar"] = precisou_reindex
        r["mae_id"] = mae_final.get("id") if mae_final else None
        r["filhas_com_mae_nova"] = n_novas
        r["filhas_com_mae_velha"] = n_velhas
        tudo_novo = n_novas == filhas and n_velhas == 0
        tudo_velho = n_novas == 0 and n_velhas == filhas
        # Uma cascata PARCIAL so e aceitavel quando o relatorio a DENUNCIA em
        # `impossiveis` -- e o mesmo criterio do SS5.3: perder em silencio e
        # pior do que dizer que perdeu.
        denunciada = bool(rel) and rel.get("impossiveis", 0) > 0 and any(
            "integridade referencial" in linha or "filhas" in linha
            for linha in rel.get("impossiveis_linhas", []))
        if tudo_novo or tudo_velho:
            r["veredito"] = "CONSISTENTE"
        elif denunciada:
            r["veredito"] = "PARCIAL_DENUNCIADO"
        else:
            r["veredito"] = "*** PARCIAL SEM AVISO ***"
        print("      atraso %6.2f ms -> %-16s mae=%s novas=%d velhas=%d reindex=%s %s"
              % (r["atraso_ms"] or 0.0, r["classe"], r["mae_id"], n_novas, n_velhas,
                 precisou_reindex, r["veredito"]))
    return {"regime": regime, "filhas": filhas, "t_calibracao_ms": t_limpo * 1000,
            "corridas": corridas}


# ------------------------------------- marca por regime, sem matar ninguem

def marcador_por_regime(regime):
    """Quanto tempo a marca FICA no disco depois de um commit que NAO caiu,
    em cada regime -- o eixo em que o regime realmente muda o que se ve
    (SS8 do TRANSACOES.md: a marca some quando a tabela sincroniza, e quem
    decide QUANDO e o regime)."""
    cfg = config(regime, lote_ms=600)
    shutil.rmtree(BASE, ignore_errors=True)
    p, off = subir(cfg, limpar=False)
    try:
        c = Ligacao()
        montar_duas_tabelas(c)
        c.ok({"op": "begin"})
        c.ok({"op": "inserir", "database": DB, "tabela": "a", "linha": {"id": 1, "v": "z"}})
        c.ok({"op": "commit"})
        logo = len(marcas())
        time.sleep(0.05)
        cedo = len(marcas())
        time.sleep(1.2)
        tarde = len(marcas())
        c.fechar()
        return {"regime": regime, "logo_apos_commit": logo, "50ms_depois": cedo,
                "1.25s_depois": tarde}
    finally:
        derrubar_limpo(p)


# ---------------------------------------------- SS5.5(c): tabela apagada

def tabela_apagada_entre_queda_e_arranque(regime, tentativas=8):
    cfg = config(regime)

    def montar(c):
        montar_duas_tabelas(c)

    def n_ops(c):
        for i in range(1, 31):
            c.ok({"op": "inserir", "database": DB, "tabela": "a",
                  "linha": {"id": i, "v": "x"}})

    t_limpo = calibrar(cfg, montar, n_ops)
    achou_marca, rel = False, None
    # Retentativa: com uma marca de so 30 operacoes o `fsync` dela e rapido
    # demais (< 1 ms) para uma folga FIXA acertar sempre -- em `por_operacao`
    # o commit inteiro pode terminar sozinho antes do "matador" acordar. Isto
    # nao e o ponto que este teste quer (ele quer uma marca VALIDA sobrando);
    # entao repete ate conseguir uma, em vez de aceitar uma corrida que nao
    # prova nada.
    for tentativa in range(tentativas):
        shutil.rmtree(BASE, ignore_errors=True)
        p, off = subir(cfg, limpar=False)
        try:
            c = Ligacao()
            montar(c)
            c.ok({"op": "begin"})
            n_ops(c)
            matou = threading.Event()

            def matador():
                fim = time.time() + 20
                while time.time() < fim:
                    if marcas():
                        matar_de_verdade(p)
                        matou.set()
                        return

            def enviar():
                try:
                    c.fala({"op": "commit"})
                except (OSError, ValueError, json.JSONDecodeError):
                    pass

            tm, tc = threading.Thread(target=matador), threading.Thread(target=enviar)
            tm.start(); tc.start()
            tc.join(25); tm.join(25)
            achou_marca = matou.is_set()
        finally:
            pass
        # Simula "alguem apagou a tabela entre a queda e o arranque": remove
        # os arquivos de "a" (mas NAO a marca), como se um DROP TABLE
        # tivesse acontecido nesse intervalo.
        for n in os.listdir(caminho_db()):
            if n.split(".")[0] == "a" and not n.endswith(".tx"):
                os.remove(os.path.join(caminho_db(), n))
        p2, off2 = subir(cfg, limpar=False)
        try:
            for _ in range(60):
                time.sleep(0.05)
                try:
                    socket.create_connection(("127.0.0.1", PORTA), 0.2).close()
                    break
                except OSError:
                    continue
            rel = ler_relatorio(off2)
        finally:
            derrubar_limpo(p2)
        if rel is not None and rel.get("completadas", 0) >= 1:
            break
    return {"regime": regime, "marca_achada_no_kill": achou_marca, "relatorio": rel}


# --------------------------------------------------------------------- main

def main():
    if not os.path.exists(PHXSQLD):
        sys.exit("falta %s -- rode cargo build --release" % PHXSQLD)

    resultado = {"gerado_em": time.strftime("%Y-%m-%d %H:%M:%S"), "regimes": {}}

    print("== marca .tx por regime: quanto tempo ela fica no disco ==")
    resultado["marcador_por_regime"] = [marcador_por_regime(r) for r in REGIMES]
    for m in resultado["marcador_por_regime"]:
        print("  %-13s logo=%d 50ms=%d 1.25s=%d"
              % (m["regime"], m["logo_apos_commit"], m["50ms_depois"], m["1.25s_depois"]))

    print("\n== ponto 1: antes do fsync da marca (deterministico) ==")
    resultado["ponto1"] = []
    for regime in REGIMES:
        cfg = config(regime)

        def montar(c):
            montar_duas_tabelas(c)

        def n_ops(c):
            for i in range(1, 51):
                c.ok({"op": "inserir", "database": DB, "tabela": "a",
                      "linha": {"id": i, "v": "x"}})

        r = ponto1_sem_commit(cfg, montar, n_ops)
        r["regime"] = regime
        resultado["ponto1"].append(r)
        print("  %-13s marca_antes=%s relatorio=%s registros=%s"
              % (regime, r["marca_antes_de_matar"], r["relatorio"], r["registros"]))

    print("\n== pontos 2/3/4: varredura de atraso, duas tabelas ==")
    resultado["varredura"] = []
    for regime in REGIMES:
        print("  -- %s --" % regime)
        resultado["varredura"].append(varredura_dois_tabelas(regime, n_a=1500, n_b=1500, passos=9))

    print("\n== ponto 5: no meio da cascata do ao_alterar ==")
    resultado["cascata"] = []
    for regime in REGIMES:
        print("  -- %s --" % regime)
        resultado["cascata"].append(varredura_cascata(regime, filhas=1200, passos=7))

    print("\n== SS5.5(c): a tabela nao abre mais no arranque ==")
    resultado["tabela_apagada"] = []
    for regime in REGIMES:
        r = tabela_apagada_entre_queda_e_arranque(regime)
        resultado["tabela_apagada"].append(r)
        print("  %-13s marca_no_kill=%s impossiveis=%s"
              % (regime, r["marca_achada_no_kill"],
                 (r["relatorio"] or {}).get("impossiveis_linhas")))

    shutil.rmtree(BASE, ignore_errors=True)
    with open(RESULTADO, "w") as f:
        json.dump(resultado, f, indent=2, ensure_ascii=False)
    print("\nresultado medido gravado em %s" % RESULTADO)


if __name__ == "__main__":
    main()
