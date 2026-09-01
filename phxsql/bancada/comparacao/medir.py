#!/usr/bin/env python3
"""Os tres motores no mesmo trabalho: PhxSql x MySQL(R) x SQLite(R).

    python3 bancada/comparacao/medir.py                    # 1.000.000, 3 rodadas
    python3 bancada/comparacao/medir.py 200000 --rodadas 5
    python3 bancada/comparacao/medir.py --so phxsql,sqlite # sem o MySQL(R)

Escreve `um-milhao.json`, que e de onde `grafico.py` desenha. Enquanto a
corrida anda o progresso vai para `um-milhao.parcial.json`, que nao e
versionado: uma corrida leva dezenas de minutos, e durante esse tempo o
arquivo versionado ficaria com meia medicao dentro -- quem olhasse via
numero, nao via «faltam duas fases».

Por que uma bancada nova, em vez de juntar as duas que existem
--------------------------------------------------------------
`bancada/medir.py` mede PhxSql x MySQL(R) e `bancada/sqlite/medir.py` mede
PhxSql x SQLite(R). Somar as duas tabelas daria um numero de tres motores
sem que os tres tivessem feito o mesmo trabalho no mesmo dia, na mesma
maquina, com a mesma carga em volta -- e comparar medidas de dias diferentes
e como comparar escalas diferentes: parte da diferenca vem do ambiente, nao
do motor. Aqui os tres correm intercalados, na mesma rodada.

E ao montar esta juntar apareceu um defeito na bancada do MySQL(R): ela grava
`'2024-10-04'` em TODA linha, enquanto PhxSql e SQLite(R) gravam
`20000 + (i % 400)` -- dado diferente do mesmo tamanho, que e violacao da
regra 1 e nao aparece em tempo nenhum. Aqui os tres recebem o dia variavel, e
a fase `conferir` compara a SOMA de `cadastro` justamente para que a proxima
divergencia dessa familia reprove em vez de publicar.

As quatro regras da casa, aplicadas a este trio
-----------------------------------------------
1. **Mesmos dados.** O `linha(i)` daqui e a traducao literal do `linha(i)` do
   `carga.rs`, inclusive a data. Sem sorteio.
2. **Mesmo esquema.** Chave em `id` e indice secundario em `cidade` nos tres.
   O SQLite(R) nao tem traducao unica para isso -- rodam as DUAS variantes, e
   a publicada e a `rowid`, que e a que casa com o InnoDB (chave primaria
   agrupada) e a que FAVORECE o SQLite(R). A outra fica no JSON.
3. **Mesma forma de pergunta.** Uma instrucao por operacao nas fases
   pontuais, nos tres. A excecao e a carga inicial, e ela esta nas ressalvas
   com o nome de quem favorece.
4. **Mesma quantidade de trabalho.** E a fase `conferir` prova: os tres tem
   de sair de cada etapa com a MESMA contagem, a MESMA soma de `valor` e a
   MESMA soma de `cadastro`, por tres codigos sem uma linha em comum.
   Divergiu, a bancada RECUSA publicar.

O piso do MySQL(R), e por que ele e medido
------------------------------------------
Os tres nao tem a mesma forma: o SQLite(R) e biblioteca em processo, o PhxSql
aqui e biblioteca tambem (o `carga`), e o MySQL(R) e daemon que recebe TEXTO
por soquete. A barra do MySQL(R) carrega transporte e analise de texto que as
outras duas nao pagam, e nao ha como tirar isso -- nao existe MySQL(R)
embutido nesta maquina.

O que da para fazer e MEDIR quanto disso e piso: 20.000 instrucoes que nao
fazem trabalho nenhum (`DO 1;`) pelo mesmo caminho. O numero vai para as
ressalvas, e o leitor subtrai. Esconder isso seria publicar uma vitoria que e
do formato, e essa e a familia de erro que esta casa ja cometeu tres vezes.
"""

import json
import os
import shutil
import sqlite3
import statistics
import subprocess
import sys
import time
from datetime import date, timedelta
from pathlib import Path

