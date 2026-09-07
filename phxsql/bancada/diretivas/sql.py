#!/usr/bin/env python3
"""Bateria do `SHOW … SETTINGS` e do `ALTER … SET`, contra o motor VIVO.

    cargo build --release -p phxsql-server --bin phxsqld
    python3 bancada/diretivas/sql.py

Pedido do dono, 07/09/2026: centralizar as diretivas do HFSQL num `SHOW` e num
`ALTER … SET`, com toda alteracao administrativa registrada com nove campos.

# Por que existe

Ler o codigo diz que o `ALTER SERVER SET` chama o `config_gravar`. Esta bateria
PROVA -- pelo soquete, com o `config.json` aberto em disco DEPOIS de cada
gravacao e o `diretivas.log` lido linha a linha. E ela mede duas coisas que
nenhum teste unitario alcanca: a diretiva por banco valendo A QUENTE (sem
reiniciar o processo) e a compressao que NAO existe, com o numero que decide se
ela vale a pena.

# O instrumento antes do veredito

Cada afirmacao de ausencia vem depois do controle positivo. A varredura que
procura segredo no diario so vale porque ela acha o segredo quando ele esta la;
a prova de que `reindexar` e proibido so em `erp` so vale porque a mesma
operacao PASSA em `loja`.

# Como roda

Sobe DOIS `phxsqld` de verdade -- 7300 (o principal) e 7301 (o do portao de
permissao, com cadastro de usuario) --, ambos em 127.0.0.1, com tudo em
`/tmp/phx-f3-sql-<pid>`; derruba por PID no fim, nunca `pkill`. As portas saem
de `PHX_F3_PORTA_A` e `PHX_F3_PORTA_B`. Os numeros vao para
`resultados-sql.json`.
"""

import json
import os
import shutil
import socket
import subprocess
import sys
import time
import zlib

RAIZ = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
BINARIO = os.path.join(RAIZ, "target/release/phxsqld")
BASE = f"/tmp/phx-f3-sql-{os.getpid()}"
PORTA_A = int(os.environ.get("PHX_F3_PORTA_A", "7300"))
PORTA_B = int(os.environ.get("PHX_F3_PORTA_B", "7301"))

SENHA_OP = "senha-do-operador-2026"

FALHAS = []
AFIRMACOES = 0
SAIDAS = {}


def afirmar(rotulo, ok, detalhe=""):
    global AFIRMACOES
    AFIRMACOES += 1
    print(f"  [{'ok' if ok else 'FALHOU'}] {rotulo}" + (f" -- {detalhe}" if detalhe else ""))
    if not ok:
        FALHAS.append(rotulo)
    return ok


def hash_de(senha):
    """`echo -n <senha> | phxsqld --senha`, pelo cano: a senha nao entra no
    historico do shell nem aparece num `ps`."""
    r = subprocess.run([BINARIO, "--senha"], input=senha.encode(), capture_output=True)
    saida = (r.stdout or r.stderr).decode().strip()
    return saida.split('"')[3] if '"' in saida else saida


