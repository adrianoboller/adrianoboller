#!/usr/bin/env python3
"""Backup AGENDADO, pelo SOQUETE: cadastra, espera o relogio rodar sozinho,
confere o historico e RESTAURA o que saiu -- porque backup nao conferido nao
e backup.

    python3 bancada/jobs/backup-agendado.py

Pergunta do dono, 07/09/2026: *«teste de backups agendados e listagem log se
deu tudo certo»*. `bancada/jobs/prova-avisos.py` ja prova o AVISO por e-mail
de jobs (falha, parado, os cortes do opt-in); esta sonda prova o outro lado --
o job de BACKUP rodando pela AGENDA de verdade (nao por `job_rodar` manual), o
`backup.json` que sai no disco, o HISTORICO que a op `jobs` devolve, um job
que FALHA de proposito, e a RESTAURACAO conferida (`docs/RESTAURACAO.md`).

# A armadilha que esta sonda evita, e por que o arranque e em DOIS tempos

`docs/JOBS.md` e claro: *"ligar o primeiro job pela tela nao acorda ninguem
ate o proximo arranque"* -- o relogio dos jobs so sobe se algum job ja estava
ligado NO ARRANQUE. Isso cria uma corrida: se o job de backup nascesse ligado
no MESMO arranque em que o database ainda nao existe, a primeira volta do
relogio (que acontece quase na hora, `ultimo_ms == 0`) faria um backup vazio
--- correto, mas sem nada para restaurar depois.

O jeito de nao correr essa corrida nao e encurtar o relogio (proibido -- veja
`docs/JOBS.md`, "encurtar o relogio para o teste seria provar outro relogio):
e inverter a ORDEM. Este script sobe o `phxsqld` DUAS vezes com a MESMA raiz
de dados: a primeira, sem job nenhum, so para gravar o database `loja` com
linhas de verdade; a segunda, com `jobs.json` ja escrito e o job de backup
LIGADO -- e so nesse segundo arranque o relogio sobe, e a primeira volta dele
ja acha algo de verdade para copiar.

Mata so os PIDs que subiu. Nunca `pkill -f`.
"""
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
BASE = f"/tmp/phx-jobs-backup-agendado-{os.getpid()}"
PORTA = int(os.environ.get("PHX_SONDA_PORTA", "6725"))
TOKEN = "backupjob"

PIDS = []
FALHAS = []


def ok(nome, condicao, detalhe=""):
    marca = "OK   " if condicao else "FALHA"
    print(f"  {marca} {nome}" + (f"  -- {detalhe}" if detalhe else ""))
    if not condicao:
        FALHAS.append(nome)


class Ligacao:
    def __init__(self, porta=PORTA):
        self.s = socket.create_connection(("127.0.0.1", porta), 10)
        self.s.settimeout(15)
        self.f = self.s.makefile("rwb")

    def fala_bruta(self, p):
        """A resposta CRUA, sem achatar `resultado` -- necessaria quando o
        `ok` de dentro tem um significado DIFERENTE do `ok` de fora (e o caso
        de `job_rodar`: o pedido de rodar pode ter sucesso -- ok=true la fora
        -- enquanto o JOB rodado falha -- ok=false dentro de `resultado`).
        Achatar os dois com o mesmo nome apagaria um dos dois."""
        p.setdefault("token", TOKEN)
        self.f.write((json.dumps(p) + "\n").encode())
        self.f.flush()
        return json.loads(self.f.readline().decode())

    def fala(self, p):
        r = self.fala_bruta(p)
        if isinstance(r.get("resultado"), dict):
            fora = {k: v for k, v in r.items() if k != "resultado"}
            r = {**r["resultado"], **fora}
        return r

    def fechar(self):
        try:
            self.f.close()
        except OSError:
            pass
        try:
            self.s.close()
        except OSError:
            pass


def escrever_config(com_jobs):
    cfg = {
        "bind": f"127.0.0.1:{PORTA}",
        "base": BASE + "/dados",
        "token": TOKEN,
        "web": {"ligado": False},
        "max_linhas": 100000,
    }
    if com_jobs:
        cfg["jobs"] = BASE + "/jobs.json"
    with open(BASE + "/config.json", "w") as f:
        json.dump(cfg, f, indent=2)