AQUI = Path(__file__).resolve().parent
RAIZ = AQUI.parent.parent
# O caminho sai DA ARVORE em que este arquivo esta: com um absoluto escrito a
# mao, uma bancada rodada numa arvore de trabalho mede o binario de OUTRA.
CARGA = Path(os.environ.get("PHX_CARGA", RAIZ / "target/release/examples/carga"))
# O PID entra no nome porque o zelador guarda o que tem PID vivo nele. Sem
# isso, uma fase longa do MySQL(R) deixa a base do PhxSql parada por mais de
# meia hora e o zelador a apaga no meio da corrida.
TRABALHO = Path(os.environ.get("PHX_TRABALHO", f"/tmp/phx-comparacao-{os.getpid()}"))
RESULTADOS = AQUI / "um-milhao.json"
PARCIAL = AQUI / "um-milhao.parcial.json"
COMANDO = AQUI / "comando.sql"

BANCO = "trio"
LOTE = 50_000
PASSO = 7_919  # o mesmo primo do `carga.rs`: espalha os alvos pela tabela

CIDADES = [
    "Blumenau", "Joinville", "Itajai", "Curitiba",
    "Chapeco", "Lages", "Florianopolis", "Criciuma",
]
EPOCA = date(1970, 1, 1)  # o `Date` do PhxSql sao dias desde aqui

FASES = ["inserir", "buscar", "atualizar", "excluir"]


def linha(i):
    """Traducao literal de `linha(i)` do `carga.rs`. Mesmos dados, sem sorteio."""
    return (
        i,
        "Produto %08d" % i,
        CIDADES[i % len(CIDADES)],
        (i % 900_000) + 100,   # o Decimal(15,2) do PhxSql guarda CENTAVOS
        20_000 + (i % 400),    # o Date do PhxSql guarda o NUMERO DO DIA
    )


def mil(x):
    """Ponto de milhar, sem estragar as virgulas da frase em volta."""
    return f"{x:,}".replace(",", ".")


def alvo(k, n):
    return (k * PASSO) % n + 1


def resumo(amostras):
    """Mediana com o menor e o maior ao lado. Nunca um numero so."""
    return {
        "mediana_s": round(statistics.median(amostras), 4),
        "min_s": round(min(amostras), 4),
        "max_s": round(max(amostras), 4),
        "amostras": [round(x, 4) for x in amostras],
    }


# ------------------------------------------------------------------- PhxSql


def roda_carga(dir_dados, fase, n, ambiente=None):
    """Uma fase do `carga`, em processo separado.

    O tempo que volta e o que o PROPRIO `carga` cronometrou, e nao o relogio
    de fora: subir o processo custa alguns milissegundos e uma fase de 20.000
    buscas leva poucos milissegundos. Medida por fora, a fase apareceria com o
    dobro do custo -- e o SQLite(R), que e chamada de funcao no mesmo
    processo, nao pagaria nada disso.
    """
    env = dict(os.environ)
    if ambiente:
        env.update(ambiente)
    p = subprocess.Popen(
        [str(CARGA), str(dir_dados), fase, str(n)],
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, env=env,
    )
    saida = p.communicate()[0].decode()
    if p.returncode != 0:
        raise SystemExit(f"carga {fase} falhou:\n{saida}")
    marca = saida.rsplit("RESULTADO ", 1)
    segundos = json.loads(marca[1])["segundos"] if len(marca) > 1 else 0.0
    return segundos, saida


def confere_phxsql(dir_dados):
    _, saida = roda_carga(dir_dados, "conferir", 0)
    bruto = saida.rsplit("CONFERE ", 1)[1].splitlines()[0]
    d = json.loads(bruto)
    return (d["linhas"], d["soma_valor"], d["soma_cadastro"])


