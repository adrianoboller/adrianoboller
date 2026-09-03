#!/usr/bin/env python3
"""Roda os exemplos do MANUAL.txt contra um servidor de VERDADE.

# Por que ela existe

A secao de chave estrangeira do manual prometia o que o motor recusa. Tinha
quatro erros de uma vez:

1. dizia *"o motor ainda NAO a IMPOE"* -- falso desde o pedido 171;
2. listava as quatro acoes numa tabela so, sem dizer o lado -- e `ao_excluir`
   aceita **so** `restringir`, por pretea;
3. dizia *"ausente e restringir"* -- verdade no excluir, FALSO no alterar,
   que nasce `cascata`;
4. o exemplo do `declarar_fk` mandava `"ao_excluir":"cascata"`, que o motor
   **recusa**: quem copiasse do manual tomava erro.

E omitia o `"verificar"`, o nasce-conferida e a exigencia de indice dos dois
lados.

Nenhum deles aparecia lendo. **Exemplo de manual que ninguem executa e exemplo
que envelhece calado** -- e um manual que mente e pior que um manual que falta,
porque quem le confia.

# Como ela roda

    python3 bancada/manual/provar-manual.py

Sobe um `phxsqld` so dela numa porta propria, num diretorio temporario, e o
derruba **pelo PID** no fim -- nunca por `pkill`, que mataria o servidor de um
vizinho.

# Duas armadilhas que ela ja pagou, e que ficam escritas

- **Asserir pelo VEREDITO passa por engano.** A primeira versao conferia
  `ok:false` e duas conferencias passaram verdes com o servidor respondendo
  *"acesso negado: faca login"* -- nao era a FK recusando, era o portao. Hoje
  cada recusa e conferida pelo MOTIVO.
- **Prova que depende da ordem das corridas mente.** A primeira versao reusava
  o database `loja`, e a segunda corrida reprovou em tres por "chave
  duplicada". Hoje cada corrida nasce num database proprio.
"""

import json
import pathlib
import socket
import subprocess
import sys
import tempfile
import time

RAIZ = pathlib.Path(__file__).resolve().parents[2]
BINARIO = RAIZ / "target" / "release" / "phxsqld"
PORTA = 6410
DB = "loja_%d" % int(time.time())

falhas = []


def confere(rotulo, cond, detalhe=""):
    print(("  ok    " if cond else "  FALHA ") + rotulo
          + (f"  -> {detalhe}" if not cond and detalhe else ""))
    if not cond:
        falhas.append(rotulo)


def pedir(token, p):
    s = socket.create_connection(("127.0.0.1", PORTA), timeout=10)
    s.settimeout(10)
    p = dict(p)
    p.setdefault("token", token)
    s.sendall((json.dumps(p) + "\n").encode())
    buf = b""
    while not buf.endswith(b"\n"):
        d = s.recv(65536)
        if not d:
            break
        buf += d
    s.close()
    return json.loads(buf.decode())


def subir(dir_):
    """Sobe um servidor so desta prova, sem cadastro de usuario.

    Sem cadastro o token da poder total, e e o que o proprio MANUAL diz na
    secao de autenticacao. Assim a prova mede a FK, e nao o portao de login.
    """
    if not BINARIO.exists():
        print(f"binario ausente: {BINARIO}\n"
              "  cargo build --release -p phxsql-server --bin phxsqld")
        sys.exit(2)
    modelo = subprocess.run([str(BINARIO), "--exemplo", "1"],
                            capture_output=True, text=True, cwd=dir_)
    cfg = json.loads(modelo.stdout)
    cfg["bind"] = f"127.0.0.1:{PORTA}"
    for k in ("usuarios", "root"):
        cfg.pop(k, None)
    (dir_ / "config.json").write_text(json.dumps(cfg, indent=2))
    log = open(dir_ / "servidor.log", "w")
    proc = subprocess.Popen([str(BINARIO)], cwd=dir_, stdout=log, stderr=log)
    for _ in range(50):
        time.sleep(0.1)
        try:
            socket.create_connection(("127.0.0.1", PORTA), timeout=1).close()
            return proc, cfg["token"]
        except OSError:
            continue
    proc.kill()
    print("o servidor da prova nao subiu:\n" + (dir_ / "servidor.log").read_text())
    sys.exit(2)