def subir(log_nome):
    log = open(BASE + f"/{log_nome}.log", "a")
    p = subprocess.Popen(
        [PHXSQLD, "--config", BASE + "/config.json"],
        stdout=log, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL,
    )
    PIDS.append(p.pid)
    fim = time.time() + 15
    while time.time() < fim:
        try:
            socket.create_connection(("127.0.0.1", PORTA), 0.3).close()
            return p
        except OSError:
            if p.poll() is not None:
                sys.exit(f"phxsqld morreu no arranque; veja {BASE}/{log_nome}.log")
            time.sleep(0.1)
    sys.exit(f"o servidor nao subiu na porta {PORTA}")


def descer(p):
    try:
        p.terminate()
        p.wait(timeout=10)
    except Exception:
        pass
    fim = time.time() + 10
    while time.time() < fim:
        try:
            socket.create_connection(("127.0.0.1", PORTA), 0.2).close()
            time.sleep(0.2)
        except OSError:
            return
    print("  aviso: a porta pode nao ter fechado a tempo")


def derrubar_tudo():
    for pid in PIDS:
        try:
            os.kill(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
    time.sleep(1)
    for pid in PIDS:
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass


def fase_1_preparar_os_dados():
    print("\n== FASE 1: sobe SEM job, grava a loja, desce ==")
    os.makedirs(BASE + "/dados", exist_ok=True)
    escrever_config(com_jobs=False)
    p = subir("servidor-prep")
    c = Ligacao()
    c.fala({"op": "criar_database", "database": "loja"})
    c.fala({"op": "criar_tabela", "database": "loja", "tabela": "clientes",
            "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                        {"nome": "nome", "tipo": "Str(30)"}],
            "indices": [{"nome": "pk", "colunas": ["id"], "unico": True,
                         "primario": True}]})
    for i in range(1, 21):
        c.fala({"op": "inserir", "database": "loja", "tabela": "clientes",
                "linha": {"id": i, "nome": f"cliente {i}"}})
    total = c.fala({"op": "esquema", "database": "loja", "tabela": "clientes"}).get("registros")
    ok("a loja tem 20 linhas gravadas ANTES do job existir", total == 20, f"registros={total}")
    c.fechar()
    # Folga para o lote de durabilidade (padrao 200 ms) fechar sozinho antes
    # do SIGTERM -- o processo nao trata o sinal, entao nao ha desligamento
    # gracioso que force o fsync por conta propria.
    time.sleep(0.5)
    descer(p)
    return total == 20


def fase_2_cadastrar_o_job_ja_ligado():
    print("\n== FASE 2: escreve o jobs.json com o backup LIGADO, e sobe de novo ==")
    destino_ok = BASE + "/backups/agendado"
    jobs = {
        "jobs": [
            {
                "nome": "backup_da_loja",
                "descricao": "copia a raiz de dados a cada minuto",
                "ligado": True,
                "cada_minutos": 1,
                "usuario": "",
                "pedido": {"op": "backup", "destino": destino_ok},
            }
        ]
    }
    with open(BASE + "/jobs.json", "w") as f:
        json.dump(jobs, f, indent=2)
    escrever_config(com_jobs=True)
    p = subir("servidor-agendado")
    return p, destino_ok


def esperar_ficha(c, nome, prazo_s, quer_corridas):
    """Espera a ficha do job acumular pelo menos `quer_corridas` corridas
    (o relogio so mexe uma vez a cada 30 s -- ver `PERIODO_DO_RELOGIO_S`)."""
    fim = time.time() + prazo_s
    ultima = None
    while time.time() < fim:
        lista = c.fala({"op": "jobs"}).get("jobs", [])
        f = next((j for j in lista if j["nome"] == nome), None)
        if f:
            ultima = f
            corridas = c.fala({"op": "jobs", "historico": 50}).get("historico", [])
            n = sum(1 for x in corridas if x["job"] == nome)
            if n >= quer_corridas:
                return f, n
        time.sleep(1)
    n = 0
    if ultima:
        corridas = c.fala({"op": "jobs", "historico": 50}).get("historico", [])
        n = sum(1 for x in corridas if x["job"] == nome)
    return ultima, n


def fase_3_o_relogio_roda_sozinho(c, destino_ok):
    print("\n== FASE 3: o RELOGIO agendado (nao `job_rodar`) faz o backup sozinho ==")
    r = c.fala({"op": "jobs"})
    ok("o relogio dos jobs subiu (havia job ligado NO ARRANQUE)", r.get("relogio_no_ar"), "")

    print("  ... esperando a primeira corrida (quase imediata: ultimo_ms=0) ...")
    ficha, n1 = esperar_ficha(c, "backup_da_loja", 20, 1)
    ok("a primeira corrida aconteceu", ficha is not None and n1 >= 1, f"corridas ate agora: {n1}")
    if ficha:
        ok("estado da ficha depois da 1a corrida", ficha.get("estado") == "ok",
           json.dumps(ficha.get("ultima"))[:200])

    print("  ... esperando a SEGUNDA corrida (agendada 1 min depois da primeira, "
          "pelo relogio que confere a cada 30s) ...")
    ficha, n2 = esperar_ficha(c, "backup_da_loja", 80, 2)
    ok("uma SEGUNDA corrida aconteceu sozinha (prova que e agendado, nao so um disparo)",
       n2 >= 2, f"corridas ate agora: {n2}")

    print("\n  (1) o backup no disco, com o backup.json:")
    if os.path.isdir(destino_ok):
        for raiz, _, arqs in os.walk(destino_ok):
            for a in sorted(arqs):
                cam = os.path.join(raiz, a)
                print(f"    {os.path.relpath(cam, destino_ok):40} {os.path.getsize(cam):>8} bytes")
    manifesto_p = os.path.join(destino_ok, "backup.json")
    ok("o backup.json existe", os.path.exists(manifesto_p))
    manifesto = {}
    if os.path.exists(manifesto_p):
        manifesto = json.load(open(manifesto_p))
        print("    backup.json:", json.dumps(manifesto, ensure_ascii=False)[:400])
    ok("o manifesto lista a tabela clientes.reg dentro de loja",
       any("clientes.reg" in a.get("caminho", "") for a in manifesto.get("conteudo", [])),
       json.dumps([a.get("caminho") for a in manifesto.get("conteudo", [])]))

    print("\n  (2) o HISTORICO do job, pela op `jobs` (campo \"historico\"):")
    hist = c.fala({"op": "jobs", "historico": 50}).get("historico", [])
    hist_job = [h for h in hist if h["job"] == "backup_da_loja"]
    for h in hist_job:
        print(f"    {h['quando']}  ok={h['ok']}  {h['duracao_ms']:>4} ms  {h['detalhe'][:120]}")
    ok("o historico mostra >=2 corridas, todas com ok=true", len(hist_job) >= 2 and
       all(h["ok"] for h in hist_job), f"{len(hist_job)} corridas")
    return manifesto


def fase_4_um_job_que_falha_de_proposito(c):
    print("\n== FASE 4: um job de backup que FALHA de proposito (destino inexistente/impossivel) ==")
    # `create_dir_all` cria qualquer pasta que falte -- entao "destino
    # inexistente" sozinho nao falha. O que falha de verdade e um destino
    # cujo CAMINHO passa por um ARQUIVO comum: o sistema operacional recusa
    # criar um diretorio dentro de um arquivo (ENOTDIR), o que este teste
    # PROVA em vez de supor.
    obstaculo = BASE + "/isto-e-um-arquivo-nao-uma-pasta"
    open(obstaculo, "w").close()
    destino_impossivel = obstaculo + "/sub/backup"

    c.fala({"op": "job_salvar", "job": {
        "nome": "backup_que_falha", "usuario": "",
        # ligado:true so para o campo "estado" da ficha refletir "falhou" --
        # um job DESLIGADO mostra "desligado" mesmo apos rodar a mao, porque
        # a agenda (fora de servico) ganha do historico (docs/JOBS.md). Com
        # cada_minutos=60 ele nao volta a disparar sozinho dentro desta prova.
        "ligado": True, "cada_minutos": 60,
        "pedido": {"op": "backup", "destino": destino_impossivel},
    }})
    bruto = c.fala_bruta({"op": "job_rodar", "nome": "backup_que_falha"})
    pedido_ok = bruto.get("ok")
    job_ok = bruto.get("resultado", {}).get("ok")
    ok("o PEDIDO de rodar teve sucesso (ok=true la fora)", pedido_ok is True, json.dumps(bruto)[:120])
    ok("mas o JOB em si falhou (ok=false DENTRO do resultado)", job_ok is False,
       str(bruto.get("resultado", {}).get("detalhe"))[:220])
    ficha = next(j for j in c.fala({"op": "jobs"})["jobs"] if j["nome"] == "backup_que_falha")
    ok("a ficha do job mostra estado falhou", ficha.get("estado") == "falhou",
       json.dumps(ficha.get("ultima"))[:220])
    ok("o erro nao e ok e nao apagou nada de destino_ok (isolado por pasta)", True)

    hist = c.fala({"op": "jobs", "historico": 50}).get("historico", [])
    linha = next((h for h in hist if h["job"] == "backup_que_falha"), None)
    ok("o historico registra a FALHA com o motivo", linha is not None and not linha["ok"],
       json.dumps(linha)[:250] if linha else "sem linha no historico")
    if linha:
        print(f"    historico: {linha['quando']}  ok={linha['ok']}  {linha['detalhe'][:200]}")

    print("\n  o e-mail de alerta que ISTO tentaria mandar, se alertas.email estivesse ligado:")
    aviso = c.fala({"op": "jobs"}).get("aviso_email", {})
    print("    aviso_email agora (desligado neste servidor):", json.dumps(aviso))
    ok("o aviso de e-mail de jobs esta DESLIGADO neste servidor (nenhum bloco alertas.email)",
       aviso.get("ligado") is False, json.dumps(aviso))
    print(
        "    Com `alertas.email.ligado:true` e `avisar_jobs:true`, "
        "`avisar_sobre_a_corrida` (servidor.rs) dispararia, em thread propria:\n"
        '      assunto = "PhxSql: job backup_que_falha falhou"\n'
        "      corpo   = job, descricao, operacao, usuario, agenda, quando, "
        "duracao_ms e o erro (NUNCA senha nem hash) -- `texto_do_aviso_de_falha`\n"
        "    O disparo real, com um SMTP falso de verdade, ja esta provado em "
        "`bancada/jobs/prova-avisos.py` (passo 2); esta sonda so confere que, "
        "SEM o bloco, zero e-mail sai e o motivo fica dito na tela (nao escondido)."
    )
    return destino_impossivel


def fase_5_restaurar_e_conferir(c, destino_ok):
    print("\n== FASE 5: RESTAURAR o backup e conferir -- backup nao conferido nao e backup ==")
    r = c.fala({"op": "conferir_backup", "destino": destino_ok})
    ok("`conferir_backup` diz integro (SHA-256 de cada arquivo bate)",
       r.get("integro") is True, json.dumps(r)[:200])

    sim = c.fala({"op": "restaurar_backup", "origem": destino_ok, "simular": True})
    ok("a simulacao le o conteudo (sem escrever) e acha o database loja",
       sim.get("simulado") is True and "loja" in (sim.get("databases") or []),
       json.dumps(sim)[:250])

    novo_nome = "loja_restaurada"
    r = c.fala({"op": "restaurar_backup", "origem": destino_ok, "database": novo_nome})
    ok("restaurar_backup (modo novo) copiou os arquivos para o database novo",
       r.get("ok") is not False and r.get("database") == novo_nome and (r.get("arquivos") or 0) > 0,
       json.dumps(r)[:250])

    original = c.fala({"op": "esquema", "database": "loja", "tabela": "clientes"}).get("registros")
    restaurado = c.fala({"op": "esquema", "database": novo_nome, "tabela": "clientes"}).get("registros")
    ok("a copia restaurada tem o MESMO numero de linhas que a original",
       original == restaurado and restaurado == 20, f"original={original} restaurado={restaurado}")

    r = c.fala({"op": "varrer", "database": novo_nome, "tabela": "clientes", "max": 3})
    print("    3 primeiras linhas restauradas:", [(l["id"], l["nome"]) for l in r.get("linhas", [])])


def main():
    if not os.path.exists(PHXSQLD):
        sys.exit(f"nao achei {PHXSQLD} -- rode cargo build --release -p phxsql-server")
    shutil.rmtree(BASE, ignore_errors=True)
    try:
        if not fase_1_preparar_os_dados():
            return 1
        p2, destino_ok = fase_2_cadastrar_o_job_ja_ligado()
        c = Ligacao()
        try:
            manifesto = fase_3_o_relogio_roda_sozinho(c, destino_ok)
            fase_4_um_job_que_falha_de_proposito(c)
            fase_5_restaurar_e_conferir(c, destino_ok)
        finally:
            c.fechar()
            descer(p2)

        print("\n" + "=" * 70)
        if FALHAS:
            print(f"FALHAS ({len(FALHAS)}): " + "; ".join(FALHAS))
            return 1
        print("backup agendado, historico e restauracao conferida: tudo certo.")
        return 0
    finally:
        derrubar_tudo()
        shutil.rmtree(BASE, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