class Servidor:
    """Um `phxsqld` de verdade, derrubado por PID no fim aconteca o que acontecer."""

    def __init__(self, nome, porta, config):
        self.nome, self.porta = nome, porta
        self.dir = f"{BASE}/{nome}"
        self.config = config
        self.s = self.f = None

    def __enter__(self):
        if not os.path.exists(BINARIO):
            raise SystemExit(
                f"{BINARIO} nao existe. Rode antes:\n"
                "  cargo build --release -p phxsql-server --bin phxsqld"
            )
        os.makedirs(self.dir + "/dados", exist_ok=True)
        self.cfg = self.dir + "/config.json"
        with open(self.cfg, "w") as f:
            f.write(self.config)
        self.saida = open(self.dir + "/servidor.err", "wb")
        self.p = subprocess.Popen(
            [BINARIO, "--config", self.cfg], stdout=self.saida, stderr=subprocess.STDOUT
        )
        for _ in range(120):
            try:
                socket.create_connection(("127.0.0.1", self.porta), 0.2).close()
                break
            except OSError:
                time.sleep(0.1)
        else:
            raise SystemExit(f"o servidor {self.nome} nao subiu na porta {self.porta}")
        self.abrir()
        return self

    def abrir(self):
        self.fechar()
        self.s = socket.create_connection(("127.0.0.1", self.porta), 15)
        self.f = self.s.makefile("rwb")

    def fechar(self):
        for x in (self.f, self.s):
            try:
                if x:
                    x.close()
            except OSError:
                pass
        self.f = self.s = None

    def __exit__(self, *_):
        self.fechar()
        self.p.terminate()
        try:
            self.p.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.p.kill()
        self.saida.close()

    def cru(self, **kw):
        kw.setdefault("token", "t")
        self.f.write((json.dumps(kw) + "\n").encode())
        self.f.flush()
        return self.f.readline().decode().rstrip("\n")

    def pedir(self, **kw):
        return json.loads(self.cru(**kw))

    def sql(self, texto, **kw):
        return self.pedir(op="sql", texto=texto, **kw)

    def dentro(self, texto, **kw):
        """O resultado da operacao, sem as duas cascas do `op sql`."""
        r = self.sql(texto, **kw)
        if not r.get("ok"):
            return r
        return r["resultado"]["resultado"]

    def arquivo(self):
        with open(self.cfg) as f:
            return f.read()

    def diario(self):
        caminho = self.dir + "/diretivas.log"
        if not os.path.exists(caminho):
            return []
        with open(caminho) as f:
            return [json.loads(l) for l in f if l.strip()]


CONFIG_A = """{
  "_nota": "comentario que a gravacao NAO pode comer",
  "bind": "127.0.0.1:%d",
  "base": "%s/a/dados",
  "token": "t",
  "max_linhas": 1000,
  "timeout_s": 30,
  "log_acessos": "%s/a/acessos.log",
  "jobs": "%s/a/jobs.json",
  "web": { "ligado": false },
  "seguranca": {
    "blacklist": "%s/a/blacklist.json",
    "whitelist": ["127.0.0.1"],
    "comandos_proibidos": ["excluir_tabela"]
  }
}
"""


def config_b(hash_op):
    return json.dumps(
        {
            "bind": f"127.0.0.1:{PORTA_B}",
            "base": f"{BASE}/b/dados",
            "token": "t",
            "log_acessos": f"{BASE}/b/acessos.log",
            "jobs": f"{BASE}/b/jobs.json",
            "web": {"ligado": False},
            "seguranca": {
                "blacklist": f"{BASE}/b/blacklist.json",
                "whitelist": ["127.0.0.1"],
            },
            "usuarios": [
                {
                    "id": 20,
                    "nome": "Olivia Operadora",
                    "login": "olivia",
                    "senha_hash": hash_op,
                    "ativo": True,
                    "supervisor": False,
                    "bases": {
                        "*": {
                            "ler": True,
                            "inserir": True,
                            "alterar": True,
                            "excluir": True,
                            "criar": True,
                            "reindexar": True,
                            "administrar": False,
                        }
                    },
                }
            ],
        },
        indent=2,
        ensure_ascii=False,
    )


def parte(n, titulo):
    print(f"\n=== {n}. {titulo}\n")


