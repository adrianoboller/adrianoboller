#!/usr/bin/env python3
"""O fluxo dos TRES numeros crescentes do PhxSql, contra o motor vivo.

    python3 bancada/sequencias/sonda.py

Pergunta do dono, 07/09/2026: *«Muitos bancos tem o sequence tipo o PostgreSQL
e como e isso no phxsql?»* e, na mesma rodada, *«Fluxo do funcionamento do auto
number id e do sequence como esta e como seria o ideal.»*

Sao TRES numeros por tabela, e confundi-los foi o primeiro erro desta bancada:

  * `rowid`   -- a posicao fisica no `.reg`, que nunca reaproveita slot;
  * `rownum`  -- a coluna de SISTEMA, ordem de chegada, contador nos bytes
                 92..100 do cabecalho do volume 1;
  * `Sequence`-- a coluna do USUARIO, uma so por tabela, contador nos bytes
                 36..44 do MESMO cabecalho.

A parte I (blocos 1 a 8) e o que a resposta `docs/pdf/respostas/00-sequencia.md`
publica. A parte II (blocos 9 em diante) percorre o CICLO DE VIDA de cada um dos
tres -- lote, carga reservada, transacao, exclusao, particao, queda, replicacao,
promocao, bidirecional -- e e de onde o `docs/AUTONUMBER.md` tira cada numero.

Escreve `bancada/sequencias/resultados.json` com a data da corrida: os numeros
do documento saem de la, e nao da memoria de quem escreveu.
"""
import json, os, shutil, signal, socket, struct, subprocess, sys, time

PORTA = int(os.environ.get("PHX_SONDA_PORTA", "6900"))
BASE = f"/tmp/phx-seq-{os.getpid()}"
BIN = os.environ.get("PHX_SONDA_BIN", "target/release/phxsqld")
AQUI = os.path.dirname(os.path.abspath(__file__))

# O que cada bloco mediu, para o `resultados.json`. Escrito pelos blocos, nunca
# a mao: numero digitado num relatorio envelhece calado.
MEDIDO = {}