def corre_phxsql(n, ops, tempos):
    d = TRABALHO / "phxsql"
    shutil.rmtree(d, ignore_errors=True)
    roda_carga(d, "criar", 0)

    seg, _ = roda_carga(d, "inserir", n)
    tempos["inserir"].append(seg)
    marcos = [confere_phxsql(d)]

    seg, saida = roda_carga(d, "buscar", ops)
    tempos["buscar"].append(seg)
    achados = int(saida.split("achados:")[1].split()[0])

    seg, _ = roda_carga(d, "atualizar", ops)
    tempos["atualizar"].append(seg)
    marcos.append(confere_phxsql(d))

    # A exclusao entra na janela: do outro lado as 20.000 vao dentro de UMA
    # transacao -- um `fsync` para as vinte mil. Sem isto o PhxSql pagaria
    # vinte mil, e o numero mentiria contra nos.
    seg, _ = roda_carga(d, "excluir", ops, {"PHX_EXCLUSAO_NA_JANELA": "1"})
    tempos["excluir"].append(seg)
    marcos.append(confere_phxsql(d))

    return {"marcos": marcos, "achados": achados, "disco": tamanho(d)}


def tamanho(caminho):
    p = Path(caminho)
    if p.is_file():
        return p.stat().st_size
    return sum(f.stat().st_size for f in p.rglob("*") if f.is_file())


# ------------------------------------------------------------------ SQLite


INSERE = "INSERT INTO precos (id, produto, cidade, valor, cadastro) VALUES (?,?,?,?,?)"
# O `atualizar` do `carga.rs` regrava a LINHA INTEIRA, entao o UPDATE tambem:
# trocar so `valor` seria menos trabalho de um lado.
ATUALIZA = "UPDATE precos SET produto=?, cidade=?, valor=?, cadastro=? WHERE id=?"


def abre_sqlite(caminho, variante):
    for extra in ("", "-journal", "-wal", "-shm"):
        Path(str(caminho) + extra).unlink(missing_ok=True)
    con = sqlite3.connect(caminho, isolation_level=None)
    con.execute("PRAGMA journal_mode=DELETE")  # o padrao; WAL seria outro compromisso
    con.execute("PRAGMA synchronous=FULL")
    if variante == "rowid":
        con.execute(
            "CREATE TABLE precos (id INTEGER PRIMARY KEY, produto TEXT NOT NULL,"
            " cidade TEXT, valor INTEGER, cadastro INTEGER)"
        )
    else:
        con.execute(
            "CREATE TABLE precos (id INTEGER NOT NULL, produto TEXT NOT NULL,"
            " cidade TEXT, valor INTEGER, cadastro INTEGER)"
        )
        con.execute("CREATE UNIQUE INDEX porId ON precos(id)")
    con.execute("CREATE INDEX porCidade ON precos(cidade)")
    return con


def confere_sqlite(con):
    n, v, c = con.execute(
        "SELECT count(*), coalesce(sum(valor),0), coalesce(sum(cadastro),0) FROM precos"
    ).fetchone()
    return (n, v, c)


def corre_sqlite(n, ops, tempos, variante):
    arq = TRABALHO / f"sqlite-{variante}.db"
    con = abre_sqlite(arq, variante)

    t0 = time.monotonic()
    con.execute("BEGIN")
    con.executemany(INSERE, (linha(i) for i in range(1, n + 1)))
    con.execute("COMMIT")
    tempos["inserir"].append(time.monotonic() - t0)
    marcos = [confere_sqlite(con)]
    disco = tamanho(arq)

    cur = con.cursor()
    achados = 0
    t0 = time.monotonic()
    for k in range(ops):
        cur.execute("SELECT * FROM precos WHERE id=?", (alvo(k, n),))
        if cur.fetchone() is not None:
            achados += 1
    tempos["buscar"].append(time.monotonic() - t0)

    t0 = time.monotonic()
    con.execute("BEGIN")
    for k in range(ops):
        a = alvo(k, n)
        _, produto, cidade, _, dia = linha(a)
        con.execute(ATUALIZA, (produto, cidade, 999_900, dia, a))
    con.execute("COMMIT")
    tempos["atualizar"].append(time.monotonic() - t0)
    marcos.append(confere_sqlite(con))

    t0 = time.monotonic()
    con.execute("BEGIN")
    for k in range(ops):
        con.execute("DELETE FROM precos WHERE id=?", (alvo(k, n),))
    con.execute("COMMIT")
    tempos["excluir"].append(time.monotonic() - t0)
    marcos.append(confere_sqlite(con))
    con.close()

    return {"marcos": marcos, "achados": achados, "disco": disco}