def rodar(token):
    print("== o que o MANUAL promete, executado ==")
    print(f"  (database da corrida: {DB})")
    pedir(token, {"op": "criar_database", "database": DB})
    pedir(token, {"op": "criar_tabela", "database": DB, "tabela": "clientes",
                  "colunas": [{"nome": "id", "tipo": "Int4", "obrigatoria": True},
                              {"nome": "nome", "tipo": "Str(30)"}],
                  "indices": [{"nome": "porId", "colunas": ["id"],
                               "unico": True, "primario": True}]})

    # 1. o exemplo do criar_tabela, LITERAL do manual
    r = pedir(token, {
        "op": "criar_tabela", "database": DB, "tabela": "pedidos",
        "colunas": [{"nome": "id", "tipo": "Int4", "obrigatoria": True},
                    {"nome": "cliente_id", "tipo": "Int4"}],
        "indices": [{"nome": "porId", "colunas": ["id"], "unico": True, "primario": True},
                    {"nome": "porCliente", "colunas": ["cliente_id"]}],
        "chaves_estrangeiras": [
            {"nome": "fk_cliente", "colunas": ["cliente_id"],
             "tabela_ref": "clientes", "colunas_ref": ["id"],
             "ao_excluir": "restringir", "ao_alterar": "cascata"}]})
    confere("o exemplo do criar_tabela roda", r.get("ok") is True, r.get("erro"))

    # 2. ao_excluir aceita SO restringir -- e recusa na DECLARACAO
    for acao in ("cascata", "anular", "nada"):
        r = pedir(token, {"op": "declarar_fk", "database": DB, "tabela": "pedidos",
                          "nome": "fk_x", "colunas": ["cliente_id"],
                          "tabela_ref": "clientes", "colunas_ref": ["id"],
                          "ao_excluir": acao})
        confere(f'ao_excluir:"{acao}" e RECUSADO na declaracao',
                r.get("ok") is False and "restringir" in str(r.get("erro", "")),
                r.get("erro"))

    # 3. ausente vale coisas DIFERENTES em cada lado
    r = pedir(token, {"op": "esquema", "database": DB, "tabela": "pedidos"})
    fk = ((r.get("resultado") or {}).get("chaves_estrangeiras") or [{}])[0]
    confere("o esquema devolve a chave declarada", fk.get("nome") == "fk_cliente", r)
    confere('ausente em ao_excluir = "restringir"',
            str(fk.get("ao_excluir", "")).lower().startswith("restring"), fk)
    confere('ausente em ao_alterar  = "cascata"',
            str(fk.get("ao_alterar", "")).lower().startswith("cascat"), fk)

    # 4. a chave declarada NASCE conferida
    confere('a chave nasce com "verificar": true', fk.get("verificar") is True, fk)

    # 5. conferida quer dizer IMPOSTA na gravacao -- e o motivo e conferido
    r = pedir(token, {"op": "inserir", "database": DB, "tabela": "pedidos",
                      "valores": {"id": 1, "cliente_id": 999}})
    confere("filha sem mae e RECUSADA no inserir, PELA FK",
            r.get("ok") is False
            and "integridade referencial" in str(r.get("erro", "")).lower(),
            r.get("erro"))

    pedir(token, {"op": "inserir", "database": DB, "tabela": "clientes",
                  "valores": {"id": 7, "nome": "Ana"}})
    r = pedir(token, {"op": "inserir", "database": DB, "tabela": "pedidos",
                      "valores": {"id": 1, "cliente_id": 7}})
    confere("filha COM mae grava", r.get("ok") is True, r.get("erro"))

    r = pedir(token, {"op": "excluir", "database": DB, "tabela": "clientes", "rowid": 1})
    confere("mae COM filha e RECUSADA no excluir, PELA FK",
            r.get("ok") is False
            and "integridade referencial" in str(r.get("erro", "")).lower(),
            r.get("erro"))

    # 6. o interruptor do lado contrario
    pedir(token, {"op": "criar_tabela", "database": DB, "tabela": "soltas",
                  "colunas": [{"nome": "id", "tipo": "Int4", "obrigatoria": True},
                              {"nome": "cliente_id", "tipo": "Int4"}],
                  "indices": [{"nome": "porId", "colunas": ["id"],
                               "unico": True, "primario": True},
                              {"nome": "porCliente", "colunas": ["cliente_id"]}],
                  "chaves_estrangeiras": [
                      {"nome": "fk_solta", "colunas": ["cliente_id"],
                       "tabela_ref": "clientes", "colunas_ref": ["id"],
                       "verificar": False}]})
    r = pedir(token, {"op": "inserir", "database": DB, "tabela": "soltas",
                      "valores": {"id": 1, "cliente_id": 999}})
    confere('"verificar": false grava sem mae', r.get("ok") is True, r.get("erro"))


def main():
    with tempfile.TemporaryDirectory(prefix="phx-manual-") as d:
        dir_ = pathlib.Path(d)
        proc, token = subir(dir_)
        try:
            rodar(token)
        finally:
            # Pelo PID, e nunca por `pkill -f phxsqld`: ha outros servidores
            # nesta maquina, e um deles pode ser o de um agente vizinho.
            proc.terminate()
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                proc.kill()
    print()
    if falhas:
        print(f"REPROVOU em {len(falhas)}: {falhas}")
        return 1
    print("o manual diz a verdade: todos os exemplos rodam como escrito")
    return 0


if __name__ == "__main__":
    sys.exit(main())
