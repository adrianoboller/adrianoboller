#!/usr/bin/env python3
"""Bancada de comparacao PhxSql x MySQL(R), com IO, CPU e memoria medidos.

Como mede
---------
Nao ha estimativa em lugar nenhum. Cada fase e cercada por:

  * tempo de parede             (time.monotonic)
  * CPU do processo             /proc/<pid>/stat utime+stime, em jiffies
  * pico de memoria residente   /proc/<pid>/status VmHWM
  * bytes lidos e escritos      /proc/<pid>/io read_bytes/write_bytes

Do lado do PhxSql, o processo e o da fase -- cada fase roda sozinha, e os
contadores sao dela do inicio ao fim.

Do lado do MySQL(R), o trabalho acontece no mysqld, que ja estava rodando.
Entao os contadores do mysqld sao lidos ANTES e DEPOIS e entram por
diferenca, e o pico de memoria e o VmHWM do daemon na janela. O cliente
(mysql) tem os proprios numeros somados, porque ele tambem gasta CPU.

O que NAO e comparado
---------------------
Durabilidade. O MySQL(R) com innodb_flush_log_at_trx_commit=1 grava o log a
cada transacao; aqui as duas bancadas carregam em massa com uma sincronizacao
por lote. Isso esta dito no relatorio, e nao escondido no numero.
"""

import json
import os
import subprocess
import sys
import time
from pathlib import Path

JIFFY = os.sysconf("SC_CLK_TCK")
BASE = Path(__file__).resolve().parent
# O caminho sai DA ARVORE em que este arquivo esta, e nao de um absoluto
# escrito a mao: com o absoluto, uma bancada rodada numa arvore de trabalho
# media o binario de OUTRA -- que e a armadilha do binario velho num degrau
# mais alto, porque nem recompilar aqui resolveria.
CARGA = os.environ.get(
    "PHX_CARGA",
    str(Path(__file__).resolve().parent.parent / "target/release/examples/carga"),
)
PHX_DIR = BASE / "phxsql"
RESULTADOS = BASE / "resultados.json"
# Enquanto a corrida anda, o progresso vai para ca. O arquivo versionado so e
# tocado no fim, quando a medicao esta inteira.
#
# Nao e capricho de arrumacao: uma corrida de dez milhoes leva vinte minutos, e
# durante esse tempo o `resultados.json` do repositorio ficava com meia medicao
# dentro. Quem olhasse via numero, nao via "faltam quatro fases".
PARCIAL = BASE / "resultados.parcial.json"


def le_io(pid):
    try:
        d = {}
        for linha in Path(f"/proc/{pid}/io").read_text().splitlines():
            k, _, v = linha.partition(":")
            d[k.strip()] = int(v)
        return d
    except (FileNotFoundError, ProcessLookupError, PermissionError):
        return {}


def le_cpu(pid):
    try:
        campos = Path(f"/proc/{pid}/stat").read_text().rsplit(") ", 1)[1].split()
        # utime e o campo 14 do proc(5); depois do ") " ele vira o indice 11.
        return (int(campos[11]) + int(campos[12])) / JIFFY
    except (FileNotFoundError, ProcessLookupError, IndexError):
        return 0.0


def le_pico_rss(pid):
    try:
        for linha in Path(f"/proc/{pid}/status").read_text().splitlines():
            if linha.startswith("VmHWM:"):
                return int(linha.split()[1]) * 1024
    except (FileNotFoundError, ProcessLookupError):
        pass
    return 0


def pid_do_mysqld():
    saida = subprocess.run(["pgrep", "-x", "mysqld"], capture_output=True, text=True)
    linhas = saida.stdout.split()
    return int(linhas[0]) if linhas else None


def zera_pico(pid):
    """Reinicia o VmHWM, para o pico ser o DESTA fase e nao o de antes."""
    try:
        Path(f"/proc/{pid}/clear_refs").write_text("5\n")
        return True
    except (PermissionError, FileNotFoundError, OSError):
        return False


# ----------------------------------------------------------------- PhxSql