# ------------------------------------------------------------------- MySQL


def sql(comando, banco=BANCO):
    """Manda o comando por ARQUIVO, sempre.

    Uma carga de um milhao de linhas nao cabe na linha de comando -- o sistema
    recusa com «Argument list too long». Por arquivo cabe, e o caminho e o
    mesmo para todo comando, entao a medicao nao muda de forma no meio.
    """
    COMANDO.write_text(comando)
    return subprocess.run(
        ["mysql", "--protocol=socket", "-N", "-B"] + ([banco] if banco else [])
        + ["-e", f"SOURCE {COMANDO};"],
        capture_output=True, text=True,
    )


def cronometra_mysql(comando):
    """O arquivo e escrito ANTES do relogio: gravar 70 MB de texto e trabalho
    do medidor, nao do motor, e entrar na conta seria cobrar do MySQL(R) uma
    coisa que ele nao fez."""
    COMANDO.write_text(comando)
    t0 = time.monotonic()
    r = subprocess.run(
        ["mysql", "--protocol=socket", "-N", "-B", BANCO, "-e", f"SOURCE {COMANDO};"],
        capture_output=True, text=True,
    )
    seg = time.monotonic() - t0
    if r.returncode != 0:
        raise SystemExit(f"mysql falhou:\n{r.stderr[:600]}")
    return seg, r.stdout


def data_sql(dia):
    return (EPOCA + timedelta(days=dia)).isoformat()


def confere_mysql():
    # `TO_DAYS(x) - TO_DAYS('1970-01-01')` desfaz a conversao da data e devolve
    # o numero do dia -- a mesma grandeza que os outros dois somam.
    r = sql(
        "SELECT count(*), coalesce(sum(valor),0),"
        " coalesce(sum(TO_DAYS(cadastro) - TO_DAYS('1970-01-01')),0) FROM precos;"
    )
    campos = r.stdout.split()
    return (int(campos[0]), int(float(campos[1])), int(float(campos[2])))


def corre_mysql(n, ops, tempos):
    sql(f"DROP DATABASE IF EXISTS {BANCO}; CREATE DATABASE {BANCO};", banco="")
    sql(
        """CREATE TABLE precos (
             id BIGINT NOT NULL,
             produto VARCHAR(40) NOT NULL,
             cidade VARCHAR(20),
             valor BIGINT,
             cadastro DATE,
             PRIMARY KEY (id),
             KEY porCidade (cidade)
           ) ENGINE=InnoDB"""
    )

    # UMA transacao para a carga inteira, como os outros dois: uma
    # sincronizacao no fim da fase, e nao uma por lote. Os lotes de 50.000 sao
    # so o tamanho da instrucao -- o `max_allowed_packet` nao aceita um
    # `INSERT` de um milhao de linhas.
    partes = ["START TRANSACTION;\n"]
    for base in range(0, n, LOTE):
        quantos = min(LOTE, n - base)
        valores = ",".join(
            "(%d,'%s','%s',%d,'%s')" % (i, p, c, v, data_sql(d))
            for i, p, c, v, d in (linha(x) for x in range(base + 1, base + quantos + 1))
        )
        partes.append(f"INSERT INTO precos VALUES {valores};\n")
    partes.append("COMMIT;\n")
    seg, _ = cronometra_mysql("".join(partes))
    tempos["inserir"].append(seg)
    marcos = [confere_mysql()]

    seg, saida = cronometra_mysql(
        "".join(f"SELECT id FROM precos WHERE id={alvo(k, n)};\n" for k in range(ops))
    )
    tempos["buscar"].append(seg)
    achados = len(saida.split())

    seg, _ = cronometra_mysql(
        "START TRANSACTION;\n"
        + "".join(
            "UPDATE precos SET produto='%s', cidade='%s', valor=999900, cadastro='%s'"
            " WHERE id=%d;\n" % (p, c, data_sql(d), i)
            for i, p, c, _, d in (linha(alvo(k, n)) for k in range(ops))
        )
        + "COMMIT;\n"
    )
    tempos["atualizar"].append(seg)
    marcos.append(confere_mysql())

    seg, _ = cronometra_mysql(
        "START TRANSACTION;\n"
        + "".join(f"DELETE FROM precos WHERE id={alvo(k, n)};\n" for k in range(ops))
        + "COMMIT;\n"
    )
    tempos["excluir"].append(seg)
    marcos.append(confere_mysql())

    return {"marcos": marcos, "achados": achados, "disco": disco_mysql()}