class Servidor:
    """Um `phxsqld` de pe, numa base propria, falado por soquete."""

    def __init__(self, porta=None, rotulo="", extra=None):
        self.porta = porta or PORTA
        self.base = BASE if not rotulo else f"{BASE}-{rotulo}"
        self.extra = extra or {}

    def __enter__(self):
        if not os.path.exists(BIN):
            raise SystemExit(
                f"{BIN} nao existe: flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server"
            )
        shutil.rmtree(self.base, ignore_errors=True)
        os.makedirs(self.base + "/dados")
        cfg = self.base + "/config.json"
        c = {"bind": f"127.0.0.1:{self.porta}", "base": self.base + "/dados",
             "token": "t", "web": {"ligado": False}}
        c.update(self.extra)
        json.dump(c, open(cfg, "w"))
        self.cfg = cfg
        self.subir()
        return self

    def subir(self):
        self.p = subprocess.Popen([BIN, "--config", self.cfg],
                                  stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(150):
            try:
                socket.create_connection(("127.0.0.1", self.porta), 0.2).close()
                break
            except OSError:
                time.sleep(0.1)
        else:
            raise SystemExit(f"servidor da porta {self.porta} nao subiu")
        self.s = socket.create_connection(("127.0.0.1", self.porta), 10)
        self.f = self.s.makefile("rwb")

    def derrubar(self, sinal=signal.SIGTERM):
        """Derruba ESTE processo, pelo PID guardado. Nunca por nome."""
        try:
            self.f.close(); self.s.close()
        except OSError:
            pass
        self.p.send_signal(sinal)
        self.p.wait(timeout=15)

    def __exit__(self, *_):
        try:
            self.derrubar()
        except Exception:
            pass
        shutil.rmtree(self.base, ignore_errors=True)

    def pedir(self, **kw):
        kw.setdefault("token", "t")
        self.f.write((json.dumps(kw) + "\n").encode()); self.f.flush()
        r = json.loads(self.f.readline().decode())
        if isinstance(r.get("resultado"), dict):
            r = {**r["resultado"], **{k: v for k, v in r.items() if k != "resultado"}}
        return r

    def cru(self, texto):
        """Manda a linha exatamente como esta e devolve a resposta CRUA.

        Existe porque o `json` do Python no meio do caminho ESCONDE o defeito
        do bloco 19: um inteiro acima de 2^53 perde precisao no protocolo, e
        quem serializa e desserializa dos dois lados nunca ve a perda.
        """
        self.f.write((texto + "\n").encode()); self.f.flush()
        return self.f.readline().decode().strip()

    def cabecalho(self, database, tabela, volume=1, sufixo=None):
        """Os contadores lidos do DISCO. `sufixo` para a particao por letra,
        onde o volume 1 e o balde `_A` e nao `_001`."""
        d = f"{self.base}/dados/{database}"
        if sufixo is not None:
            nome = f"{d}/{tabela}{sufixo}.reg"
        elif volume == 1 and os.path.exists(f"{d}/{tabela}.reg"):
            nome = f"{d}/{tabela}.reg"
        else:
            nome = f"{d}/{tabela}_{volume:03d}.reg"
        if not os.path.exists(nome):
            return None
        with open(nome, "rb") as f:
            b = f.read(128)
        u = lambda o: struct.unpack_from("<Q", b, o)[0]
        return {"slot_count": u(20), "live_count": u(28), "proxima_sequencia": u(36),
                "proximo_rownum": u(92), "marcadas": u(108)}


def diz(rot, r, corte=160):
    print(f"  [{'OK  ' if r.get('ok') else 'ERRO'}] {rot}"
          + ("" if r.get("ok") else " -- " + str(r.get("erro"))[:corte]))
    return r


def tres(sv, database, tabela, **kw):
    """As linhas como (rowid, Sequence, rownum) -- os tres numeros lado a lado."""
    r = sv.pedir(op="varrer", database=database, tabela=tabela, max=kw.pop("max", 300), **kw)
    return [(l["rowid"], l.get("id"), l.get("rownum")) for l in r.get("linhas", [])]


def tabela_com_sequencia(sv, database, tabela, unico=True, **extra):
    idx = ([{"nome": "pk_" + tabela, "colunas": ["id"], "unico": True, "primario": True}]
           if unico else [])
    return sv.pedir(op="criar_tabela", database=database, tabela=tabela,
                    colunas=[{"nome": "id", "tipo": "Sequence", "obrigatoria": True},
                             {"nome": "c", "tipo": "Str(30)", "obrigatoria": True}],
                    indices=idx, **extra)


def hash_da_senha(senha):
    """O hash sai do proprio servidor -- nao ha uma segunda implementacao."""
    saida = subprocess.run([BIN, "--senha"], input=senha + "\n",
                           capture_output=True, text=True).stdout
    return saida.split('": "')[1].split('"')[0]


PERM = {"*": {"ler": True, "inserir": True, "alterar": True, "excluir": True,
              "criar": True, "administrar": True, "diario": True,
              "verificar": True, "replicar": True}}


# ------------------------------------------------------------------ parte I

def parte_i(sv):
    print("=== 1. Declarar: a coluna e do tipo `Sequence` (uma so por tabela)")
    diz("criar_tabela com id Sequence", sv.pedir(
        op="criar_tabela", database="loja", tabela="pedidos",
        colunas=[{"nome": "id", "tipo": "Sequence", "obrigatoria": True},
                 {"nome": "cliente", "tipo": "Str(40)", "obrigatoria": True}],
        indices=[{"nome": "porId", "colunas": ["id"], "unico": True}]))
    diz("segunda coluna Sequence na mesma tabela (tem de recusar)", sv.pedir(
        op="criar_tabela", database="loja", tabela="duas",
        colunas=[{"nome": "a", "tipo": "Sequence"}, {"nome": "b", "tipo": "Sequence"}]))

    print("\n=== 2. Inserir SEM informar o id: o motor numera")
    for c in ("Alves", "Silva", "Andrade"):
        r = sv.pedir(op="inserir", database="loja", tabela="pedidos", linha={"cliente": c})
        print(f"  inserir {c:8} -> ok={r.get('ok')} rowid={r.get('rowid')}")
    r = sv.pedir(op="varrer", database="loja", tabela="pedidos", max=10)
    print("  linhas:", [(l["id"], l["cliente"]) for l in r.get("linhas", [])])

    print("\n=== 3. Inserir INFORMANDO o id (o contador acompanha, como no PostgreSQL nao acompanha)")
    diz("inserir id=100", sv.pedir(op="inserir", database="loja", tabela="pedidos",
                                   linha={"id": 100, "cliente": "Manual"}))
    r = sv.pedir(op="inserir", database="loja", tabela="pedidos", linha={"cliente": "Depois do 100"})
    r2 = sv.pedir(op="ler", database="loja", tabela="pedidos", rowid=r.get("rowid"))
    seguinte = (r2.get("linha") or r2).get("id")
    print("  o proximo automatico saiu:", seguinte)
    MEDIDO["acompanha_valor_a_mao"] = {"gravado_a_mao": 100, "proximo_automatico": seguinte}

    print("\n=== 4. Listar os contadores: op `sequencias`")
    r = sv.pedir(op="sequencias", database="loja")
    for s in r.get("sequencias", []):
        print("  ", s)

    print("\n=== 5. Ajustar: op `ajustar_sequencia` (exige administrar)")
    diz("zerar (proxima=0)", sv.pedir(op="ajustar_sequencia", database="loja",
                                      tabela="pedidos", proxima=0))
    diz("pular para 5000", sv.pedir(op="ajustar_sequencia", database="loja",
                                    tabela="pedidos", proxima=5000))
    r = sv.pedir(op="inserir", database="loja", tabela="pedidos", linha={"cliente": "Depois do salto"})
    r2 = sv.pedir(op="ler", database="loja", tabela="pedidos", rowid=r.get("rowid"))
    print("  o proximo automatico saiu:", (r2.get("linha") or r2).get("id"))

    print("\n=== 6. Ajustar para TRAS de uma chave ja gravada: o indice unico e quem recusa a repeticao")
    diz("ajustar para 1 (ja existe)", sv.pedir(op="ajustar_sequencia", database="loja",
                                               tabela="pedidos", proxima=1))
    r = diz("inserir depois do ajuste (o id 1 ja existe: tem de recusar)",
            sv.pedir(op="inserir", database="loja", tabela="pedidos", linha={"cliente": "Repetido"}))
    MEDIDO["ajuste_para_tras_com_indice_unico"] = {"recusou": not r.get("ok"),
                                                   "erro": r.get("erro")}

    print("\n=== 7. Onde o contador mora: byte 36 do cabecalho do volume 1 (o 92 e o do rownum)")
    # A primeira versao desta sonda leu o byte 92 e imprimiu 7 -- que e o
    # proximo_rownum, nao a sequencia. FORMATO.md tem os dois contadores:
    # `proxima_sequencia` em 36 e `proximo_rownum` em 92. Imprimir os dois
    # lado a lado e o que impede a confusao de voltar.
    cab = sv.cabecalho("loja", "pedidos")
    print(f"  byte 36 proxima_sequencia = {cab['proxima_sequencia']}   "
          f"byte 92 proximo_rownum = {cab['proximo_rownum']}")
    print("  (o cabecalho vai ao disco a cada insercao, num `write` de 128 bytes;")
    print("   o `fsync` e que espera o fecho da janela de durabilidade -- bloco 18)")

    print("\n=== 8. Reabrir e conferir que o contador sobreviveu")
    print("  (a persistencia do contador entre reaberturas e provada nos testes do phxsql-store")
    print("   e, contra o sistema operacional, no bloco 18 desta sonda)")


# ----------------------------------------------------------------- parte II

def bloco_9(sv):
    print("\n=== 9. Os TRES numeros lado a lado numa insercao comum")
    tabela_com_sequencia(sv, "loja", "tres")
    for c in "ABC":
        sv.pedir(op="inserir", database="loja", tabela="tres", linha={"c": c})
    linhas = tres(sv, "loja", "tres")
    print("  (rowid, Sequence, rownum):", linhas)
    print("  cabecalho:", sv.cabecalho("loja", "tres"))
    MEDIDO["insercao_comum"] = {"linhas": linhas, "cabecalho": sv.cabecalho("loja", "tres")}


def bloco_10(sv):
    print("\n=== 10. `inserir_lote`: a numeracao e a mesma, uma linha por vez la dentro")
    tabela_com_sequencia(sv, "loja", "lote")
    sv.pedir(op="inserir_lote", database="loja", tabela="lote",
             linhas=[{"c": f"x{i}"} for i in range(5)])
    print("  cinco de uma vez:", tres(sv, "loja", "lote"))
    sv.pedir(op="inserir_lote", database="loja", tabela="lote",
             linhas=[{"c": "a"}, {"id": 900, "c": "b"}, {"c": "c"}])
    linhas = tres(sv, "loja", "lote")
    print("  com um id a mao no meio:", linhas)
    print("  cabecalho:", sv.cabecalho("loja", "lote"))
    MEDIDO["lote"] = {"linhas": linhas, "cabecalho": sv.cabecalho("loja", "lote")}


def bloco_11(sv):
    print("\n=== 11. `BULKINSERT`: a reserva da tabela nao muda a numeracao")
    tabela_com_sequencia(sv, "loja", "carga")
    diz("reservar", sv.pedir(op="bulkinsert", database="loja", tabela="carga", ligado=True))
    for i in range(3):
        sv.pedir(op="inserir", database="loja", tabela="carga", linha={"c": f"y{i}"})
    durante = sv.cabecalho("loja", "carga")
    diz("soltar", sv.pedir(op="bulkinsert", database="loja", tabela="carga", ligado=False))
    depois = sv.cabecalho("loja", "carga")
    print("  cabecalho durante a carga:", durante)
    print("  cabecalho depois de soltar:", depois)
    print("  linhas:", tres(sv, "loja", "carga"))
    MEDIDO["bulkinsert"] = {"durante": durante, "depois": depois,
                            "contador_mudou_no_fecho": durante != depois}


def bloco_12(sv):
    print("\n=== 12. Transacao: o ROLLBACK devolve o numero, e o COMMIT e quem numera")
    tabela_com_sequencia(sv, "loja", "tx")
    sv.pedir(op="inserir", database="loja", tabela="tx", linha={"c": "antes"})
    antes = sv.cabecalho("loja", "tx")
    sv.pedir(op="begin")
    sv.pedir(op="inserir", database="loja", tabela="tx", linha={"c": "na transacao"})
    dentro = tres(sv, "loja", "tx")
    durante = sv.cabecalho("loja", "tx")
    print("  o que a transacao VE da propria linha:", dentro)
    print("  cabecalho durante a transacao:", durante)
    sv.pedir(op="rollback")
    pos_rollback = sv.cabecalho("loja", "tx")
    print("  cabecalho depois do ROLLBACK:", pos_rollback)
    sv.pedir(op="begin")
    sv.pedir(op="inserir", database="loja", tabela="tx", linha={"c": "confirmada"})
    sv.pedir(op="commit")
    depois = tres(sv, "loja", "tx")
    print("  depois do COMMIT:", depois)
    print("  cabecalho:", sv.cabecalho("loja", "tx"))
    MEDIDO["transacao"] = {
        "sequencia_e_rownum_dentro_da_transacao": dentro[-1],
        "contador_antes": antes, "contador_durante": durante,
        "contador_pos_rollback": pos_rollback,
        "rollback_devolve_o_numero": antes == pos_rollback,
        "linhas_pos_commit": depois,
    }


def bloco_13(sv):
    print("\n=== 13. Exclusao suave, restaurar e de vez: nenhum dos tres contadores volta")
    tabela_com_sequencia(sv, "loja", "ex")
    for c in "ABCDE":
        sv.pedir(op="inserir", database="loja", tabela="ex", linha={"c": c})
    passos = [("antes", sv.cabecalho("loja", "ex"))]
    sv.pedir(op="excluir", database="loja", tabela="ex", rowid=2, motivo="teste")
    passos.append(("suave no rowid 2", sv.cabecalho("loja", "ex")))
    sv.pedir(op="restaurar", database="loja", tabela="ex", rowid=2, motivo="volta")
    passos.append(("restaurar o 2", sv.cabecalho("loja", "ex")))
    sv.pedir(op="excluir", database="loja", tabela="ex", rowid=3, fisico=True, motivo="teste")
    passos.append(("de vez no rowid 3", sv.cabecalho("loja", "ex")))
    sv.pedir(op="inserir", database="loja", tabela="ex", linha={"c": "F"})
    passos.append(("a insercao seguinte", sv.cabecalho("loja", "ex")))
    for rot, c in passos:
        print(f"  {rot:22} seq={c['proxima_sequencia']} rownum={c['proximo_rownum']} "
              f"slots={c['slot_count']} vivas={c['live_count']} marcadas={c['marcadas']}")
    print("  linhas:", tres(sv, "loja", "ex"))
    MEDIDO["exclusao"] = {"passos": {r: c for r, c in passos}, "linhas": tres(sv, "loja", "ex")}


def bloco_14(sv):
    print("\n=== 14. Particao por LETRA: o `rownum` NAO cresce com o `rowid`, e o contador mora no `_A`")
    tabela_com_sequencia(sv, "loja", "alfa", registros_por_arquivo=1000,
                         particao="letra", particao_coluna="c")
    for n in ("Silva", "Alves", "Andrade", "Zico"):
        r = sv.pedir(op="inserir", database="loja", tabela="alfa", linha={"c": n})
        print(f"  {n:9} rowid={r.get('rowid')}")
    linhas = tres(sv, "loja", "alfa")
    print("  (rowid, Sequence, rownum):", linhas)
    arquivos = sorted(f for f in os.listdir(f"{sv.base}/dados/loja")
                      if f.startswith("alfa") and f.endswith(".reg"))
    print("  volumes:", arquivos)
    print("  cabecalho do balde _A (o volume 1):", sv.cabecalho("loja", "alfa", sufixo="_A"))
    print("  cabecalho do balde _S:", sv.cabecalho("loja", "alfa", sufixo="_S"))
    sv.pedir(op="begin")
    r = diz("inserir DENTRO de transacao numa alfanumerica (tem de recusar)",
            sv.pedir(op="inserir", database="loja", tabela="alfa", linha={"c": "Costa"}))
    sv.pedir(op="rollback")
    MEDIDO["particao_por_letra"] = {
        "linhas": linhas, "volumes": arquivos,
        "cab_A": sv.cabecalho("loja", "alfa", sufixo="_A"),
        "cab_S": sv.cabecalho("loja", "alfa", sufixo="_S"),
        "transacao_recusa": r.get("erro"),
    }


def bloco_15(sv):
    print("\n=== 15. Particao por QUANTIDADE e por PERIODO: o contador e so do volume 1")
    tabela_com_sequencia(sv, "loja", "qtd", registros_por_arquivo=2)
    for i in range(5):
        sv.pedir(op="inserir", database="loja", tabela="qtd", linha={"c": f"n{i}"})
    print("  quantidade -> (rowid, Sequence, rownum):", tres(sv, "loja", "qtd"))
    for v in (1, 2, 3):
        print(f"    volume {v}:", sv.cabecalho("loja", "qtd", v))
    sv.pedir(op="criar_tabela", database="loja", tabela="mov",
             colunas=[{"nome": "id", "tipo": "Sequence", "obrigatoria": True},
                      {"nome": "quando", "tipo": "Date", "obrigatoria": True}],
             indices=[{"nome": "pk_mov", "colunas": ["id"], "unico": True, "primario": True}],
             registros_por_arquivo=1000, particao="mensal", particao_coluna="quando")
    for d in ("2026-01-05", "2026-02-03", "2026-03-11", "2026-01-30"):
        sv.pedir(op="inserir", database="loja", tabela="mov", linha={"quando": d})
    r = sv.pedir(op="varrer", database="loja", tabela="mov", max=10)
    print("  periodo -> (rowid, Sequence, rownum):",
          [(l["rowid"], l["id"], l["rownum"]) for l in r.get("linhas", [])])
    print("    volume 1:", sv.cabecalho("loja", "mov", 1))
    print("    volume 2:", sv.cabecalho("loja", "mov", 2))
    MEDIDO["particao_por_volume"] = {
        "quantidade": tres(sv, "loja", "qtd"),
        "cab_qtd_v1": sv.cabecalho("loja", "qtd", 1),
        "cab_qtd_v2": sv.cabecalho("loja", "qtd", 2),
        "cab_mov_v1": sv.cabecalho("loja", "mov", 1),
        "cab_mov_v2": sv.cabecalho("loja", "mov", 2),
    }


def bloco_16(sv):
    print("\n=== 16. Ajustar para TRAS SEM indice unico: repete calado")
    tabela_com_sequencia(sv, "loja", "semu", unico=False)
    for i in range(3):
        sv.pedir(op="inserir", database="loja", tabela="semu", linha={"c": f"n{i}"})
    diz("ajustar para 1", sv.pedir(op="ajustar_sequencia", database="loja",
                                   tabela="semu", proxima=1))
    diz("inserir depois do ajuste", sv.pedir(op="inserir", database="loja",
                                             tabela="semu", linha={"c": "repetido"}))
    linhas = tres(sv, "loja", "semu")
    ids = [l[1] for l in linhas]
    print("  (rowid, Sequence, rownum):", linhas)
    print(f"  ids: {ids} -- repetidos: {len(ids) - len(set(ids))}")
    MEDIDO["ajuste_para_tras_sem_indice_unico"] = {"linhas": linhas,
                                                   "repetidos": len(ids) - len(set(ids))}


def bloco_17(sv):
    print("\n=== 17. O contador reposto para tras NO DISCO: o CRC do cabecalho pega")
    tabela_com_sequencia(sv, "loja", "crc")
    for i in range(4):
        sv.pedir(op="inserir", database="loja", tabela="crc", linha={"c": f"n{i}"})
    antes = sv.cabecalho("loja", "crc")
    sv.derrubar()
    with open(f"{sv.base}/dados/loja/crc.reg", "r+b") as f:
        f.seek(36); f.write(struct.pack("<Q", 2))
    sv.subir()
    depois = sv.cabecalho("loja", "crc")
    v = diz("verificar", sv.pedir(op="verificar", database="loja", tabela="crc"))
    r = diz("reparar", sv.pedir(op="reparar", database="loja", tabela="crc"))
    i = diz("inserir", sv.pedir(op="inserir", database="loja", tabela="crc", linha={"c": "z"}))
    print(f"  contador antes {antes['proxima_sequencia']} -> reposto {depois['proxima_sequencia']}")
    MEDIDO["cabecalho_adulterado"] = {"antes": antes, "depois": depois,
                                      "verificar": v.get("erro"), "reparar": r.get("erro"),
                                      "inserir": i.get("erro")}


def bloco_18():
    print("\n=== 18. A QUEDA do processo (SIGKILL): o contador nao volta atras")
    with Servidor(PORTA + 3, "queda") as sv:
        sv.pedir(op="criar_database", database="loja")
        tabela_com_sequencia(sv, "loja", "q")
        for i in range(7):
            sv.pedir(op="inserir", database="loja", tabela="q", linha={"c": f"n{i}"})
        antes = sv.cabecalho("loja", "q")
        sv.derrubar(signal.SIGKILL)
        no_disco = sv.cabecalho("loja", "q")
        sv.subir()
        reaberto = sv.cabecalho("loja", "q")
        r = sv.pedir(op="inserir", database="loja", tabela="q", linha={"c": "depois da queda"})
        linhas = tres(sv, "loja", "q")
        print("  antes do SIGKILL :", antes)
        print("  no disco, morto  :", no_disco)
        print("  reaberto         :", reaberto)
        print("  a insercao seguinte:", linhas[-1])
        ids = [l[1] for l in linhas]
        MEDIDO["queda_do_processo"] = {
            "antes": antes, "no_disco": no_disco, "reaberto": reaberto,
            "repetidos_depois": len(ids) - len(set(ids)), "linhas": linhas}


def bloco_19():
    print("\n=== 19. O teto REAL do numero no protocolo: 2^53, e nao 2^64-1")
    # O `Json` desta casa tem um so tipo numerico -- `Numero(f64)`. Acima de
    # 2^53 o inteiro deixa de ser representavel, e a perda acontece em SILENCIO.
    # A coluna `c` guarda o texto do que foi MANDADO, para a divergencia
    # aparecer lado a lado em vez de precisar de fe.
    #
    # DEPOIS DO CONSERTO DA FRENTE G2 (defeito b, ver docs/AUTONUMBER.md Parte
    # C): num numero CRU acima de 2^53 o `inserir` passa a RECUSAR ("perde
    # precisao"), entao a linha nem entra -- ler "0 de 3 divergentes" aqui NAO
    # quer dizer que o numero foi gravado certo, quer dizer que foi recusado. O
    # mesmo valor como TEXTO ("id": str(v)) atravessa intacto. Quem reexecutar
    # esta sonda com o binario novo deve olhar o `ok` de cada `inserir`.
    with Servidor(PORTA + 4, "teto") as sv:
        sv.pedir(op="criar_database", database="loja")
        tabela_com_sequencia(sv, "loja", "t", unico=False)
        for v in (9007199254740992, 9007199254740993, 9007199254740995):
            sv.cru(json.dumps({"token": "t", "op": "inserir", "database": "loja",
                               "tabela": "t", "linha": {"id": v, "c": str(v)}}))
        crua = sv.cru(json.dumps({"token": "t", "op": "varrer", "database": "loja",
                                  "tabela": "t", "max": 9}))
        trecho = crua[crua.index('"linhas"'):]
        print("  a resposta CRUA do servidor (mandado na coluna `c`, gravado em `id`):")
        for pedaco in trecho.split("},{"):
            print("   ", pedaco.strip("[]{}"))
        r = sv.pedir(op="varrer", database="loja", tabela="t", max=9)
        perdidos = [l for l in r.get("linhas", []) if str(l["id"]) != l["c"]]
        print(f"  linhas em que o gravado DIVERGE do mandado: {len(perdidos)} de 3")
        MEDIDO["teto_do_protocolo"] = {
            "limite_exato": 9007199254740992,
            "divergentes": [{"mandado": l["c"], "gravado": l["id"]} for l in perdidos],
            "resposta_crua": trecho[:400]}


def bloco_20(sv):
    print("\n=== 20. Backup e restauracao: o contador volta ao INSTANTE do backup")
    tabela_com_sequencia(sv, "loja", "bkp")
    for i in range(4):
        sv.pedir(op="inserir", database="loja", tabela="bkp", linha={"c": f"n{i}"})
    no_backup = sv.cabecalho("loja", "bkp")
    destino = sv.base + "/bkp"
    diz("backup", sv.pedir(op="backup", database="loja", destino=destino))
    for i in range(3):
        sv.pedir(op="inserir", database="loja", tabela="bkp", linha={"c": f"depois-{i}"})
    agora = sv.cabecalho("loja", "bkp")
    diz("restaurar com outro nome", sv.pedir(op="restaurar_backup", origem=destino,
                                             database="restaurada", de="loja"))
    restaurada = sv.cabecalho("restaurada", "bkp")
    print("  no instante do backup:", no_backup)
    print("  no banco vivo, depois:", agora)
    print("  no banco restaurado  :", restaurada)
    MEDIDO["backup"] = {"no_backup": no_backup, "vivo": agora, "restaurado": restaurada,
                        "volta_ao_instante": no_backup == restaurada}


def bloco_21(sv):
    print("\n=== 21. `reindexar` e `verificar` nao tocam nos contadores")
    antes = sv.cabecalho("loja", "tres")
    diz("reindexar", sv.pedir(op="reindexar", database="loja", tabela="tres"))
    diz("verificar", sv.pedir(op="verificar", database="loja", tabela="tres"))
    depois = sv.cabecalho("loja", "tres")
    print("  antes :", antes)
    print("  depois:", depois)
    MEDIDO["reindexar"] = {"antes": antes, "depois": depois, "igual": antes == depois}


# ------------------------------------------------- os blocos de replicacao

def servidor_replicado(porta, rotulo, replicacao, h, somente_leitura=False):
    extra = {"replicacao": replicacao,
             "usuarios": [{"login": "adm", "nome": "A", "id": 10,
                           "senha_hash": h, "bases": PERM}]}
    if somente_leitura:
        extra["somente_leitura"] = True
    return Servidor(porta, rotulo, extra=extra)


def bloco_22_23(h):
    print("\n=== 22/23. Replicacao: o valor vem da ORIGEM; e a promovida continua de onde ELA parou")
    pf, pr = PORTA + 10, PORTA + 11
    fonte = servidor_replicado(pf, "fonte", {"papel": "source", "imagem_da_linha": True,
                                             "id_servidor": "fonte"}, h)
    # `replica`, e nao `spare`: o spare recusa ATE LEITURA (codigo 4004), e a
    # primeira corrida desta sonda imprimiu a lista de linhas vazia sem que
    # nada estivesse errado -- o instrumento mentindo, nao o motor.
    spare = servidor_replicado(pr, "replica", {
        "papel": "replica", "id_servidor": "replica01", "imagem_da_linha": True,
        "origens": [{"nome": "fonte", "host": "127.0.0.1", "porta": pf, "token": "t",
                     "usuario": "adm", "senha_hash": h, "databases": ["loja"],
                     "reconectar_em": 1}]}, h, somente_leitura=True)
    with fonte, spare:
        fonte.pedir(op="login", usuario="adm", senha="segredo1")
        spare.pedir(op="login", usuario="adm", senha="segredo1")
        fonte.pedir(op="criar_database", database="loja")
        tabela_com_sequencia(fonte, "loja", "p")
        for c in "ABCDE":
            fonte.pedir(op="inserir", database="loja", tabela="p", linha={"c": c})
        fonte.pedir(op="inserir", database="loja", tabela="p", linha={"id": 700, "c": "a mao"})
        fonte.pedir(op="inserir", database="loja", tabela="p", linha={"c": "depois do 700"})
        fonte.pedir(op="excluir", database="loja", tabela="p", rowid=3, fisico=True, motivo="t")
        time.sleep(6)
        cf, cr = fonte.cabecalho("loja", "p"), spare.cabecalho("loja", "p")
        print("  fonte  :", tres(fonte, "loja", "p"))
        print("  replica:", tres(spare, "loja", "p"))
        print("  cabecalho da fonte  :", cf)
        print("  cabecalho da replica:", cr)
        diz("escrever direto na replica (somente_leitura: tem de recusar)",
            spare.pedir(op="inserir", database="loja", tabela="p", linha={"c": "na replica"}))

        print("\n  -- a promocao com ATRASO: a replica para, a fonte continua, a fonte morre")
        spare.p.send_signal(signal.SIGSTOP)
        for i in range(5):
            fonte.pedir(op="inserir", database="loja", tabela="p", linha={"c": f"que a replica nao viu {i}"})
        fonte_no_fim = fonte.cabecalho("loja", "p")
        fonte.derrubar(signal.SIGKILL)
        spare.p.send_signal(signal.SIGCONT)
        time.sleep(3)
        diz("spare_promover", spare.pedir(op="spare_promover", motivo="a fonte morreu"))
        promovida = spare.cabecalho("loja", "p")
        r = spare.pedir(op="inserir", database="loja", tabela="p", linha={"c": "depois da promocao"})
        depois = tres(spare, "loja", "p")
        print("  a fonte morta tinha  :", fonte_no_fim)
        print("  a promovida tinha    :", promovida)
        print("  e a insercao dela saiu:", depois[-1])
        MEDIDO["replicacao"] = {
            "cab_fonte": cf, "cab_replica": cr, "iguais": cf == cr,
            "linhas_da_replica": tres(spare, "loja", "p")[:6]}
        MEDIDO["promocao_com_atraso"] = {
            "fonte_no_fim": fonte_no_fim, "promovida": promovida,
            "primeira_da_promovida": depois[-1],
            "numeros_reemitidos": max(0, fonte_no_fim["proxima_sequencia"]
                                      - promovida["proxima_sequencia"])}


def bloco_24(h):
    print("\n=== 24. Bidirecional: os dois numerando a mesma faixa, e a faixa disjunta que nao dura")
    # DEPOIS DO CONSERTO DA FRENTE G2 (defeito a, ver docs/AUTONUMBER.md Parte
    # C): a perda de linha na mesma faixa CONTINUA (o conserto pleno e faixa por
    # no, que muda formato), mas deixou de ser calada -- `replicacao_estado`
    # agora traz `colisoes_de_sequencia` com o contador por tabela. Quem
    # reexecutar deve conferir que esse contador ficou > 0 nas tabelas `p`/`q`.
    pa, pb = PORTA + 12, PORTA + 13
    a = servidor_replicado(pa, "alfa", {
        "papel": "multi", "id_servidor": "alfa", "imagem_da_linha": True,
        "origens": [{"nome": "beta", "host": "127.0.0.1", "porta": pb, "token": "t",
                     "usuario": "adm", "senha_hash": h, "databases": ["loja"],
                     "reconectar_em": 1}]}, h)
    b = servidor_replicado(pb, "beta", {
        "papel": "multi", "id_servidor": "beta", "imagem_da_linha": True,
        "origens": [{"nome": "alfa", "host": "127.0.0.1", "porta": pa, "token": "t",
                     "usuario": "adm", "senha_hash": h, "databases": ["loja"],
                     "reconectar_em": 1}]}, h)
    with a, b:
        for sv in (a, b):
            sv.pedir(op="login", usuario="adm", senha="segredo1")
            sv.pedir(op="criar_database", database="loja")
            tabela_com_sequencia(sv, "loja", "p")
            tabela_com_sequencia(sv, "loja", "q")

        print("  -- estagio 1: os dois numeram sozinhos, na MESMA faixa")
        for i in range(2):
            a.pedir(op="inserir", database="loja", tabela="p", linha={"c": f"de-alfa-{i}"})
            b.pedir(op="inserir", database="loja", tabela="p", linha={"c": f"de-beta-{i}"})
        time.sleep(8)
        la = [l["c"] for l in a.pedir(op="varrer", database="loja", tabela="p", max=50).get("linhas", [])]
        lb = [l["c"] for l in b.pedir(op="varrer", database="loja", tabela="p", max=50).get("linhas", [])]
        print(f"    4 insercoes -> alfa ficou com {len(la)}: {la}")
        print(f"                   beta ficou com {len(lb)}: {lb}")

        print("  -- estagio 2: faixas DISJUNTAS por `ajustar_sequencia` (o remendo de hoje)")
        a.pedir(op="ajustar_sequencia", database="loja", tabela="q", proxima=1)
        b.pedir(op="ajustar_sequencia", database="loja", tabela="q", proxima=1000000)
        for i in range(2):
            a.pedir(op="inserir", database="loja", tabela="q", linha={"c": f"alfa-{i}"})
            b.pedir(op="inserir", database="loja", tabela="q", linha={"c": f"beta-{i}"})
        time.sleep(8)
        qa = tres(a, "loja", "q")
        # LIDO AGORA, e nao la embaixo: guardar a chamada para o fim do bloco
        # publicaria o contador DEPOIS do estagio 3, e a corrida anterior
        # imprimiu 1.000.002 na tela e gravou 1.000.003 no resultados.json.
        cab_apos_a_volta = a.cabecalho("loja", "q")
        print("    alfa q:", qa)
        print("    contador de alfa depois da ida e volta:", cab_apos_a_volta)

        print("  -- estagio 3: a faixa disjunta sobrevive a primeira ida e volta?")
        a.pedir(op="inserir", database="loja", tabela="q", linha={"c": "alfa-2"})
        b.pedir(op="inserir", database="loja", tabela="q", linha={"c": "beta-2"})
        time.sleep(8)
        qa2 = [l["c"] for l in a.pedir(op="varrer", database="loja", tabela="q", max=50).get("linhas", [])]
        qb2 = [l["c"] for l in b.pedir(op="varrer", database="loja", tabela="q", max=50).get("linhas", [])]
        print(f"    6 insercoes -> alfa ficou com {len(qa2)}: {qa2}")
        print(f"                   beta ficou com {len(qb2)}: {qb2}")
        print("  -- estagio 4: a MESMA prova com chave `Uuid` v7, o caminho que o Cassandra escolheu")
        for sv in (a, b):
            sv.pedir(op="criar_tabela", database="loja", tabela="u",
                     colunas=[{"nome": "id", "tipo": "Uuid", "obrigatoria": True},
                              {"nome": "c", "tipo": "Str(30)", "obrigatoria": True}],
                     indices=[{"nome": "pk_u", "colunas": ["id"], "unico": True,
                               "primario": True}])
        # O `Uuid` NAO nasce sozinho como a `Sequence`: quem nao manda nada leva
        # «coluna id e obrigatoria e recebeu NULL». O cliente pede "novo".
        sem_mandar = a.pedir(op="inserir", database="loja", tabela="u", linha={"c": "sem id"})
        for i in range(2):
            a.pedir(op="inserir", database="loja", tabela="u", linha={"id": "novo", "c": f"alfa-{i}"})
            b.pedir(op="inserir", database="loja", tabela="u", linha={"id": "novo", "c": f"beta-{i}"})
        time.sleep(8)
        ua = [l["c"] for l in a.pedir(op="varrer", database="loja", tabela="u", max=50).get("linhas", [])]
        ub = [l["c"] for l in b.pedir(op="varrer", database="loja", tabela="u", max=50).get("linhas", [])]
        diz("inserir sem mandar o Uuid (a Sequence nasceria sozinha)", sem_mandar)
        print(f"    4 insercoes -> alfa ficou com {len(ua)}: {sorted(ua)}")
        print(f"                   beta ficou com {len(ub)}: {sorted(ub)}")
        MEDIDO["chave_uuid_no_bidirecional"] = {
            "insercoes": 4, "sobreviveram_alfa": len(ua), "sobreviveram_beta": len(ub),
            "linhas_alfa": sorted(ua),
            "uuid_nao_nasce_sozinho": sem_mandar.get("erro")}
        MEDIDO["bidirecional"] = {
            "mesma_faixa": {"insercoes": 4, "sobreviveram_alfa": len(la),
                            "sobreviveram_beta": len(lb), "linhas_alfa": la},
            "faixa_disjunta": {"linhas": qa,
                               "contador_de_alfa_apos_a_volta":
                                   cab_apos_a_volta["proxima_sequencia"]},
            "faixa_disjunta_segunda_rodada": {"insercoes": 6,
                                              "sobreviveram_alfa": len(qa2),
                                              "sobreviveram_beta": len(qb2),
                                              "linhas_alfa": qa2}}


def bloco_25():
    print("\n=== 25. O PRECO: o que um contador DURAVEL custaria, e o que o de hoje custa")
    import statistics
    d = f"{BASE}-fsync"
    os.makedirs(d, exist_ok=True)
    p = d + "/contador.bin"
    medidas = {}
    for rot, func in (("fsync", os.fsync), ("fdatasync", os.fdatasync)):
        fd = os.open(p, os.O_CREAT | os.O_WRONLY, 0o600)
        os.write(fd, b"\0" * 128); func(fd)
        am = []
        for _ in range(9):
            t0 = time.perf_counter()
            for i in range(200):
                os.pwrite(fd, i.to_bytes(8, "little") + b"\0" * 120, 0)
                func(fd)
            am.append((time.perf_counter() - t0) / 200 * 1e6)
        os.close(fd)
        medidas[rot] = {"mediana_us": round(statistics.median(am), 1),
                        "min_us": round(min(am), 1), "max_us": round(max(am), 1),
                        "amostras": 9, "por_amostra": 200}
        print(f"  {rot:10}: mediana {medidas[rot]['mediana_us']:7.1f} us  "
              f"min {medidas[rot]['min_us']:7.1f}  max {medidas[rot]['max_us']:7.1f}")
    fd = os.open(p, os.O_WRONLY)
    am = []
    for _ in range(9):
        t0 = time.perf_counter()
        for i in range(2000):
            os.pwrite(fd, i.to_bytes(8, "little") + b"\0" * 120, 0)
        am.append((time.perf_counter() - t0) / 2000 * 1e6)
    os.close(fd)
    medidas["write"] = {"mediana_us": round(statistics.median(am), 2),
                        "min_us": round(min(am), 2), "max_us": round(max(am), 2),
                        "amostras": 9, "por_amostra": 2000}
    print(f"  {'so write':10}: mediana {medidas['write']['mediana_us']:7.2f} us  "
          f"min {medidas['write']['min_us']:7.2f}  max {medidas['write']['max_us']:7.2f}")
    # DUAS razoes, e nao uma: este conteiner e COMPARTILHADO -- na corrida de
    # 07/09/2026 havia tres `rustc` de outra frente ocupando 3,9 de carga --, e
    # a mediana de um `fsync` sobe com a disputa enquanto o `write` nao se mexe.
    # A razao no PISO da faixa e a que fala do disco; a da mediana fala da
    # maquina naquele minuto. Publicar so uma seria escolher o numero que agrada.
    medidas["carga_da_maquina"] = open("/proc/loadavg").read().split()[:3]
    medidas["fsync_sobre_write_mediana"] = round(medidas["fsync"]["mediana_us"]
                                                 / medidas["write"]["mediana_us"], 1)
    medidas["fsync_sobre_write_no_piso"] = round(medidas["fdatasync"]["min_us"]
                                                 / medidas["write"]["min_us"], 1)
    print(f"  carga da maquina na corrida: {' '.join(medidas['carga_da_maquina'])}")
    print(f"  um sincronizar custa {medidas['fsync_sobre_write_no_piso']}x o `write` NO PISO da faixa, "
          f"e {medidas['fsync_sobre_write_mediana']}x na mediana desta corrida")
    shutil.rmtree(d, ignore_errors=True)

    # O custo MARGINAL da coluna `Sequence`: a mesma tabela com e sem ela.
    N = 400
    def corrida(porta, rotulo, com):
        with Servidor(porta, rotulo) as sv:
            sv.pedir(op="criar_database", database="b")
            cols = ([{"nome": "id", "tipo": "Sequence", "obrigatoria": True}] if com
                    else [{"nome": "id", "tipo": "Int8", "obrigatoria": True}])
            cols.append({"nome": "c", "tipo": "Str(20)", "obrigatoria": True})
            sv.pedir(op="criar_tabela", database="b", tabela="t", colunas=cols,
                     indices=[{"nome": "pk", "colunas": ["id"], "unico": True, "primario": True}])
            t0 = time.perf_counter()
            for i in range(N):
                sv.pedir(op="inserir", database="b", tabela="t",
                         linha={"c": "x"} if com else {"id": i + 1, "c": "x"})
            return (time.perf_counter() - t0) / N * 1e6
    # AS DUAS BRACADAS SE ALTERNAM, e isso nao e capricho: a corrida anterior
    # mediu as cinco de uma e depois as cinco da outra, a carga da maquina
    # mudou no meio, e o resultado saiu com as faixas quase sem se cruzar --
    # dizendo que a coluna Sequence e MAIS RAPIDA que gravar o numero a mao,
    # que e absurdo. Alternando, a deriva da maquina cai nas duas igualmente.
    faixas, am = {}, {True: [], False: []}
    for i in range(6):
        for com in (True, False):
            am[com].append(corrida(PORTA + 20 + i * 2 + (0 if com else 1),
                                   f"custo-{com}-{i}", com))
    for com in (True, False):
        rot = "Sequence" if com else "Int8 a mao"
        chave = f"insercao_{'sequence' if com else 'int8'}_us"
        faixas[chave] = (min(am[com]), max(am[com]))
        medidas[chave] = {"mediana_us": round(statistics.median(am[com]), 1),
                          "min_us": round(min(am[com]), 1),
                          "max_us": round(max(am[com]), 1), "n": N, "amostras": 6,
                          "alternadas": True}
        print(f"  {N} insercoes pela rede, coluna {rot:10}: mediana "
              f"{statistics.median(am[com]):7.1f} us  min {min(am[com]):7.1f}  max {max(am[com]):7.1f}")
    # A regra do pedido 155: vencedor SO quando as faixas nao se cruzam.
    (a1, a2), (b1, b2) = faixas["insercao_sequence_us"], faixas["insercao_int8_us"]
    cruzam = not (a2 < b1 or b2 < a1)
    medidas["faixas_se_cruzam"] = cruzam
    print("  as faixas se CRUZAM: o custo marginal da coluna Sequence nao se distingue do ruido"
          if cruzam else "  as faixas NAO se cruzam: ha diferenca medida")
    MEDIDO["preco"] = medidas


# ---------------------------------------------------------------------- main

def main():
    commit = subprocess.run(["git", "rev-parse", "--short", "HEAD"],
                            capture_output=True, text=True).stdout.strip()
    quando = time.strftime("%Y-%m-%d %H:%M:%S UTC", time.gmtime())
    print(f"sequencias -- {quando} -- commit {commit}\n")
    MEDIDO["quando"] = quando
    MEDIDO["medido_em"] = time.strftime("%Y-%m-%d %H:%M", time.gmtime())
    MEDIDO["commit"] = commit
    MEDIDO["binario"] = BIN

    bloco_25()
    with Servidor() as sv:
        sv.pedir(op="criar_database", database="loja")
        parte_i(sv)
        print("\n" + "=" * 72)
        print("PARTE II -- o ciclo de vida dos tres numeros")
        print("=" * 72)
        bloco_9(sv); bloco_10(sv); bloco_11(sv); bloco_12(sv); bloco_13(sv)
        bloco_14(sv); bloco_15(sv); bloco_16(sv); bloco_17(sv)
    bloco_18()
    bloco_19()
    with Servidor(PORTA + 5, "bkp") as sv:
        sv.pedir(op="criar_database", database="loja")
        tabela_com_sequencia(sv, "loja", "tres")
        for c in "ABC":
            sv.pedir(op="inserir", database="loja", tabela="tres", linha={"c": c})
        bloco_20(sv)
        bloco_21(sv)
    h = hash_da_senha("segredo1")
    bloco_22_23(h)
    bloco_24(h)

    saida = os.path.join(AQUI, "resultados.json")
    with open(saida, "w") as f:
        json.dump(MEDIDO, f, indent=1, ensure_ascii=False)
    print(f"\ngravado: {saida}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