def fase_phxsql(fase, n):
    proc = subprocess.Popen(
        [CARGA, str(PHX_DIR), fase, str(n)],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    t0 = time.monotonic()
    pico = 0
    io_fim, cpu_fim = {}, 0.0
    # Amostra enquanto roda: quando o processo morre, /proc some.
    while proc.poll() is None:
        p = le_pico_rss(proc.pid)
        if p:
            pico = max(pico, p)
        i = le_io(proc.pid)
        if i:
            io_fim = i
        c = le_cpu(proc.pid)
        if c:
            cpu_fim = c
        time.sleep(0.05)
    saida = proc.stdout.read()
    segundos = time.monotonic() - t0

    linha = [l for l in saida.splitlines() if l.startswith("RESULTADO ")]
    detalhe = json.loads(linha[-1][len("RESULTADO "):]) if linha else {}
    return {
        "motor": "PhxSql",
        "fase": fase,
        "operacoes": detalhe.get("operacoes", n),
        "segundos": round(segundos, 3),
        "cpu_s": round(cpu_fim, 3),
        "pico_rss_mb": round(pico / 1048576, 1),
        "lido_mb": round(io_fim.get("read_bytes", 0) / 1048576, 1),
        "escrito_mb": round(io_fim.get("write_bytes", 0) / 1048576, 1),
        "saida": saida.strip().splitlines()[-2:],
    }


# ------------------------------------------------------------------ MySQL


def sql(comando, banco="bench"):
    """Manda o comando por ARQUIVO, sempre.

    Uma lista IN com 100.000 identificadores nao cabe na linha de comando --
    o sistema recusa com "Argument list too long". Por arquivo cabe, e o
    caminho e o mesmo para todos os comandos, entao a medicao nao muda de
    forma no meio do experimento."""
    arq = BASE / "comando.sql"
    arq.write_text(comando)
    r = subprocess.run(
        ["mysql", "--protocol=socket", "-N", "-B", banco, "-e", f"SOURCE {arq};"],
        capture_output=True,
        text=True,
    )
    return r


def fase_mysql(fase, n, comando):
    pid = pid_do_mysqld()
    zerou = zera_pico(pid) if pid else False
    io0 = le_io(pid) if pid else {}
    cpu0 = le_cpu(pid) if pid else 0.0

    t0 = time.monotonic()
    r = sql(comando)
    segundos = time.monotonic() - t0

    io1 = le_io(pid) if pid else {}
    cpu1 = le_cpu(pid) if pid else 0.0
    pico = le_pico_rss(pid) if pid else 0

    return {
        "motor": "MySQL",
        "fase": fase,
        "operacoes": n,
        "segundos": round(segundos, 3),
        "cpu_s": round(cpu1 - cpu0, 3),
        "pico_rss_mb": round(pico / 1048576, 1),
        "pico_confiavel": zerou,
        "lido_mb": round((io1.get("read_bytes", 0) - io0.get("read_bytes", 0)) / 1048576, 1),
        "escrito_mb": round(
            (io1.get("write_bytes", 0) - io0.get("write_bytes", 0)) / 1048576, 1
        ),
        "erro": r.stderr.strip()[:200] or None,
        "saida": r.stdout.strip().splitlines()[-2:],
    }


def guardar(resultados):
    """Grava o progresso no arquivo de trabalho, nao no versionado."""
    PARCIAL.write_text(json.dumps(resultados, indent=2, ensure_ascii=False))


def publicar():
    """Promove a medicao completa. So aqui o arquivo do repositorio muda.

    `os.replace` troca de uma vez: ou o `resultados.json` e o antigo inteiro,
    ou o novo inteiro, nunca metade de cada.
    """
    if PARCIAL.exists():
        os.replace(PARCIAL, RESULTADOS)


def main():
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 10_000_000
    lote = 50_000
    resultados = []

    print(f"=== carga de {n:,} registros ===".replace(",", "."), flush=True)

    # ------------------------------------------------------------ preparo
    subprocess.run(["rm", "-rf", str(PHX_DIR)], check=False)
    PHX_DIR.mkdir(parents=True, exist_ok=True)
    subprocess.run([CARGA, str(PHX_DIR), "criar"], capture_output=True)

    sql("DROP DATABASE IF EXISTS bench; CREATE DATABASE bench;", banco="")
    sql(
        """CREATE TABLE precos (
             id BIGINT NOT NULL,
             produto VARCHAR(40) NOT NULL,
             cidade VARCHAR(20),
             valor DECIMAL(15,2),
             cadastro DATE,
             PRIMARY KEY (id),
             KEY porCidade (cidade)
           ) ENGINE=InnoDB"""
    )

    # ----------------------------------------------------------- inserir
    # Os dois carregam em lotes de 50.000, com uma sincronizacao por lote.
    print("inserindo…", flush=True)
    phx_total = {"segundos": 0.0, "cpu_s": 0.0, "lido_mb": 0.0, "escrito_mb": 0.0, "pico_rss_mb": 0.0}
    feitos = 0
    while feitos < n:
        quantos = min(lote, n - feitos)
        r = fase_phxsql("inserir", quantos)
        for k in ("segundos", "cpu_s", "lido_mb", "escrito_mb"):
            phx_total[k] += r[k]
        phx_total["pico_rss_mb"] = max(phx_total["pico_rss_mb"], r["pico_rss_mb"])
        feitos += quantos
        if feitos % 1_000_000 == 0 or feitos == n:
            print(
                f"  PhxSql {feitos:>10,} em {phx_total['segundos']:7.1f}s "
                f"({feitos/max(phx_total['segundos'],1e-9):,.0f}/s)".replace(",", "."),
                flush=True,
            )
            guardar(resultados + [dict(phx_total, motor="PhxSql", fase="inserir", operacoes=feitos)])
    resultados.append(dict(phx_total, motor="PhxSql", fase="inserir", operacoes=n,
                           segundos=round(phx_total["segundos"], 3)))
    guardar(resultados)

    # MySQL: mesmo tamanho de lote, uma transacao por lote.
    gerador = BASE / "gera.sql"
    feitos = 0
    acumulado = {"segundos": 0.0, "cpu_s": 0.0, "lido_mb": 0.0, "escrito_mb": 0.0, "pico_rss_mb": 0.0}
    while feitos < n:
        quantos = min(lote, n - feitos)
        valores = ",".join(
            f"({i},'Produto {i:08}','{['Blumenau','Joinville','Itajai','Curitiba','Chapeco','Lages','Florianopolis','Criciuma'][i%8]}',"
            f"{((i%900000)+100)/100:.2f},'2024-10-04')"
            for i in range(feitos + 1, feitos + quantos + 1)
        )
        gerador.write_text(f"START TRANSACTION;\nINSERT INTO precos VALUES {valores};\nCOMMIT;\n")
        r = fase_mysql("inserir", quantos, gerador.read_text())
        if r.get("erro"):
            print("ERRO MySQL:", r["erro"], flush=True)
            break
        for k in ("segundos", "cpu_s", "lido_mb", "escrito_mb"):
            acumulado[k] += r[k]
        acumulado["pico_rss_mb"] = max(acumulado["pico_rss_mb"], r["pico_rss_mb"])
        feitos += quantos
        if feitos % 1_000_000 == 0 or feitos == n:
            print(
                f"  MySQL  {feitos:>10,} em {acumulado['segundos']:7.1f}s "
                f"({feitos/max(acumulado['segundos'],1e-9):,.0f}/s)".replace(",", "."),
                flush=True,
            )
    resultados.append(dict(acumulado, motor="MySQL", fase="inserir", operacoes=feitos,
                           segundos=round(acumulado["segundos"], 3)))
    guardar(resultados)
    gerador.unlink(missing_ok=True)

    # ------------------------------------------ as outras fases, 20.000 cada
    #
    # UMA INSTRUCAO POR OPERACAO dos dois lados. A primeira versao desta
    # bancada mandava ao MySQL(R) um unico "WHERE id IN (100.000 ids)" e ao
    # PhxSql 100.000 buscas separadas -- e comparava os dois tempos. Nao era
    # comparacao: era uma consulta em lote contra vinte mil consultas. O
    # numero saia 41x a favor do MySQL(R) por causa da FORMA da pergunta, nao
    # do motor.
    #
    # Agora os dois recebem vinte mil instrucoes independentes. E mais lento
    # dos dois lados, e e a unica forma de o numero querer dizer alguma coisa.
    OPS = 20_000
    alvos = [(k * 7919) % n + 1 for k in range(OPS)]

    fases = [
        ("buscar", "".join(f"SELECT id FROM precos WHERE id={a};\n" for a in alvos)),
        # A varredura por faixa e naturalmente uma consulta so dos dois lados.
        ("varrer", "SELECT COUNT(*), SUM(valor) FROM precos WHERE cidade='Blumenau';"),
        ("atualizar",
         "START TRANSACTION;\n"
         + "".join(f"UPDATE precos SET valor=9999.00 WHERE id={a};\n" for a in alvos)
         + "COMMIT;\n"),
        ("excluir",
         "START TRANSACTION;\n"
         + "".join(f"DELETE FROM precos WHERE id={a};\n" for a in alvos)
         + "COMMIT;\n"),
    ]
    for fase, comando in fases:
        print(f"{fase}…", flush=True)
        resultados.append(fase_phxsql(fase, OPS))
        guardar(resultados)
        resultados.append(fase_mysql(fase, OPS, comando))
        guardar(resultados)
        print(f"  PhxSql {resultados[-2]['segundos']:7.2f}s  |  "
              f"MySQL {resultados[-1]['segundos']:7.2f}s", flush=True)

    # ------------------------------------------------------- tamanho em disco
    du = lambda p: int(subprocess.run(["du", "-sb", p], capture_output=True, text=True)
                       .stdout.split()[0] or 0)
    resultados.append({
        "motor": "PhxSql", "fase": "disco",
        "bytes": du(str(PHX_DIR)),
    })
    resultados.append({
        "motor": "MySQL", "fase": "disco",
        "bytes": du("/var/lib/mysql/bench"),
    })
    guardar(resultados)
    publicar()
    print("=== fim ===", flush=True)


if __name__ == "__main__":
    main()