def piso_mysql(ops):
    """Quanto custa mandar 20.000 instrucoes que nao fazem nada.

    E o transporte mais a analise do texto -- o que a barra do MySQL(R)
    carrega e as outras duas nao. Sem este numero o leitor nao tem como
    separar o motor do formato.
    """
    seg, _ = cronometra_mysql("".join("DO 1;\n" for _ in range(ops)))
    return seg


def disco_mysql():
    r = subprocess.run(
        ["du", "-sb", f"/var/lib/mysql/{BANCO}"], capture_output=True, text=True
    )
    try:
        return int(r.stdout.split()[0])
    except (IndexError, ValueError):
        return 0


def ajuste_do_mysql():
    """O regime de durabilidade sai do servidor, e nao de uma frase escrita a
    mao aqui: campo de configuracao citado de memoria e campo que envelhece."""
    r = sql(
        "SELECT @@innodb_flush_log_at_trx_commit, @@sync_binlog, @@version;", banco=""
    )
    campos = r.stdout.split()
    if len(campos) < 3:
        return {}
    return {"flush_log_at_trx_commit": campos[0], "sync_binlog": campos[1],
            "versao": campos[2]}


# -------------------------------------------------------------------- corrida


def confere_trio(marcos, achados, etapas):
    """A prova de trabalho igual. Divergiu, a bancada RECUSA publicar.

    Tres codigos sem uma linha em comum tem de chegar a mesma contagem, a
    mesma soma de `valor` e a mesma soma de `cadastro` em cada etapa. Um
    numero de tempo so quer dizer alguma coisa depois disto.
    """
    motores = list(marcos)
    problemas = []
    for i, etapa in enumerate(etapas):
        vistos = {m: marcos[m][i] for m in motores}
        if len(set(vistos.values())) > 1:
            problemas.append(f"  apos {etapa}: " + ", ".join(
                f"{m}=({mil(q)} linhas, valor {mil(v)}, cadastro {mil(c)})"
                for m, (q, v, c) in vistos.items()
            ))
    if len(set(achados.values())) > 1:
        problemas.append("  buscar achou: " + ", ".join(
            f"{m}={q}" for m, q in achados.items()
        ))
    if problemas:
        raise SystemExit(
            "os motores NAO fizeram o mesmo trabalho:\n"
            + "\n".join(problemas)
            + "\nA bancada nao publica numero de trabalho desigual."
        )


def guardar(d):
    PARCIAL.write_text(json.dumps(d, indent=2, ensure_ascii=False), encoding="utf-8")


def fmt_rodada(t):
    return "  ".join(f"{f} {t[f][-1]:7.3f}s" for f in FASES if t[f])


def durabilidade(ajuste):
    """O regime de cada motor, em texto de tela -- montado do que o servidor
    respondeu, e nao de uma frase digitada aqui."""
    return {
        "PhxSql": "sincroniza uma vez por fase (exclusão na janela)",
        "MySQL(R)": "uma transação por fase; "
                    + ", ".join(f"{k}={v}" for k, v in ajuste.items()),
        "SQLite(R)": "synchronous=FULL, journal DELETE, uma transação por fase",
    }