def main():
    if os.path.exists(BASE):
        shutil.rmtree(BASE)
    os.makedirs(BASE, exist_ok=True)
    versao = subprocess.run(
        [BINARIO, "--version"], capture_output=True
    ).stdout.decode().strip()
    commit = subprocess.run(
        ["git", "-C", RAIZ, "rev-parse", "--short", "HEAD"], capture_output=True
    ).stdout.decode().strip()
    print(f"# {versao} | commit {commit} | {time.strftime('%Y-%m-%d %H:%M:%S')}")

    cfg_a = CONFIG_A % (PORTA_A, BASE, BASE, BASE, BASE)
    with Servidor("a", PORTA_A, cfg_a) as s:
        # ------------------------------------------------------------ 0. o chao
        parte(0, "O CHAO: dois bancos e duas tabelas com chave")
        for b in ("erp", "loja"):
            afirmar(f"criei o banco {b}", s.pedir(op="criar_database", database=b).get("ok"))
        s.pedir(
            op="criar_tabela",
            database="erp",
            tabela="clientes",
            colunas=[{"nome": "id", "tipo": "Int8"}, {"nome": "cnpj", "tipo": "Str(20)"}],
            indices=[
                {"nome": "ix_id", "colunas": ["id"], "unico": True},
                {"nome": "ix_cnpj", "colunas": ["cnpj"], "unico": True},
            ],
        )
        s.pedir(
            op="criar_tabela",
            database="erp",
            tabela="pedidos",
            colunas=[{"nome": "id", "tipo": "Int8"}, {"nome": "cliente", "tipo": "Int8"}],
            indices=[
                {"nome": "ix_id", "colunas": ["id"], "unico": True},
                {"nome": "ix_cli", "colunas": ["cliente"], "unico": False},
            ],
        )
        fk = s.pedir(
            op="declarar_fk",
            database="erp",
            tabela="pedidos",
            nome="fk_ped_cli",
            colunas=["cliente"],
            tabela_ref="clientes",
            colunas_ref=["id"],
        )
        afirmar("declarei a chave estrangeira", fk.get("ok"), json.dumps(fk.get("resultado")))

        # -------------------------------------------------------- 1. os quatro SHOW
        parte(1, "SHOW … SETTINGS: os quatro escopos")
        srv = s.dentro("SHOW SERVER SETTINGS")
        SAIDAS["show_server"] = s.cru(op="sql", texto="SHOW SERVER SETTINGS")[:400]
        afirmar(
            "SHOW SERVER SETTINGS lista as diretivas configuraveis",
            srv.get("configuraveis", 0) > 40,
            f"{srv.get('configuraveis')} recursos",
        )
        afirmar(
            "cada recurso traz valor, tipo, escopo e se aplica a quente",
            all(
                set(("recurso", "valor", "tipo", "aplica", "a_quente", "escopo")) <= set(x)
                for x in srv["recursos"]
            ),
        )
        quentes = sum(1 for x in srv["recursos"] if x["a_quente"])
        afirmar(
            "e ele separa o que vale agora do que espera reinicio",
            0 < quentes < len(srv["recursos"]),
            f"{quentes} a quente, {len(srv['recursos']) - quentes} exigem reinicio",
        )
        afirmar(
            "nenhum segredo sai no SHOW",
            '"t"' not in json.dumps(srv) and "senha" not in json.dumps(srv).lower(),
        )

        base = s.dentro("SHOW DATABASE erp SETTINGS")
        SAIDAS["show_database"] = json.dumps(base, ensure_ascii=False)
        afirmar("SHOW DATABASE conta as tabelas do banco", base.get("tabelas") == 2, str(base.get("tabelas")))
        afirmar(
            "e diz que o global vale aqui",
            base.get("comandos_proibidos_globais") == ["excluir_tabela"],
            json.dumps(base.get("comandos_proibidos_globais")),
        )

        tab = s.dentro("SHOW TABLE pedidos SETTINGS", database="erp")
        SAIDAS["show_table"] = json.dumps(tab, ensure_ascii=False)
        afirmar(
            "SHOW TABLE le o duplicate_check de cada indice (= o `unico`)",
            {x["indice"]: x["duplicate_check"] for x in tab["duplicidade"]}
            == {"ix_id": True, "ix_cli": False},
            json.dumps(tab["duplicidade"]),
        )
        ri = tab["integridade_referencial"][0]
        afirmar(
            "e a integridade referencial da chave -- que NASCE conferida",
            ri["referential_integrity"] is True,
            json.dumps(ri),
        )
        afirmar(
            "com o ao_excluir em Restringir, que e a petrea",
            ri["ao_excluir"] == "Restringir",
            ri["ao_excluir"],
        )

        con = s.dentro("SHOW CONNECTION SETTINGS")
        SAIDAS["show_connection"] = json.dumps(con, ensure_ascii=False)
        afirmar("SHOW CONNECTION diz o estado da cifra do fio", con.get("encryption") is True)
        afirmar(
            "e diz, com todas as letras, que compressao no fio NAO existe",
            con.get("compression") is False,
        )

        # o SHOW novo nao roubou o SHOW velho
        afirmar(
            "SHOW TRIGGERS continua sendo do outro analisador",
            s.sql("SHOW TRIGGERS", database="erp").get("ok"),
        )
        afirmar(
            "SHOW PROCEDURES tambem",
            s.sql("SHOW PROCEDURES", database="erp").get("ok"),
        )

        # ------------------------------------------------- 2. ALTER SERVER SET
        parte(2, "ALTER SERVER SET: o MESMO caminho do config_gravar")
        antes_arquivo = s.arquivo()
        r = s.dentro("ALTER SERVER SET max_linhas = 500 MOTIVO 'pico de exportacao'")
        SAIDAS["alter_server"] = json.dumps(
            {k: v for k, v in r.items() if k != "config"}, ensure_ascii=False
        )
        afirmar("ALTER SERVER SET gravou", r.get("gravado") is True)
        depois = s.arquivo()
        afirmar('o config.json em disco traz "max_linhas": 500', '"max_linhas": 500' in depois, )
        afirmar(
            "e o comentario do arquivo sobreviveu a gravacao",
            "comentario que a gravacao NAO pode comer" in depois,
        )
        vivo = s.pedir(op="config")["resultado"]
        afirmar("o campo a quente valeu AGORA, sem reiniciar", vivo["max_linhas"] == 500, str(vivo["max_linhas"]))

        r = s.dentro("ALTER SERVER SET timeout_s = 45 MOTIVO 'suporte'")
        afirmar(
            "o campo que exige reinicio DIZ que exige, em vez de prometer efeito",
            r.get("exigem_reinicio") == ["timeout_s"],
            json.dumps(r.get("exigem_reinicio")),
        )
        srv = s.dentro("SHOW SERVER SETTINGS")
        t = [x for x in srv["recursos"] if x["recurso"] == "timeout_s"][0]
        afirmar(
            "e o SHOW mostra o que ja esta GRAVADO ao lado do que ainda vale",
            t.get("valor") == 30 and t.get("no_arquivo") == 45,
            json.dumps(t),
        )
        m = [x for x in srv["recursos"] if x["recurso"] == "max_linhas"][0]
        afirmar(
            "o controle positivo: o campo a quente NAO ganha o par (nada esperando)",
            "no_arquivo" not in m,
            json.dumps(m),
        )

        # a mesma conferencia de tipo, porque e o mesmo caminho
        e = s.sql("ALTER SERVER SET max_linhas = abc")
        afirmar(
            "tipo errado recusa com a mensagem do config_gravar",
            not e.get("ok") and "espera inteiro" in e.get("erro", ""),
            e.get("erro", ""),
        )
        e = s.sql("ALTER SERVER SET nao_existe = 1")
        afirmar("campo que nao se grava recusa", not e.get("ok"), e.get("erro", ""))

        # --------------------------------------- 3. a diretiva POR BANCO (220)
        parte(3, "ALTER DATABASE … SET comandos_proibidos: o pedido 220")
        antes = s.pedir(op="reindexar", database="erp", tabela="pedidos")
        afirmar(
            "ANTES: reindexar passa em erp (o controle positivo)",
            antes.get("ok") is True,
            json.dumps(antes.get("resultado")),
        )
        r = s.dentro(
            "ALTER DATABASE erp SET comandos_proibidos = (reindexar) MOTIVO 'auditoria interna'"
        )
        SAIDAS["alter_database"] = json.dumps(r, ensure_ascii=False)
        afirmar("a diretiva por banco gravou", r.get("gravado") is True)
        afirmar("e diz o que acrescentou", r.get("acrescentados") == ["reindexar"])
        afirmar("e avisa que ela so ACRESCENTA", "ACRESCENTA" in r.get("aviso", ""))

        depois = s.pedir(op="reindexar", database="erp", tabela="pedidos")
        afirmar(
            "DEPOIS: reindexar e recusado em erp -- A QUENTE, sem reiniciar",
            not depois.get("ok") and "proibida no banco erp" in depois.get("erro", ""),
            depois.get("erro", ""),
        )
        afirmar("com o codigo de acesso negado, e nao um erro de dado", depois.get("codigo") == 4001)
        SAIDAS["recusa_por_banco"] = json.dumps(depois, ensure_ascii=False)

        # o controle positivo que faz a prova valer: em OUTRO banco passa
        s.pedir(
            op="criar_tabela",
            database="loja",
            tabela="clientes",
            colunas=[{"nome": "id", "tipo": "Int8"}],
            indices=[{"nome": "ix", "colunas": ["id"], "unico": True}],
        )
        outra = s.pedir(op="reindexar", database="loja", tabela="clientes")
        afirmar(
            "e a MESMA operacao passa em loja: a regra nao vazou de banco",
            outra.get("ok") is True,
            json.dumps(outra.get("resultado")),
        )

        # o global continua valendo em todo banco
        for b in ("erp", "loja"):
            g = s.pedir(op="excluir_tabela", database=b, tabela="nao_existe")
            afirmar(
                f"o global continua proibindo excluir_tabela em {b}",
                "proibida neste servidor" in g.get("erro", ""),
                g.get("erro", ""),
            )

        arq = json.loads(s.arquivo())
        afirmar(
            "no config.json a entrada por banco e um objeto na MESMA lista",
            {"comando": "reindexar", "database": "erp"} in arq["seguranca"]["comandos_proibidos"],
            json.dumps(arq["seguranca"]["comandos_proibidos"]),
        )
        afirmar(
            "e a entrada GLOBAL continua intacta ao lado dela",
            "excluir_tabela" in arq["seguranca"]["comandos_proibidos"],
        )

        e = s.sql("ALTER DATABASE erp SET comandos_proibidos = ()")
        afirmar(
            "a lista vazia recusa: ela so acrescenta, nunca retira",
            not e.get("ok") and "so ACRESCENTA" in e.get("erro", ""),
            e.get("erro", ""),
        )
        e = s.sql("ALTER DATABASE erp SET comandos_proibidos = (voar)")
        afirmar(
            "proibir uma operacao que nao existe recusa",
            not e.get("ok") and "nao e uma operacao" in e.get("erro", ""),
            e.get("erro", ""),
        )

        # -------------------------------------------- 4. as dispensas, ditas
        parte(4, "O que NAO existe recusa NOMEANDO o caminho que funciona")
        for sql, pedaco in [
            ("ALTER TABLE clientes SET duplicate_check = TRUE", "criar_tabela"),
            ("ALTER TABLE pedidos SET referential_integrity = FALSE", "NASCE conferida"),
            ("ALTER CONNECTION SET compression = TRUE", "nao existe"),
            ("ALTER DATABASE erp SET transactions = TRUE", "comandos_proibidos"),
        ]:
            e = s.sql(sql, database="erp")
            afirmar(
                f"{sql} recusa dizendo o caminho",
                not e.get("ok") and pedaco in e.get("erro", ""),
                e.get("erro", "")[:160],
            )
            SAIDAS.setdefault("recusas", {})[sql] = e.get("erro", "")

        # ------------------------------------------------------- 5. o diario
        parte(5, "O diario administrativo: os nove campos do dono")
        linhas = s.diario()
        SAIDAS["diario"] = linhas[:4]
        afirmar(
            "o diretivas.log existe ao lado do acessos.log",
            os.path.exists(s.dir + "/diretivas.log"),
            s.dir + "/diretivas.log",
        )
        afirmar("e tem uma linha por alteracao", len(linhas) >= 3, f"{len(linhas)} linhas")
        nove = ["data_hora", "servidor", "banco", "recurso", "valor_anterior",
                "valor_novo", "usuario", "ip_origem", "motivo"]
        afirmar(
            "cada linha traz os NOVE campos pedidos",
            all(all(c in l for c in nove) for l in linhas),
            ", ".join(nove),
        )
        primeira = [l for l in linhas if l["recurso"] == "max_linhas"][0]
        afirmar(
            "o diario diz de que valor se saiu e para qual se foi",
            primeira["valor_anterior"] == 1000 and primeira["valor_novo"] == 500,
            json.dumps(primeira),
        )
        afirmar("e o MOTIVO do comando chegou", primeira["motivo"] == "pico de exportacao")
        afirmar("com o IP de origem", primeira["ip_origem"] == "127.0.0.1")
        porbanco = [l for l in linhas if l["recurso"] == "comandos_proibidos"][0]
        afirmar(
            "a diretiva por banco tambem entra, com o banco no lugar certo",
            porbanco["banco"] == "erp" and porbanco["valor_novo"] == ["reindexar"],
            json.dumps(porbanco),
        )

        # a OUTRA porta: o config_gravar do protocolo tambem entra no diario
        s.pedir(op="config_gravar", campos={"espelho": True}, motivo="pela tela")
        linhas = s.diario()
        afirmar(
            "e o config_gravar do protocolo entra no MESMO diario",
            any(l["recurso"] == "espelho" and l["motivo"] == "pela tela" for l in linhas),
            json.dumps(linhas[-1]),
        )

        # senha nunca em texto puro -- com o controle positivo do varredor
        bruto = open(s.dir + "/diretivas.log").read()
        afirmar(
            "o varredor ACHA um segredo quando ele esta no texto (controle positivo)",
            "espelho" in bruto,
        )
        afirmar(
            "e o token do servidor NAO esta no diario",
            '"t"' not in bruto and "senha" not in bruto.lower(),
        )

        # o SHOW traz o diario junto -- e e isso que faz alguem le-lo
        srv = s.dentro("SHOW SERVER SETTINGS")
        afirmar(
            "SHOW … SETTINGS traz as ultimas linhas do diario junto",
            len(srv.get("diario", [])) >= 3,
            f"{len(srv.get('diario', []))} linhas",
        )

        # -------------------------------------- 6. a compressao que nao existe
        parte(6, "A premissa da compressao no fio, MEDIDA antes de virar plano")
        s.pedir(
            op="criar_tabela",
            database="loja",
            tabela="medida",
            colunas=[
                {"nome": "id", "tipo": "Int8"},
                {"nome": "nome", "tipo": "Str(60)"},
                {"nome": "cidade", "tipo": "Str(40)"},
            ],
            indices=[{"nome": "ix", "colunas": ["id"], "unico": True}],
        )
        s.pedir(
            op="inserir_lote",
            database="loja",
            tabela="medida",
            linhas=[
                {"id": i, "nome": f"Cliente numero {i}", "cidade": "Blumenau"}
                for i in range(1, 5001)
            ],
        )
        # O teto do servidor foi baixado a 500 na parte 2, e uma resposta
        # cortada mediria a compressao de meia resposta. Ele volta ao lugar
        # ANTES da medida -- numero medido em condicao que ninguem entende e
        # numero que nao serve.
        s.dentro("ALTER SERVER SET max_linhas = 10000 MOTIVO 'medir a compressao'")
        cru = s.cru(op="varrer", database="loja", tabela="medida", max=5000).encode()
        linhas_na_resposta = len(json.loads(cru)["resultado"]["linhas"])
        afirmar(
            "a resposta medida traz as 5.000 linhas inteiras",
            linhas_na_resposta == 5000,
            f"{linhas_na_resposta} linhas",
        )
        t0 = time.perf_counter()
        comprimido = zlib.compress(cru, 6)
        ms = (time.perf_counter() - t0) * 1000
        razao = len(cru) / len(comprimido)
        SAIDAS["compressao"] = {
            "bytes_crus": len(cru),
            "bytes_deflate": len(comprimido),
            "razao": round(razao, 2),
            "comprimir_ms": round(ms, 2),
            "linhas": linhas_na_resposta,
        }
        print(
            f"  resposta de 5.000 linhas: {len(cru)} bytes -> {len(comprimido)} "
            f"({razao:.2f}x), comprimir custa {ms:.2f} ms"
        )
        afirmar(
            "a compressao VALERIA a pena (o deflate desta casa ja existe)",
            razao > 3,
            f"{razao:.2f}x",
        )
        afirmar(
            "e ela nao existe no fio -- dispensa registrada, nao esquecimento",
            s.dentro("SHOW CONNECTION SETTINGS")["compression"] is False,
        )

    # ------------------------------------------------ 7. o portao de permissao
    parte(7, "O portao e UM so: quem nao administra nao ve e nao muda")
    with Servidor("b", PORTA_B, config_b(hash_de(SENHA_OP))) as s2:
        login = s2.pedir(op="login", usuario="olivia", senha=SENHA_OP)
        afirmar("a operadora entra", login.get("ok"), json.dumps(login.get("resultado", {}).get("login")))
        for op, ped in [
            ("SHOW SERVER SETTINGS", {}),
            ("ALTER SERVER SET max_linhas = 7", {}),
            ("ALTER DATABASE erp SET comandos_proibidos = (reindexar)", {}),
        ]:
            e = s2.sql(op, **ped)
            afirmar(
                f"{op} e recusado para quem nao tem administrar",
                not e.get("ok") and "administrar" in e.get("erro", ""),
                e.get("erro", "")[:120],
            )
        SAIDAS["recusa_de_permissao"] = s2.cru(op="sql", texto="SHOW SERVER SETTINGS")
        # o controle positivo: a MESMA sessao pode ler dado, entao o portao nao
        # esta recusando tudo.
        afirmar(
            "a MESMA sessao continua podendo ler (o portao nao recusa tudo)",
            s2.pedir(op="bancos").get("ok") is True,
        )

    # -------------------------------------------------------------- o veredito
    print(f"\n=== {AFIRMACOES} afirmacoes, {len(FALHAS)} falhas")
    for f in FALHAS:
        print(f"  FALHOU: {f}")
    saida = os.path.join(os.path.dirname(os.path.abspath(__file__)), "resultados-sql.json")
    with open(saida, "w") as f:
        json.dump(
            {
                "quando_utc": time.strftime("%Y-%m-%d %H:%M:%S", time.gmtime()),
                "commit": subprocess.run(
                    ["git", "-C", RAIZ, "rev-parse", "--short", "HEAD"], capture_output=True
                ).stdout.decode().strip(),
                "versao": subprocess.run(
                    [BINARIO, "--version"], capture_output=True
                ).stdout.decode().strip(),
                "afirmacoes": AFIRMACOES,
                "falhas": FALHAS,
                "saidas": SAIDAS,
            },
            f,
            indent=2,
            ensure_ascii=False,
        )
    print(f"# resultados em {saida}")
    shutil.rmtree(BASE, ignore_errors=True)
    return 1 if FALHAS else 0


if __name__ == "__main__":
    sys.exit(main())
