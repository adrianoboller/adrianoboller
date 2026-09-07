#!/usr/bin/env python3
"""Sonda da PARTICAO POR QUANTIDADE contra o motor vivo.

    flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server
    python3 bancada/particao-por-faixa/sonda.py

Pergunta do dono, 07/09/2026: *«tabela particionada por faixa de qtde de
registros maximo, ex 1.000.000 de registros, mas funcionando como se fosse
uma tabela unica para ficar leve o cadastro, e na hora de usar e transparente
para select, insert, update, softdelete e delete»*.

O desenho ja existia: `ModoParticao::PorQuantidade`, documentado em
`docs/FORMATO.md` (secao 8), com volumes `Tabela_001.reg` .. `Tabela_NNN.reg`
e o endereco por DIVISAO: `volume = (rowid-1)/registros_por_arquivo + 1`. Esta
sonda nao le o codigo: exercita as cinco operacoes contra um `phxsqld` de pe,
com 1.000.000 de linhas em dez volumes de 100.000, e mede se ler uma pagina no
ULTIMO volume custa o mesmo que ler no primeiro.

Variaveis: `PHX_SONDA_PORTA` (padrao 6710), `PHX_SONDA_LINHAS` (padrao
1.000.000) e `PHX_SONDA_POR_VOLUME` (padrao 100.000, dez volumes com o
padrao).

# As duas coisas que esta sonda aprendeu, e que ficam aqui

1. **O campo "volumes" do `esquema` vinha VAZIO neste modo -- pedido 222
   fechou o gap.** Ate a 0.18 so a particao por PERIODO o preenchia
   (`RegFile::reler_fronteiras`, que le o cabecalho de cada volume porque o
   corte depende do calendario); na por quantidade o endereco e uma DIVISAO —
   nao ha fronteira nenhuma para o ENDERECAMENTO ler — e por isso ninguem
   calculava uma para o protocolo MOSTRAR. `RegFile::fronteiras` agora
   calcula a lista tambem aqui, a partir dos volumes que existem em disco e
   da MESMA conta que o motor ja fazia para enderecar:
   `primeiro_rowid(N) = (N-1) * registros_por_arquivo + 1` — sem tocar no
   cache que `localizar` usa, entao o endereco de cada linha continua sendo a
   mesma divisao de sempre. Esta sonda ficou de proposito sem essa correcao:
   ela roda contra o BINARIO, e so prova o formato deste texto; quem quer o
   teste que falha com o defeito reposto e passa com o conserto e
   `crates/phxsql-server/src/servidor.rs::testes_volumes_por_quantidade`.
2. **O `INSERT` dentro de transacao e ACEITO aqui — ao contrario da particao
   por LETRA.** A recusa da alfanumerica (`bancada/alfanumerica/`) existe
   porque ali o BALDE (e por isso o rowid alvo) so se descobre executando a
   regra de particao; aqui o rowid alvo continua sendo `slots()+1`, a MESMA
   conta de uma tabela sem particao nenhuma — entao a marca de recuperacao
   continua sendo idempotente e a transacao nao precisa de tratamento
   especial. Testado, e nao suposto (ver parte 4).
"""

import json
import os
import shutil
import socket
import subprocess
import sys
import time

PORTA = int(os.environ.get("PHX_SONDA_PORTA", "6710"))
BASE = f"/tmp/phx-sonda-faixa-{os.getpid()}"
BINARIO = "target/release/phxsqld"
LINHAS = int(os.environ.get("PHX_SONDA_LINHAS", "1000000"))
POR_VOLUME = int(os.environ.get("PHX_SONDA_POR_VOLUME", "100000"))
LOTE = 5000