def ressalvas(n, ops, rodadas, piso):
    """O que estes numeros nao dizem, montado a partir do que foi medido.

    Fica em funcao propria, e nao dentro do `monta`, porque este texto APARECE
    NA PAGINA: e texto de interface, leva acento, e um dia alguem vai querer
    melhorar a redacao. Se a unica forma de reescrever fosse remedir, uma
    virgula custaria quinze minutos de bancada -- e o que se faria em vez
    disso e editar o JSON a mao, que e como um numero gerado vira digitado.
    """
    piso_txt = (
        f"{statistics.median(piso):.3f} s para {mil(ops)} instruções que não "
        "fazem nada (`DO 1;`), que é o que há para subtrair da barra dele nas "
        "fases pontuais." if piso else "não medido nesta corrida."
    )
    return [
        f"A carga inicial não tem a mesma FORMA nos três: o PhxSql faz {mil(n)} "
        "chamadas de função, o SQLite(R) executa a mesma instrução preparada "
        f"{mil(n)} vezes, e o MySQL(R) recebe {(n + LOTE - 1) // LOTE} instruções "
        f"de {mil(LOTE)} linhas. A forma do MySQL(R) é a mais barata das três por "
        "linha, então a barra dele nesta fase é OTIMISTA. As fases pontuais são "
        "uma instrução por operação nos três.",
        "O MySQL(R) é o único que recebe o trabalho como TEXTO por soquete — não "
        "existe MySQL(R) embutido nesta máquina, e os outros dois são biblioteca "
        "no próprio processo. O piso desse formato foi medido: " + piso_txt,
        "O SQLite(R) publicado é a variante `rowid` (`id INTEGER PRIMARY KEY`), "
        "que é a que casa com o InnoDB por ter a chave agrupada e a que FAVORECE "
        "o SQLite(R) — são duas estruturas contra as três da variante `2ind`. A "
        "outra corre na mesma rodada e está no JSON, em `sqlite_2ind`.",
        "Durabilidade casada: uma sincronização no fim de cada fase nos três. Não "
        "é o regime de quem grava pedido a pedido — uma bancada com `commit` por "
        "linha daria outros números, e é a que importa para esse caso.",
        "Uma máquina só, com o que mais estivesse rodando nela. O bigode de mínimo "
        f"a máximo das {rodadas} rodadas é a medida dessa inquietude: barra lisa "
        "afirmaria uma precisão que o número não tem.",
    ]


def monta(n, ops, rodadas, tempos, marcos, achados, disco, piso, quais):
    fases = {}
    for f in FASES:
        fases[f] = {}
        for m in ("phxsql", "mysql", "sqlite"):
            fases[f][m] = resumo(tempos[m][f]) if tempos[m][f] else {
                "mediana_s": None, "min_s": None, "max_s": None
            }

    ress = ressalvas(n, ops, rodadas, piso)

    return {
        "linhas": n,
        "operacoes_por_fase_pontual": ops,
        "rodadas": rodadas,
        "medido_em": time.strftime("%Y-%m-%d %H:%M:%S"),
        "carga_da_maquina": os.getloadavg(),
        "motores_medidos": quais,
        "fases": fases,
        "sqlite_2ind": {f: (resumo(tempos["sqlite-2ind"][f])
                            if tempos["sqlite-2ind"][f] else None) for f in FASES},
        "piso_do_mysql_s": resumo(piso) if piso else None,
        "trabalho_conferido": {
            "etapas": ["inserir", "atualizar", "excluir"],
            "marcos_por_motor": {m: [list(x) for x in v] for m, v in marcos.items()},
            "buscar_achou": achados,
        },
        "disco_bytes": disco,
        "ajuste_do_mysql": ajuste_do_mysql(),
        "durabilidade": durabilidade(ajuste_do_mysql()),
        "ressalvas": ress,
    }


def refazer_prosa():
    """Reescreve as ressalvas do JSON a partir dos numeros que ja estao nele.

    Nao remede nada e nao toca em numero nenhum -- so na prosa que sai deles.
    """
    d = json.loads(RESULTADOS.read_text(encoding="utf-8"))
    piso = (d.get("piso_do_mysql_s") or {}).get("amostras") or []
    d["ressalvas"] = ressalvas(
        d["linhas"], d["operacoes_por_fase_pontual"], d["rodadas"], piso
    )
    # O ajuste do servidor e DADO medido: se ele nao esta guardado, pergunta-se
    # de novo -- e se o servidor nao responder, a durabilidade fica como esta,
    # em vez de virar uma frase sem os numeros dentro.
    ajuste = d.get("ajuste_do_mysql") or ajuste_do_mysql()
    if ajuste:
        d["ajuste_do_mysql"] = ajuste
        d["durabilidade"] = durabilidade(ajuste)
    PARCIAL.write_text(json.dumps(d, indent=2, ensure_ascii=False), encoding="utf-8")
    os.replace(PARCIAL, RESULTADOS)
    print(f"prosa refeita em {RESULTADOS.name}; os numeros nao foram tocados")


def principal():
    argv = sys.argv[1:]
    if "--so-prosa" in argv:
        return refazer_prosa()
    n = 1_000_000
    if argv and argv[0].isdigit():
        n, argv = int(argv[0]), argv[1:]
    rodadas = int(argv[argv.index("--rodadas") + 1]) if "--rodadas" in argv else 3
    ops = int(argv[argv.index("--ops") + 1]) if "--ops" in argv else 20_000
    quais = (argv[argv.index("--so") + 1].split(",") if "--so" in argv
             else ["phxsql", "sqlite", "mysql"])

    if not CARGA.exists():
        raise SystemExit(
            f"nao achei {CARGA}.\n"
            "Rode `cargo build --release --examples -p phxsql-store` antes: "
            "binario velho mede o passado."
        )
    TRABALHO.mkdir(parents=True, exist_ok=True)

    print(f"=== {mil(n)} linhas, {mil(ops)} operacoes, {rodadas} rodadas ===",
          flush=True)
    print(f"    motores: {', '.join(quais)}", flush=True)

    tempos = {m: {f: [] for f in FASES} for m in ("phxsql", "sqlite", "mysql")}
    tempos["sqlite-2ind"] = {f: [] for f in FASES}
    marcos, achados, disco, piso = {}, {}, {}, []

    for r in range(rodadas):
        print(f"-- rodada {r + 1}/{rodadas}", flush=True)
        if "phxsql" in quais:
            saiu = corre_phxsql(n, ops, tempos["phxsql"])
            marcos["phxsql"], achados["phxsql"] = saiu["marcos"], saiu["achados"]
            disco["phxsql"] = saiu["disco"]
            print("   phxsql  " + fmt_rodada(tempos["phxsql"]), flush=True)
        if "sqlite" in quais:
            saiu = corre_sqlite(n, ops, tempos["sqlite"], "rowid")
            marcos["sqlite"], achados["sqlite"] = saiu["marcos"], saiu["achados"]
            disco["sqlite"] = saiu["disco"]
            print("   sqlite  " + fmt_rodada(tempos["sqlite"]), flush=True)
            corre_sqlite(n, ops, tempos["sqlite-2ind"], "2ind")
        if "mysql" in quais:
            saiu = corre_mysql(n, ops, tempos["mysql"])
            marcos["mysql"], achados["mysql"] = saiu["marcos"], saiu["achados"]
            disco["mysql"] = saiu["disco"]
            piso.append(piso_mysql(ops))
            print("   mysql   " + fmt_rodada(tempos["mysql"]), flush=True)
        guardar({"parcial": True, "rodadas_feitas": r + 1, "tempos": tempos})

    confere_trio(marcos, achados, ["inserir", "atualizar", "excluir"])
    print(f"== trabalho igual conferido: {marcos[quais[0]]}", flush=True)

    d = monta(n, ops, rodadas, tempos, marcos, achados, disco, piso, quais)
    guardar(d)
    os.replace(PARCIAL, RESULTADOS)
    COMANDO.unlink(missing_ok=True)
    shutil.rmtree(TRABALHO, ignore_errors=True)
    print(f"== {RESULTADOS.name} publicado", flush=True)


if __name__ == "__main__":
    principal()