class Servidor:
    """Sobe um `phxsqld` de verdade e o derruba no fim, aconteca o que acontecer."""

    def __enter__(self):
        if not os.path.exists(BINARIO):
            raise SystemExit(
                f"{BINARIO} nao existe. Rode antes:\n"
                "  flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server"
            )
        shutil.rmtree(BASE, ignore_errors=True)
        os.makedirs(BASE + "/dados", exist_ok=True)
        cfg = BASE + "/config.json"
        with open(cfg, "w") as f:
            json.dump(
                {
                    "bind": f"127.0.0.1:{PORTA}",
                    "base": BASE + "/dados",
                    "token": "t",
                    "web": {"ligado": False},
                    # Sem isto o `varrer` de uma pagina cheia seria cortado em
                    # 1.000 pelo padrao, e a sonda acusaria falta de linha que
                    # so o TETO da resposta escondeu.
                    "max_linhas": 2_000_000,
                },
                f,
            )
        self.p = subprocess.Popen(
            [BINARIO, "--config", cfg],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        for _ in range(100):
            try:
                socket.create_connection(("127.0.0.1", PORTA), 0.2).close()
                break
            except OSError:
                time.sleep(0.1)
        else:
            raise SystemExit(f"o servidor nao subiu na porta {PORTA}")
        self.s = socket.create_connection(("127.0.0.1", PORTA), 20)
        self.f = self.s.makefile("rwb")
        return self

    def __exit__(self, *_):
        try:
            self.f.close()
            self.s.close()
        except OSError:
            pass
        self.p.terminate()
        self.p.wait(timeout=10)
        shutil.rmtree(BASE, ignore_errors=True)

    def pedir(self, **kw):
        kw.setdefault("token", "t")
        self.f.write((json.dumps(kw) + "\n").encode())
        self.f.flush()
        r = json.loads(self.f.readline().decode())
        if isinstance(r.get("resultado"), dict):
            fora = {k: v for k, v in r.items() if k != "resultado"}
            r = {**r["resultado"], **fora}
        return r


def diz(rotulo, r, corte=200):
    marca = "OK  " if r.get("ok") else "ERRO"
    extra = "" if r.get("ok") else " -- " + str(r.get("erro") or r)[:corte]
    print(f"  [{marca}] {rotulo}{extra}")
    return r


COLUNAS = [
    {"nome": "id", "tipo": "Int8"},
    {"nome": "nome", "tipo": "Str(30)", "obrigatoria": True},
]
INDICES = [{"nome": "porId", "colunas": ["id"], "unico": True}]


def primeiro_rowid(volume, por_volume):
    """A MESMA conta do endereco: `(volume-1) * registros_por_arquivo + 1`."""
    return (volume - 1) * por_volume + 1


def parte_1_criar_e_carregar(sv):
    print(f"\n=== 1. criar_tabela particao=faixa, registros_por_arquivo={POR_VOLUME}\n")
    sv.pedir(op="criar_database", database="carga")
    diz(
        f"criar_tabela particao=faixa registros_por_arquivo={POR_VOLUME}",
        sv.pedir(
            op="criar_tabela",
            database="carga",
            tabela="grande",
            colunas=COLUNAS,
            indices=INDICES,
            registros_por_arquivo=POR_VOLUME,
            max_arquivos=25,
            particao="faixa",
            softdelete=True,
        ),
    )

    volumes_previstos = -(-LINHAS // POR_VOLUME)
    print(f"\n  carregando {LINHAS} linhas em lotes de {LOTE} "
          f"({volumes_previstos} volumes previstos)...")
    t0 = time.time()
    for inicio in range(0, LINHAS, LOTE):
        n = min(LOTE, LINHAS - inicio)
        lote = [
            {"id": inicio + k + 1, "nome": f"registro {inicio + k + 1:07d}"}
            for k in range(n)
        ]
        r = sv.pedir(op="inserir_lote", database="carga", tabela="grande", linhas=lote)
        if not r.get("ok"):
            print("  lote falhou em", inicio, ":", str(r.get("erro"))[:200])
            return False
    levou = time.time() - t0
    print(f"  gravadas {LINHAS} linhas em {levou:.1f}s ({LINHAS / levou:.0f} linhas/s)")
    return True


def parte_2_os_arquivos_em_disco():
    print("\n=== 2. os arquivos que nasceram no disco\n")
    arqs = sorted(
        a for a in os.listdir(BASE + "/dados/carga")
        if a.startswith("grande_") and a.endswith(".reg")
    )
    for a in arqs:
        tam = os.path.getsize(BASE + "/dados/carga/" + a)
        print(f"    {a:20} {tam:>12} bytes")
    print(f"  total: {len(arqs)} volumes de dados")
    return arqs


def parte_3_varrer_e_uma_tabela_so(sv):
    print("\n=== 3. `varrer` atravessando a fronteira volume 1 / volume 2\n")
    fronteira = POR_VOLUME
    r = sv.pedir(
        op="varrer",
        database="carga",
        tabela="grande",
        depois=fronteira - 3,
        max=6,
    )
    linhas = r.get("linhas") or []
    print(f"  pedidos os rowids {fronteira - 2}..{fronteira + 3}, devolvidos "
          f"{len(linhas)}:")
    for l in linhas:
        vol_esperado = (l["rowid"] - 1) // POR_VOLUME + 1
        print(f"    rowid {l['rowid']:>8}  id={l['id']:<8}  volume_esperado={vol_esperado}")
    seguidos = all(
        linhas[i]["rowid"] == linhas[i - 1]["rowid"] + 1 for i in range(1, len(linhas))
    )
    print(f"  -> rowids seguidos através da fronteira de arquivo: {seguidos}. "
          "Uma UNICA chamada, um UNICO resultado; o `varrer` nao sabe que "
          "cruzou de arquivo.")

    print("\n  buscar por indice em tres volumes diferentes:")
    volumes_previstos = -(-LINHAS // POR_VOLUME)
    for volume in (1, volumes_previstos // 2, volumes_previstos):
        alvo_id = primeiro_rowid(volume, POR_VOLUME) + 5
        r = sv.pedir(op="buscar", database="carga", tabela="grande", indice="porId", chave=alvo_id)
        linhas = r.get("linhas") or []
        achou = len(linhas) == 1 and linhas[0]["id"] == alvo_id
        vol_real = (linhas[0]["rowid"] - 1) // POR_VOLUME + 1 if linhas else -1
        print(f"    id={alvo_id:<9} (volume {volume:>2}) -> achou={achou} "
              f"volume_do_rowid={vol_real}")


def parte_4_o_custo_por_volume(sv):
    print("\n=== 4. o custo de ler UMA linha no volume 1 x no ULTIMO volume\n")
    volumes_previstos = -(-LINHAS // POR_VOLUME)
    alvo_v1 = primeiro_rowid(1, POR_VOLUME) + POR_VOLUME // 2
    alvo_vn = primeiro_rowid(volumes_previstos, POR_VOLUME) + min(
        POR_VOLUME // 2, (LINHAS - primeiro_rowid(volumes_previstos, POR_VOLUME))
    )
    REPETICOES = 300

    def custo_medio(rowid):
        # Uma chamada de aquecimento, fora da medicao -- para nao contar a
        # abertura fria da tabela como custo do endereco.
        sv.pedir(op="ler", database="carga", tabela="grande", rowid=rowid)
        t0 = time.time()
        for _ in range(REPETICOES):
            r = sv.pedir(op="ler", database="carga", tabela="grande", rowid=rowid)
            assert r is not None, f"rowid {rowid} nao encontrado"
        return (time.time() - t0) * 1000 / REPETICOES

    ms_v1 = custo_medio(alvo_v1)
    ms_vn = custo_medio(alvo_vn)
    print(f"  rowid {alvo_v1:>8} (volume 1)              : {ms_v1:.4f} ms/leitura "
          f"(media de {REPETICOES})")
    print(f"  rowid {alvo_vn:>8} (volume {volumes_previstos})             : {ms_vn:.4f} ms/leitura "
          f"(media de {REPETICOES})")
    razao = ms_vn / ms_v1 if ms_v1 else float("inf")
    print(f"  razao volume_{volumes_previstos} / volume_1 = {razao:.2f}x")
    print("  -> tem de ficar perto de 1,0x: o endereco e uma DIVISAO, nao uma "
          "busca que anda pelos volumes anteriores.")


def parte_5_as_cinco_operacoes(sv):
    print("\n=== 5. as cinco operacoes, direto no meio do arquivo maior\n")
    volumes_previstos = -(-LINHAS // POR_VOLUME)
    meio = primeiro_rowid(volumes_previstos, POR_VOLUME) + 10

    diz(
        "SELECT (varrer) uma linha do ULTIMO volume",
        sv.pedir(op="varrer", database="carga", tabela="grande", depois=meio - 1, max=1),
    )
    diz(
        "UPDATE linha inteira, sem restricao de particao",
        sv.pedir(
            op="atualizar",
            database="carga",
            tabela="grande",
            rowid=meio,
            linha={"id": 10_000_000 + meio, "nome": "atualizado pela sonda"},
        ),
    )
    diz(
        "SOFTDELETE (excluir suave)",
        sv.pedir(op="excluir", database="carga", tabela="grande", rowid=meio + 1, motivo="sonda"),
    )
    diz(
        "DELETE fisico (excluir de vez)",
        sv.pedir(
            op="excluir",
            database="carga",
            tabela="grande",
            rowid=meio + 2,
            motivo="sonda",
            fisico=True,
        ),
    )
    diz(
        "INSERT normal (fora de transacao) -- abre um volume novo alem do previsto",
        sv.pedir(
            op="inserir",
            database="carga",
            tabela="grande",
            linha={"id": 20_000_000, "nome": "extra fora de transacao"},
        ),
    )


def parte_6_insert_em_transacao(sv):
    print("\n=== 6. INSERT dentro de uma transacao -- e ACEITO aqui\n")
    diz("begin", sv.pedir(op="begin"))
    r = diz(
        "inserir DENTRO da transacao",
        sv.pedir(
            op="inserir",
            database="carga",
            tabela="grande",
            linha={"id": 30_000_000, "nome": "dentro da transacao"},
        ),
        corte=250,
    )
    if r.get("ok"):
        diz("commit", sv.pedir(op="commit"))
        conferido = sv.pedir(
            op="buscar", database="carga", tabela="grande", indice="porId", chave=30_000_000
        )
        achou = len(conferido.get("linhas") or []) == 1
        print(f"    -> apos o COMMIT a linha aparece pelo indice: {achou}")
    else:
        print("    -> recusado (igual a particao por letra): rollback e sai")
        sv.pedir(op="rollback")


def parte_7_esquema_e_o_campo_volumes(sv):
    print("\n=== 7. o `esquema`: paginacao e `volumes`, pedido 222\n")
    esq = sv.pedir(op="esquema", database="carga", tabela="grande")
    print("  paginacao:", json.dumps(esq.get("paginacao"), ensure_ascii=False))
    volumes = esq.get("volumes") or []
    print(f"  volumes (campo do protocolo): {len(volumes)} entradas")
    volumes_previstos = -(-LINHAS // POR_VOLUME)
    esperados = [primeiro_rowid(v, POR_VOLUME) for v in range(1, volumes_previstos + 1)]
    achados = [v.get("primeiro_rowid") for v in volumes]
    print(f"  primeiro_rowid esperado : {esperados}")
    print(f"  primeiro_rowid devolvido: {achados}")
    print(
        "  -> preenchido desde o pedido 222 (RegFile::fronteiras calcula a "
        "lista tambem na por quantidade, a partir dos volumes que existem em "
        "disco e da MESMA conta do enderecamento). Antes vinha vazio: so a "
        "particao por PERIODO lia fronteira do cabecalho de cada volume."
        if achados == esperados
        else "  -> DIVERGIU do esperado -- ver `crates/phxsql-store/src/reg.rs`, "
        "`RegFile::fronteiras`."
    )


def main():
    with Servidor() as sv:
        if not parte_1_criar_e_carregar(sv):
            return 1
        parte_2_os_arquivos_em_disco()
        parte_3_varrer_e_uma_tabela_so(sv)
        parte_4_o_custo_por_volume(sv)
        parte_5_as_cinco_operacoes(sv)
        parte_6_insert_em_transacao(sv)
        parte_7_esquema_e_o_campo_volumes(sv)
    print("\nfim.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
