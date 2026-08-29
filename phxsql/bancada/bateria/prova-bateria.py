#!/usr/bin/env python3
"""A bateria de ponta a ponta dos SEIS itens, pelo SOQUETE.

    cargo build --release
    cargo build --release --examples -p phxsql-store   # a regra do binario velho
    python3 bancada/bateria/prova-bateria.py

Os seis itens que o dono pediu, feitos como um usuario faria -- criando o
banco, criando as tabelas, gerando as chaves, pendurando os gatilhos, chamando
os procedimentos e carregando cinco mil linhas -- e cada passo com o resultado
esperado escrito ANTES de rodar, que e o que separa prova de demonstracao:

 1. criar um database;
 2. criar tabelas dentro dele;
 3. UUID v7 como chave e relacionamento 1:N entre as tabelas;
 4. gatilhos (BEFORE, AFTER, SIGNAL, no lote, e a GUARDA DE CADEIA);
 5. procedimentos (IN/OUT, SELECT ... INTO, e a porta dos fundos fechada);
 6. carga de 5.000 registros, medida.

Sobe um phxsqld PROPRIO em 6300 (dados) e 6301 (web), com cadastro de
usuarios de verdade -- porque a prova da permissao do item 5 exige um usuario
que NAO pode ler uma das tabelas -- e mata SO o processo que ele mesmo criou,
pelo PID. Nunca toca em phxsqld de outra pessoa.

  --medir       roda tambem a medicao (item 6 inteiro); sem isso, so as provas
  --rodadas N   quantas rodadas intercaladas na medicao (padrao 3)
"""
import json
import os
import shutil
import socket
import subprocess
import sys
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
PHXSQLD = os.path.join(RAIZ, "target", "release", "phxsqld")
BASE = "/tmp/phx-bateria"
PORTA, PORTA_WEB = 6300, 6301
TOKEN = "bateria"
DB = "escola"

# As senhas ficam AQUI em texto porque sao de um servidor de teste que morre no
# fim do script; o que vai para o config.json e sempre o hash, gerado na hora
# pelo proprio phxsqld -- o sal muda a cada rodada, entao colar um hash fixo
# seria colar um sal fixo.
SENHAS = {"adm": "adm-1234", "pedro": "pedro-1234"}

falhas = []
notas = []


def confere(rotulo, visto, esperado):
    ok = visto == esperado
    print(f"  {'ok  ' if ok else 'ERRO'} {rotulo}: {visto!r}"
          + ("" if ok else f"   (esperava {esperado!r})"))
    if not ok:
        falhas.append(rotulo)


def confere_contem(rotulo, texto, pedaco):
    ok = pedaco.lower() in (texto or "").lower()
    print(f"  {'ok  ' if ok else 'ERRO'} {rotulo}: {str(texto)[:120]!r}"
          + ("" if ok else f"   (esperava conter {pedaco!r})"))
    if not ok:
        falhas.append(rotulo)


def nota(texto):
    notas.append(texto)
    print(f"  nota  {texto}")


def hash_da_senha(senha):
    r = subprocess.run([PHXSQLD, "--senha"], input=senha.encode(),
                       capture_output=True, check=True)
    # A saida e a linha pronta do config: "senha_hash": "pbkdf2-...".
    return r.stdout.decode().split('"')[3]


class Servidor:
    """Um phxsqld nosso. Morre pelo PID, e so ele."""

    def __init__(self, limpar=True):
        if limpar:
            shutil.rmtree(BASE, ignore_errors=True)
            os.makedirs(BASE, exist_ok=True)
            with open(os.path.join(BASE, "config.json"), "w") as f:
                json.dump(self.config(), f, indent=2)
        self.log = open(os.path.join(BASE, "servidor.log"), "a")
        self.proc = subprocess.Popen([PHXSQLD], cwd=BASE, stdout=self.log,
                                     stderr=subprocess.STDOUT,
                                     stdin=subprocess.DEVNULL)
        for _ in range(60):
            time.sleep(0.25)
            if self.no_ar():
                return
        raise SystemExit("o servidor nao subiu; veja " + os.path.join(BASE, "servidor.log"))

    @staticmethod
    def config():
        return {
            "base": "base",
            "bind": f"127.0.0.1:{PORTA}",
            "token": TOKEN,
            "web": {"ligado": True, "bind": f"127.0.0.1:{PORTA_WEB}"},
            "recursos": {"cache_paginas": 2048},
            "root": {"id": 1, "nome": "root", "login": "root",
                     "senha_hash": hash_da_senha("root-1234")},
            "usuarios": [
                {"id": 2, "nome": "Administrador da bateria", "login": "adm",
                 "senha_hash": hash_da_senha(SENHAS["adm"]),
                 "supervisor": True, "ativo": True, "bases": {}},
                # O Pedro e a prova do item 5: opera na base inteira, MENOS a
                # tabela `alunos`. Se um `CALL` conseguir ler `alunos` para
                # ele, o portao tem uma porta dos fundos.
                {"id": 3, "nome": "Pedro Operador", "login": "pedro",
                 "senha_hash": hash_da_senha(SENHAS["pedro"]),
                 "supervisor": False, "ativo": True,
                 "bases": {DB: {"ler": True, "inserir": True, "alterar": True,
                                "excluir": True, "criar": True,
                                "tabelas": {"alunos": {}}}}},
            ],
        }

    def no_ar(self):
        try:
            socket.create_connection(("127.0.0.1", PORTA), timeout=0.5).close()
            return True
        except OSError:
            return False

    def vivo(self):
        return self.proc.poll() is None

    def parar(self):
        if self.vivo():
            self.proc.terminate()
            try:
                self.proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait(timeout=10)
        self.log.close()


class Cliente:
    def __init__(self, login="adm", senha=None):
        self.s = socket.create_connection(("127.0.0.1", PORTA))
        self.s.settimeout(120)
        self.f = self.s.makefile("rwb")
        self.eu = self.ok({"op": "login", "usuario": login,
                           "senha": senha if senha is not None else SENHAS[login]})

    def fala(self, p):
        p.setdefault("token", TOKEN)
        self.f.write((json.dumps(p) + "\n").encode())
        self.f.flush()
        linha = self.f.readline()
        if not linha:
            return {"ok": False, "erro": "<<a conexao caiu sem resposta>>",
                    "caiu": True}
        return json.loads(linha.decode())

    def ok(self, p):
        r = self.fala(p)
        if not r.get("ok"):
            raise SystemExit(f"FALHOU {p.get('op')}: "
                             + json.dumps(r, ensure_ascii=False)[:400])
        return r["resultado"]

    def sql(self, texto, database=DB):
        return self.ok({"op": "sql", "database": database, "texto": texto})

    def fechar(self):
        try:
            self.f.close()
            self.s.close()
        except OSError:
            pass


def visiveis(c, tabela, database=DB):
    """Linhas que quem consulta VE. O excluir suave tira da lista sem mexer no
    contador do cabecalho, entao `registros` mentiria aqui."""
    n, cursor, passos = 0, None, 0
    while passos < 2000:
        passos += 1
        p = {"op": "varrer", "database": database, "tabela": tabela, "max": 500}
        # `depois` e o CURSOR do protocolo. Ja mandei `de` aqui, que a operacao
        # ignora em silencio -- e a varredura devolvia a MESMA pagina para
        # sempre. Campo que o servidor nao le nao avisa que nao leu.
        if cursor is not None:
            p["depois"] = cursor
        r = c.ok(p)
        n += len(r["linhas"])
        if not r.get("ha_mais"):
            return n
        cursor = r["cursor_fim"]
    raise SystemExit("varredura sem fim")


# ===================================================================== 1 e 2

def item_1_e_2_banco_e_tabelas(c):
    print("\n=== 1. criar um database ===\n")
    r = c.ok({"op": "criar_database", "database": DB})
    confere("o database nasceu", r["database"], DB)
    confere("e e um diretorio", os.path.isdir(os.path.join(BASE, "base", DB)), True)
    confere("e aparece na lista", DB in c.ok({"op": "bancos"}), True)

    print("\n=== 2. criar tabelas dentro dele ===\n")
    # A MAE do 1:N. Chave primaria Uuid -- e o que o item 3 pede.
    c.ok({"op": "criar_tabela", "database": DB, "tabela": "turmas",
          "colunas": [{"nome": "id", "tipo": "Uuid", "obrigatoria": True},
                      {"nome": "nome", "tipo": "Str(40)", "obrigatoria": True},
                      {"nome": "ano", "tipo": "Int4"}],
          "indices": [{"nome": "porId", "colunas": ["id"],
                       "unico": True, "primario": True}]})
    # A FILHA, com a chave estrangeira declarada no proprio criar_tabela.
    c.ok({"op": "criar_tabela", "database": DB, "tabela": "alunos",
          "colunas": [{"nome": "id", "tipo": "Uuid", "obrigatoria": True},
                      {"nome": "turma_id", "tipo": "Uuid", "obrigatoria": True},
                      {"nome": "nome", "tipo": "Str(60)"},
                      {"nome": "cidade", "tipo": "Str(40)"},
                      {"nome": "nota", "tipo": "Decimal(5,2)"}],
          "indices": [{"nome": "porId", "colunas": ["id"],
                       "unico": True, "primario": True},
                      {"nome": "porTurma", "colunas": ["turma_id"]}],
          "chaves_estrangeiras": [{"nome": "fk_turma", "colunas": ["turma_id"],
                                   "tabela_ref": "turmas", "colunas_ref": ["id"],
                                   "ao_excluir": "restringir"}]})
    c.ok({"op": "criar_tabela", "database": DB, "tabela": "auditoria",
          "colunas": [{"nome": "evento", "tipo": "Str(120)"},
                      {"nome": "quem", "tipo": "Str(40)"}]})
    # A tabela que NUNCA tera gatilho: e ela que prova o comportamento velho.
    c.ok({"op": "criar_tabela", "database": DB, "tabela": "sem_gatilho",
          "colunas": [{"nome": "id", "tipo": "Uuid", "obrigatoria": True},
                      {"nome": "cidade", "tipo": "Str(40)"}],
          "indices": [{"nome": "porId", "colunas": ["id"],
                       "unico": True, "primario": True}]})

    tabelas = sorted(c.ok({"op": "tabelas", "database": DB})["tabelas"])
    confere("as quatro tabelas existem", tabelas,
            ["alunos", "auditoria", "sem_gatilho", "turmas"])

    # Criar, excluir e criar de novo com o MESMO nome. E o que qualquer pessoa
    # faz ao errar o tipo de uma coluna -- e era o que NAO funcionava: o
    # `excluir_tabela` deixava o `.trash`, o `.reason` e o `.pag` para tras, e a
    # segunda criacao morria com "ja existe; use Table::abrir". Foi assim que
    # esta bateria achou o defeito: reaproveitando um nome de tabela.
    c.ok({"op": "criar_tabela", "database": DB, "tabela": "rascunho",
          "colunas": [{"nome": "n", "tipo": "Int8"}],
          "motivo_obrigatorio": True})
    r = c.ok({"op": "inserir", "database": DB, "tabela": "rascunho",
              "linha": {"n": 1}})
    # Excluir DE VEZ enche o `.trash` e o `.reason`, que sao os que ficavam.
    c.ok({"op": "excluir", "database": DB, "tabela": "rascunho",
          "rowid": r["rowid"], "definitivo": True, "motivo": "errei o tipo"})
    c.ok({"op": "excluir_tabela", "database": DB, "tabela": "rascunho",
          "confirmar": "rascunho"})
    confere("nao sobra arquivo nenhum da tabela excluida",
            sorted(f for f in os.listdir(os.path.join(BASE, "base", DB))
                   if f.startswith("rascunho.")), [])
    de_novo = c.fala({"op": "criar_tabela", "database": DB, "tabela": "rascunho",
                      "colunas": [{"nome": "n", "tipo": "Str(10)"}]})
    confere("e o nome volta a estar livre", de_novo.get("ok"), True)
    c.ok({"op": "excluir_tabela", "database": DB, "tabela": "rascunho",
          "confirmar": "rascunho"})

    e = c.ok({"op": "esquema", "database": DB, "tabela": "alunos"})
    # Cinco declaradas + as duas de sistema, e as de sistema no FIM.
    confere("sete colunas (5 + as 2 de sistema)", len(e["colunas"]), 7)
    confere("as de sistema sao as duas ULTIMAS",
            [c_["nome"] for c_ in e["colunas"] if c_["sistema"]],
            ["softdeleted", "rownum"])
    confere("e sao mesmo as duas do fim",
            [c_["nome"] for c_ in e["colunas"][-2:]], ["softdeleted", "rownum"])
    # O id de cada coluna do cadastro de campos e um v7, e eles CRESCEM na
    # ordem de declaracao: e a mesma promessa do item 3, um nivel abaixo.
    ids = [c_["id"] for c_ in e["colunas"]]
    confere("o id de cada coluna e um v7", all(x[14] == "7" for x in ids), True)
    confere("e eles crescem na ordem de declaracao", ids, sorted(ids))


# ======================================================================== 3

def item_3_uuid_v7_e_1_para_n(c):
    print("\n=== 3a. UUID v7 como chave: crescente de verdade ===\n")
    # A palavra "novo" pede um v7 ao servidor -- e o cliente nao precisa saber
    # montar um. Duzentas linhas, e a promessa e que NENHUMA saia fora de ordem.
    turmas = []
    for n in range(3):
        r = c.ok({"op": "inserir", "database": DB, "tabela": "turmas",
                  "linha": {"id": "novo", "nome": f"turma {n}", "ano": 2026}})
        turmas.append(r["rowid"])
    linhas = c.ok({"op": "varrer", "database": DB, "tabela": "turmas",
                   "max": 100})["linhas"]
    ids = [l["id"] for l in linhas]
    confere("a versao declarada e 7", {x[14] for x in ids}, {"7"})
    confere("e a ordem de digitacao ja sai crescente", ids, sorted(ids))

    # Agora sob concorrencia: quatro conexoes pedindo id ao MESMO tempo. E o
    # caso que o relogio sozinho nao separa -- por isso o contador de 12 bits.
    import threading
    saidas = []
    trava = threading.Lock()

    def cava():
        cc = Cliente("adm")
        meus = []
        for _ in range(50):
            r = cc.ok({"op": "inserir", "database": DB, "tabela": "sem_gatilho",
                       "linha": {"id": "novo", "cidade": "Blumenau"}})
            meus.append(r["rowid"])
        cc.fechar()
        with trava:
            saidas.extend(meus)

    fios = [threading.Thread(target=cava) for _ in range(4)]
    [f.start() for f in fios]
    [f.join() for f in fios]
    todos = [l["id"] for l in c.ok({"op": "varrer", "database": DB,
                                    "tabela": "sem_gatilho", "max": 500})["linhas"]]
    confere("200 ids sob 4 conexoes", len(todos), 200)
    confere("nenhum repetido", len(set(todos)), 200)
    # A ordem de DIGITACAO e a ordem dos ids: e disto que o .ndx depende, e e
    # a razao de o v7 existir aqui (o rownum da a ordem de chegada).
    confere("e a ordem de chegada e a ordem dos ids", todos, sorted(todos))

    print("\n=== 3b. relacionamento 1:N ===\n")
    mae = [l for l in linhas if l["nome"] == "turma 0"][0]
    outra = [l for l in linhas if l["nome"] == "turma 1"][0]
    for i in range(5):
        c.ok({"op": "inserir", "database": DB, "tabela": "alunos",
              "linha": {"id": "novo", "turma_id": mae["id"],
                        "nome": f"aluno {i}", "cidade": "Blumenau",
                        "nota": "7.50"}})
    c.ok({"op": "inserir", "database": DB, "tabela": "alunos",
          "linha": {"id": "novo", "turma_id": outra["id"],
                    "nome": "sozinho", "cidade": "Gaspar", "nota": "9.00"}})
    # O N do 1:N: o indice porTurma devolve as cinco filhas de uma mae so.
    filhas = c.ok({"op": "buscar", "database": DB, "tabela": "alunos",
                   "indice": "porTurma", "chave": [mae["id"]], "max": 100})
    confere("as cinco filhas da turma 0", len(filhas["linhas"]), 5)
    confere("e todas apontam para ela",
            {l["turma_id"] for l in filhas["linhas"]}, {mae["id"]})
    outra_f = c.ok({"op": "buscar", "database": DB, "tabela": "alunos",
                    "indice": "porTurma", "chave": [outra["id"]], "max": 100})
    confere("a outra turma tem uma filha so", len(outra_f["linhas"]), 1)

    # A chave declarada esta la, e sobrevive a fechar e abrir.
    e = c.ok({"op": "esquema", "database": DB, "tabela": "alunos"})
    fk = e["chaves_estrangeiras"][0]
    confere("a chave estrangeira esta declarada", fk["nome"], "fk_turma")
    confere("aponta para a mae", (fk["tabela_ref"], fk["colunas_ref"]),
            ("turmas", ["id"]))
    confere("com a acao que se pediu", fk["ao_excluir"], "Restringir")
    confere("e a coluna sabe que e estrangeira",
            [c_["estrangeira"] for c_ in e["colunas"] if c_["nome"] == "turma_id"],
            [True])

    # O COMPORTAMENTO VELHO, e ele e o que este teste trava: a chave e
    # DECLARADA e NAO E IMPOSTA. Escrito como teste para o dia em que isso
    # mudar quebrar aqui -- e nao na cabeca de quem confiou na declaracao.
    orfa = c.fala({"op": "inserir", "database": DB, "tabela": "alunos",
                   "linha": {"id": "novo",
                             "turma_id": "00000000-0000-7000-8000-000000000000",
                             "nome": "orfa", "cidade": "x", "nota": "1.00"}})
    confere("HOJE a filha orfa ENTRA (a chave nao e imposta)", orfa.get("ok"), True)
    antes = visiveis(c, "turmas")
    saiu = c.fala({"op": "excluir", "database": DB, "tabela": "turmas",
                   "rowid": mae["rowid"]})
    confere("HOJE a mae com filhas SAI (restringir nao restringe)",
            saiu.get("ok"), True)
    confere("e ela saiu mesmo", visiveis(c, "turmas"), antes - 1)
    c.ok({"op": "restaurar", "database": DB, "tabela": "turmas",
          "rowid": mae["rowid"]})
    nota("a chave estrangeira e declarada e nao imposta -- os dois passos acima "
         "travam esse limite; o dia em que o motor a impuser, eles falham")


# ======================================================================== 4

def item_4_gatilhos(c):
    print("\n=== 4a. BEFORE INSERT normaliza, e o AFTER audita ===\n")
    c.sql("CREATE TRIGGER normaliza BEFORE INSERT ON alunos FOR EACH ROW "
          "SET NEW.cidade = UPPER(TRIM(NEW.cidade))")
    c.sql("CREATE TRIGGER audita AFTER INSERT ON alunos FOR EACH ROW "
          "INSERT INTO auditoria (evento, quem) "
          "VALUES (CONCAT('entrou ', NEW.nome, ' de ', NEW.cidade), 'gatilho')")
    antes_aud = visiveis(c, "auditoria")
    turma = c.ok({"op": "varrer", "database": DB, "tabela": "turmas",
                  "max": 10})["linhas"][0]
    r = c.ok({"op": "inserir", "database": DB, "tabela": "alunos",
              "linha": {"id": "novo", "turma_id": turma["id"],
                        "nome": "Ana", "cidade": "  joinville ", "nota": "8.00"}})
    lida = c.ok({"op": "ler", "database": DB, "tabela": "alunos",
                 "rowid": r["rowid"]})
    confere("o BEFORE normalizou", lida["cidade"], "JOINVILLE")
    confere("a resposta nao ganhou campo novo", "gatilhos_avisos" in r, False)
    aud = c.ok({"op": "varrer", "database": DB, "tabela": "auditoria",
                "max": 500})["linhas"]
    # UMA linha de auditoria, nem zero nem duas: o gatilho dispara UMA vez.
    confere("o AFTER disparou exatamente uma vez", len(aud) - antes_aud, 1)
    confere("e viu a linha como ela FICOU", aud[-1]["evento"],
            "entrou Ana de JOINVILLE")

    print("\n=== 4b. SIGNAL recusa, e a linha NAO entra ===\n")
    c.sql("CREATE TRIGGER exige_nota BEFORE INSERT ON alunos FOR EACH ROW "
          "IF NEW.nota IS NULL THEN "
          "  SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'aluno sem nota nao entra'; "
          "END IF")
    antes, antes_aud = visiveis(c, "alunos"), visiveis(c, "auditoria")
    neg = c.fala({"op": "inserir", "database": DB, "tabela": "alunos",
                  "linha": {"id": "novo", "turma_id": turma["id"],
                            "nome": "Sem nota", "cidade": "x"}})
    confere("recusou", neg.get("ok"), False)
    confere("com o nome do erro", neg.get("nome"), "SINAL")
    confere("e o codigo", neg.get("codigo"), 3005)
    confere("nao adianta repetir", neg.get("repetir"), False)
    confere_contem("a MESSAGE_TEXT chega inteira ao cliente", neg.get("erro"),
                   "aluno sem nota nao entra")
    confere("a linha recusada NAO entrou", visiveis(c, "alunos"), antes)
    # O AFTER nao roda para a linha que o BEFORE recusou: se rodasse, a
    # auditoria contaria uma insercao que nao aconteceu.
    confere("e o AFTER nao auditou o que nao entrou",
            visiveis(c, "auditoria"), antes_aud)

    print("\n=== 4c. o lote de 200 passa pelo BEFORE linha a linha ===\n")
    antes, antes_aud = visiveis(c, "alunos"), visiveis(c, "auditoria")
    linhas = [{"id": "novo", "turma_id": turma["id"], "nome": f"lote {i}",
               "cidade": " itajai ", "nota": "6.00"} for i in range(200)]
    # A do meio vem sem nota: o SIGNAL tem de recusar SO ela, com a POSICAO.
    linhas[100].pop("nota")
    r = c.ok({"op": "inserir_lote", "database": DB, "tabela": "alunos",
              "parar_no_erro": False, "linhas": linhas})
    confere("199 gravadas de 200", (r["gravadas"], r["recusadas"]), (199, 1))
    confere("e a posicao da recusada e a 101", r["erros"][0]["linha"], 101)
    confere_contem("com a frase do dono", r["erros"][0]["erro"],
                   "aluno sem nota nao entra")
    confere("as 199 entraram", visiveis(c, "alunos"), antes + 199)
    amostra = c.ok({"op": "buscar", "database": DB, "tabela": "alunos",
                    "indice": "porTurma", "chave": [turma["id"]], "max": 500})
    confere("e o BEFORE normalizou dentro do lote",
            "ITAJAI" in {l["cidade"] for l in amostra["linhas"]}, True)
    # Uma auditoria por linha GRAVADA -- nem por linha recebida.
    confere("o AFTER disparou 199 vezes, uma por linha gravada",
            visiveis(c, "auditoria") - antes_aud, 199)

    print("\n=== 4d. a cadeia de gatilhos tem fundo ===\n")
    # O caso que derrubava o SERVIDOR INTEIRO: um AFTER INSERT que grava na
    # propria tabela chama a si mesmo sem parar, e o Rust aborta o processo com
    # "stack overflow". A guarda corta a cadeia e devolve um aviso.
    c.ok({"op": "criar_tabela", "database": DB, "tabela": "cadeia",
          "colunas": [{"nome": "n", "tipo": "Int8"},
                      {"nome": "quem", "tipo": "Str(20)"}]})
    c.sql("CREATE TRIGGER se_multiplica AFTER INSERT ON cadeia FOR EACH ROW "
          "INSERT INTO cadeia (n, quem) VALUES (NEW.n + 1, 'gatilho')")
    r = c.fala({"op": "inserir", "database": DB, "tabela": "cadeia",
                "linha": {"n": 1, "quem": "gente"}})
    confere("a conexao NAO caiu", r.get("caiu"), None)
    confere("e a resposta e ok (o AFTER que falha e aviso)", r.get("ok"), True)
    avisos = (r.get("resultado") or {}).get("gatilhos_avisos") or []
    confere("com um aviso dizendo o que aconteceu", len(avisos) >= 1, True)
    if avisos:
        confere_contem("e o aviso nomeia a cadeia", avisos[0], "cadeia de gatilhos")
    confere("o servidor continua atendendo",
            c.ok({"op": "ping"})["phxsql"] != "", True)
    quantas = visiveis(c, "cadeia")
    nota(f"a cadeia parou em {quantas} linhas -- e o teto, nao o infinito")
    confere("e ela parou mesmo (menos de 100 linhas)", quantas < 100, True)
    c.sql("DROP TRIGGER se_multiplica")

    print("\n=== 4e. o comportamento VELHO: tabela sem gatilho grava igual ===\n")
    # Com gatilho ligado em OUTRA tabela -- que e justamente quando o portao
    # esta verdadeiro e a procura acontece.
    antes = visiveis(c, "sem_gatilho")
    r = c.ok({"op": "inserir", "database": DB, "tabela": "sem_gatilho",
              "linha": {"id": "novo", "cidade": "  blumenau "}})
    confere("a resposta tem as chaves de sempre e so elas",
            sorted(r.keys()), ["registros", "rowid"])
    lida = c.ok({"op": "ler", "database": DB, "tabela": "sem_gatilho",
                 "rowid": r["rowid"]})
    confere("e o dado entrou CRU, sem gatilho de outra tabela mexer",
            lida["cidade"], "  blumenau ")
    confere("uma linha a mais", visiveis(c, "sem_gatilho"), antes + 1)

    print("\n=== 4f. SHOW TRIGGERS, e o DROP volta ao cru ===\n")
    mostrados = c.sql("SHOW TRIGGERS")["gatilhos"]
    confere("os tres gatilhos aparecem",
            sorted(g["nome"] for g in mostrados),
            ["audita", "exige_nota", "normaliza"])
    # O campo `quebrado` so aparece quando ha motivo: ausente e o normal.
    confere("nenhum quebrado", [g["nome"] for g in mostrados if "quebrado" in g], [])


def item_4g_sobrevive_ao_reinicio(c):
    print("\n=== 4g. os gatilhos sobrevivem ao REINICIO do processo ===\n")
    mostrados = c.sql("SHOW TRIGGERS")["gatilhos"]
    confere("os tres continuam la depois de subir de novo",
            sorted(g["nome"] for g in mostrados),
            ["audita", "exige_nota", "normaliza"])
    turma = c.ok({"op": "varrer", "database": DB, "tabela": "turmas",
                  "max": 10})["linhas"][0]
    neg = c.fala({"op": "inserir", "database": DB, "tabela": "alunos",
                  "linha": {"id": "novo", "turma_id": turma["id"],
                            "nome": "depois do reinicio", "cidade": "x"}})
    confere_contem("e a regra do dono ainda recusa", neg.get("erro"),
                   "aluno sem nota nao entra")


# ======================================================================== 5

def item_5_procedimentos(c):
    print("\n=== 5a. procedimento com IN, OUT e WHILE ===\n")
    c.sql("CREATE PROCEDURE somar(IN ate INT, OUT total INT) BEGIN "
          "  DECLARE i INT DEFAULT 1; "
          "  SET total = 0; "
          "  WHILE i <= ate DO SET total = total + i; SET i = i + 1; END WHILE; "
          "END")
    confere("soma de 1 a 100", c.sql("CALL somar(100)")["saida"]["total"], 5050)
    confere("soma de 1 a 10", c.sql("CALL somar(10)")["saida"]["total"], 55)

    print("\n=== 5b. procedimento le o motor com SELECT ... INTO ===\n")
    # O corpo le POR INDICE. Um WHERE sobre coluna sem indice e recusado pelo
    # NOME, dentro da rotina como fora dela -- a camada SELECT nao varre a
    # tabela inteira escondendo o custo, e a rotina nao ganha um atalho.
    sem_indice = c.fala({"op": "sql", "database": DB,
                         "texto": "SELECT nome FROM alunos WHERE nome = 'Ana'"})
    confere("WHERE sem indice e recusado", sem_indice.get("ok"), False)
    confere_contem("dizendo o que falta", sem_indice.get("erro"), "exige um indice")

    ana = [l for l in c.ok({"op": "varrer", "database": DB, "tabela": "alunos",
                            "max": 500})["linhas"] if l["nome"] == "Ana"][0]
    c.sql("CREATE PROCEDURE resumo(IN qual VARCHAR(40), OUT quantos INT, "
          "                        OUT quem VARCHAR(60)) BEGIN "
          "  SELECT COUNT(*) INTO quantos FROM alunos; "
          "  SELECT nome INTO quem FROM alunos WHERE id = qual; "
          "END")
    saida = c.sql(f"CALL resumo('{ana['id']}')")["saida"]
    # COUNT(*) contra COUNT(*), e nao contra a varredura: nao sao a mesma
    # pergunta. O COUNT(*) sai do cabecalho em O(1) e conta o slot da linha
    # excluida SUAVEMENTE; a varredura nao a mostra. Esta prova ja apanhou por
    # comparar as duas como se fossem uma.
    direto = c.sql("SELECT COUNT(*) FROM alunos")["contagem"]
    confere("o COUNT(*) chegou ao OUT", saida["quantos"], direto)
    confere("e o SELECT ... INTO trouxe a linha", saida["quem"], "Ana")

    print("\n=== 5c. procedimento GRAVA, e pelo portao de sempre ===\n")
    c.sql("CREATE PROCEDURE matricular(IN quem VARCHAR(60), IN onde VARCHAR(40)) "
          "INSERT INTO auditoria (evento, quem) "
          "VALUES (CONCAT('matricula de ', quem), onde)")
    antes = visiveis(c, "auditoria")
    c.sql("CALL matricular('Carlos', 'secretaria')")
    confere("gravou uma linha", visiveis(c, "auditoria"), antes + 1)
    ult = c.ok({"op": "varrer", "database": DB, "tabela": "auditoria",
                "max": 5000})["linhas"][-1]
    confere("com o que o corpo montou", (ult["evento"], ult["quem"]),
            ("matricula de Carlos", "secretaria"))

    print("\n=== 5d. CALL nao e a porta dos fundos para a tabela negada ===\n")
    # O Pedro opera a base inteira MENOS `alunos`. As duas metades importam:
    # o que ele NAO pode continua nao podendo por dentro do CALL, e o que ele
    # PODE continua podendo -- senao a regra nova tiraria direito de alguem.
    pedro = Cliente("pedro")
    negado = pedro.fala({"op": "varrer", "database": DB, "tabela": "alunos",
                         "max": 10})
    confere("pela porta da frente, alunos e negado a ele", negado.get("ok"), False)
    confere_contem("com o motivo escrito", negado.get("erro"), "alunos")

    # O `resumo` LE alunos. Chamado pelo Pedro, tem de morrer no mesmo portao.
    porta = pedro.fala({"op": "sql", "database": DB,
                        "texto": f"CALL resumo('{ana['id']}')"})
    confere("e pelo CALL tambem", porta.get("ok"), False)
    confere_contem("com o mesmo motivo", porta.get("erro"), "alunos")

    # O `matricular` grava em `auditoria`, que ele PODE. Continua podendo.
    antes = visiveis(c, "auditoria")
    pode = pedro.fala({"op": "sql", "database": DB,
                       "texto": "CALL matricular('Pedro', 'pedro')"})
    confere("o que ele pode, ele continua podendo pelo CALL", pode.get("ok"), True)
    confere("e a linha entrou", visiveis(c, "auditoria"), antes + 1)

    # Criar rotina exige administrar -- com `criar` bastando, quem pode criar
    # tabela penduraria um AFTER INSERT na tabela alheia.
    cria = pedro.fala({"op": "sql", "database": DB,
                       "texto": "CREATE PROCEDURE minha() "
                                "INSERT INTO auditoria (evento) VALUES ('x')"})
    confere("mas criar rotina exige administrar", cria.get("ok"), False)
    confere_contem("e a recusa diz qual poder falta", cria.get("erro"),
                   "administrar")
    ver = pedro.fala({"op": "sql", "database": DB, "texto": "SHOW PROCEDURES"})
    confere("listar tambem", ver.get("ok"), False)
    pedro.fechar()


# ======================================================================== 6

def carga(c, tabela, quantas, turma_id, por_lote=1000):
    """Grava `quantas` linhas em lotes, e devolve os segundos so da gravacao."""
    total = 0.0
    for inicio in range(0, quantas, por_lote):
        fim = min(inicio + por_lote, quantas)
        linhas = [{"id": "novo", "turma_id": turma_id, "nome": f"aluno {i}",
                   "cidade": "blumenau", "nota": "7.50"}
                  for i in range(inicio, fim)]
        t0 = time.perf_counter()
        r = c.ok({"op": "inserir_lote", "database": DB, "tabela": tabela,
                  "parar_no_erro": True, "linhas": linhas})
        total += time.perf_counter() - t0
        if r["gravadas"] != fim - inicio:
            raise SystemExit(f"a carga em {tabela} gravou {r['gravadas']} de "
                             f"{fim - inicio}: {r.get('erros')}")
    return total


def tabela_de_carga(c, nome, tipo_da_chave="Uuid"):
    c.ok({"op": "criar_tabela", "database": DB, "tabela": nome,
          "colunas": [{"nome": "id", "tipo": tipo_da_chave, "obrigatoria": True},
                      {"nome": "turma_id", "tipo": "Uuid", "obrigatoria": True},
                      {"nome": "nome", "tipo": "Str(60)"},
                      {"nome": "cidade", "tipo": "Str(40)"},
                      {"nome": "nota", "tipo": "Decimal(5,2)"}],
          "indices": [{"nome": "porId", "colunas": ["id"],
                       "unico": True, "primario": True},
                      {"nome": "porTurma", "colunas": ["turma_id"]}]})


def item_6_carga_de_5000(c, rodadas):
    print("\n=== 6. a carga de 5.000 registros, medida ===\n")
    QUANTAS = 5000
    turma = c.ok({"op": "varrer", "database": DB, "tabela": "turmas",
                  "max": 10})["linhas"][0]["id"]

    # Os quatro cenarios fazem O MESMO TRABALHO -- as mesmas 5.000 linhas, a
    # mesma tabela, os mesmos dois indices -- e mudam SO o gatilho. Cada rodada
    # usa uma tabela NOVA, para nenhum cenario herdar a arvore quente do
    # anterior; e as rodadas sao INTERCALADAS, porque medir em bloco poe toda a
    # deriva da maquina dentro de um cenario e a chama de custo dele.
    CENARIOS = [
        ("sem gatilho", None, None),
        ("BEFORE que normaliza um campo",
         "SET NEW.cidade = UPPER(TRIM(NEW.cidade))", None),
        ("AFTER que so calcula (nao fala com o motor)", None,
         "BEGIN DECLARE v INT DEFAULT 1; SET v = v + 1; END"),
        ("AFTER que grava auditoria (1 INSERT derivado por linha)", None,
         "INSERT INTO auditoria (evento, quem) VALUES (NEW.nome, 'carga')"),
    ]
    medidas = {n: [] for n, _, _ in CENARIOS}
    for r in range(rodadas):
        for i, (nome, antes, depois) in enumerate(CENARIOS):
            t = f"carga_{i}_{r}"
            tabela_de_carga(c, t)
            if antes:
                c.sql(f"CREATE TRIGGER g_{i}_{r}_a BEFORE INSERT ON {t} "
                      f"FOR EACH ROW {antes}")
            if depois:
                c.sql(f"CREATE TRIGGER g_{i}_{r}_d AFTER INSERT ON {t} "
                      f"FOR EACH ROW {depois}")
            s = carga(c, t, QUANTAS, turma)
            medidas[nome].append(s)
            c.ok({"op": "excluir_tabela", "database": DB, "tabela": t,
                  "confirmar": t})

    # E a linha a linha, uma vez: e o controle contra o numero que a
    # bancada/carga ja publica.
    t = "carga_uma_a_uma"
    tabela_de_carga(c, t)
    t0 = time.perf_counter()
    for i in range(QUANTAS):
        c.ok({"op": "inserir", "database": DB, "tabela": t,
              "linha": {"id": "novo", "turma_id": turma, "nome": f"aluno {i}",
                        "cidade": "blumenau", "nota": "7.50"}})
    uma_a_uma = time.perf_counter() - t0
    confere("as 5.000 uma a uma entraram", visiveis(c, t), QUANTAS)
    c.ok({"op": "excluir_tabela", "database": DB, "tabela": t, "confirmar": t})

    def mediana(v):
        v = sorted(v)
        return v[len(v) // 2] if len(v) % 2 else (v[len(v) // 2 - 1] + v[len(v) // 2]) / 2

    print(f"\n  5.000 linhas, 2 indices, {rodadas} rodadas intercaladas\n")
    print(f"  {'cenario':<52} {'mediana':>9} {'linhas/s':>9} {'µs/linha':>9} {'espalha':>9}")
    linhas_do_json = []
    for nome, _, _ in CENARIOS:
        v = medidas[nome]
        m = mediana(v)
        espalha = max(v) - min(v)
        print(f"  {nome:<52} {m:8.3f}s {QUANTAS / m:9.0f} "
              f"{m * 1e6 / QUANTAS:9.2f} {espalha * 1e6 / QUANTAS:8.2f}")
        linhas_do_json.append({
            "cenario": nome, "s": round(m, 4),
            "linhas_por_s": round(QUANTAS / m),
            "us_por_linha": round(m * 1e6 / QUANTAS, 2),
            "espalhamento_us": round(espalha * 1e6 / QUANTAS, 2),
            "todas_s": [round(x, 4) for x in v]})
    print(f"  {'uma a uma (op inserir, 5.000 viagens)':<52} {uma_a_uma:8.3f}s "
          f"{QUANTAS / uma_a_uma:9.0f} {uma_a_uma * 1e6 / QUANTAS:9.2f}")

    # A regra da casa: a diferenca so vira numero se for maior que o ruido.
    base = mediana(medidas["sem gatilho"])
    maior_espalhamento = max(
        (max(v) - min(v)) * 1e6 / QUANTAS for v in medidas.values())
    print()
    for nome, _, _ in CENARIOS[1:]:
        d = (mediana(medidas[nome]) - base) * 1e6 / QUANTAS
        veredito = ("acima do ruido" if abs(d) > maior_espalhamento
                    else "NAO aparece acima do ruido")
        print(f"  {nome:<52} {d:+8.2f} µs/linha   {veredito}")
    nota(f"maior espalhamento dentro de um cenario: {maior_espalhamento:.2f} µs/linha "
         "-- e a regua com que as diferencas acima se leem")
    return {"quantas": QUANTAS, "rodadas": rodadas, "cenarios": linhas_do_json,
            "uma_a_uma_s": round(uma_a_uma, 4),
            "uma_a_uma_por_s": round(QUANTAS / uma_a_uma),
            "maior_espalhamento_us": round(maior_espalhamento, 2)}


def medir_a_chave(c, rodadas, quantas=100_000):
    """v7 contra v4 contra Sequence: a hipotese do proprio uuid.rs, medida.

    O modulo diz que chave ALEATORIA espalha a insercao por folhas diferentes
    da B+tree e chave CRESCENTE cai sempre na folha da direita. A afirmacao
    nunca tinha sido medida AQUI, com o mesmo trabalho dos dois lados.

    E a medicao tem de ser em DUAS escalas, senao ela responde a pergunta
    errada: enquanto o `.ndx` inteiro cabe na cache de paginas, espalhar nao
    custa quase nada -- a folha "longe" tambem esta na memoria. O que a
    hipotese diz e que o custo aparece quando a arvore deixa de caber. Uma
    escala so mediria a cache, e chamaria isso de chave."""
    print(f"\n=== 6b. a chave: v7 contra v4 contra Sequence, {quantas:,} linhas ===\n"
          .replace(",", "."))
    turma = c.ok({"op": "varrer", "database": DB, "tabela": "turmas",
                  "max": 10})["linhas"][0]["id"]
    CH = [("Uuid v7 (crescente)", "Uuid", "novo"),
          ("Uuid v4 (sorteado)", "Uuid", "v4"),
          ("Sequence (o motor numera)", "Sequence", None)]
    medidas = {n: [] for n, _, _ in CH}
    for r in range(rodadas):
        for i, (nome, tipo, palavra) in enumerate(CH):
            t = f"chave_{quantas}_{i}_{r}"
            tabela_de_carga(c, t, tipo)
            total = 0.0
            for ini in range(0, quantas, 5000):
                fim = min(ini + 5000, quantas)
                linhas = []
                for k in range(ini, fim):
                    l = {"turma_id": turma, "nome": f"n {k}",
                         "cidade": "blumenau", "nota": "7.50"}
                    l["id"] = palavra if palavra else k + 1
                    linhas.append(l)
                t0 = time.perf_counter()
                c.ok({"op": "inserir_lote", "database": DB, "tabela": t,
                      "parar_no_erro": True, "linhas": linhas})
                total += time.perf_counter() - t0
            medidas[nome].append(total)
            c.ok({"op": "excluir_tabela", "database": DB, "tabela": t,
                  "confirmar": t})

    def mediana(v):
        v = sorted(v)
        return v[len(v) // 2] if len(v) % 2 else (v[len(v) // 2 - 1] + v[len(v) // 2]) / 2

    print(f"  {'chave primaria':<32} {'mediana':>9} {'linhas/s':>9} "
          f"{'µs/linha':>9} {'espalha':>9}")
    saida = []
    for nome, _, _ in CH:
        v = medidas[nome]
        m = mediana(v)
        print(f"  {nome:<32} {m:8.3f}s {quantas / m:9.0f} "
              f"{m * 1e6 / quantas:9.2f} {(max(v) - min(v)) * 1e6 / quantas:8.2f}")
        saida.append({"chave": nome, "s": round(m, 4),
                      "linhas_por_s": round(quantas / m),
                      "us_por_linha": round(m * 1e6 / quantas, 2),
                      "espalhamento_us": round((max(v) - min(v)) * 1e6 / quantas, 2),
                      "todas_s": [round(x, 4) for x in v]})
    v7, v4 = mediana(medidas[CH[0][0]]), mediana(medidas[CH[1][0]])
    seq = mediana(medidas[CH[2][0]])
    print(f"\n  v4 custa {v4 / v7:.2f}x o v7   |   v7 custa {v7 / seq:.2f}x a Sequence")
    return {"quantas": quantas, "rodadas": rodadas, "chaves": saida,
            "v4_sobre_v7": round(v4 / v7, 3), "v7_sobre_sequence": round(v7 / seq, 3)}


# ========================================================================

def provar_pela_tela(tiros):
    """A mesma bateria pelo NAVEGADOR, contra o servidor que ja esta no ar.

    Um processo a parte, e nao um `import`: o Playwright e Node, e o que ele
    prova nao se prova daqui -- que o `find` de uma coluna de sistema nao
    derrubou o formulario, que o recado do gatilho chega ao usuario, que a
    grade nao troca a caixa do dado."""
    node = "/opt/node22/bin/node"
    roteiro = os.path.join(AQUI, "prova-tela.mjs")
    if not os.path.exists(node):
        nota(f"sem {node}: a prova pela tela nao rodou")
        return
    print("\n" + "-" * 66 + "\n  a mesma bateria, agora pela TELA\n" + "-" * 66)
    ambiente = dict(os.environ, PLAYWRIGHT_BROWSERS_PATH="/opt/pw-browsers")
    r = subprocess.run(
        [node, roteiro, str(PORTA_WEB), str(PORTA), TOKEN, SENHAS["adm"], tiros],
        env=ambiente)
    if r.returncode != 0:
        falhas.append("a bateria da tela")


def main():
    if not os.path.exists(PHXSQLD):
        sys.exit(f"nao achei {PHXSQLD} -- rode `cargo build --release` antes")
    medir = "--medir" in sys.argv
    tela = "--tela" in sys.argv
    rodadas = 3
    if "--rodadas" in sys.argv:
        rodadas = int(sys.argv[sys.argv.index("--rodadas") + 1])
    tiros = ""
    if "--tiros" in sys.argv:
        tiros = sys.argv[sys.argv.index("--tiros") + 1]
        os.makedirs(tiros, exist_ok=True)

    srv = Servidor()
    resultados = {"quando": time.strftime("%Y-%m-%d"), "porta": PORTA}
    try:
        c = Cliente("adm")
        item_1_e_2_banco_e_tabelas(c)
        item_3_uuid_v7_e_1_para_n(c)
        item_4_gatilhos(c)
        c.fechar()
        # O reinicio prova o que teste unitario nao prova: que o cadastro de
        # rotinas mora no disco e volta.
        srv.parar()
        srv = Servidor(limpar=False)
        c = Cliente("adm")
        item_4g_sobrevive_ao_reinicio(c)
        item_5_procedimentos(c)
        if tela:
            # A tela roda ANTES da medicao: ela olha as tabelas do item 3, e a
            # medicao cria e apaga dezenas de tabelas de carga no meio.
            provar_pela_tela(tiros)
        if medir:
            resultados["carga"] = item_6_carga_de_5000(c, rodadas)
            resultados["chave"] = [medir_a_chave(c, rodadas, n)
                                   for n in (100_000, 1_000_000)]
        else:
            print("\n(sem --medir: a medicao do item 6 nao rodou)")
        c.fechar()
    finally:
        srv.parar()

    if medir:
        with open(os.path.join(AQUI, "resultados.json"), "w") as f:
            json.dump(resultados, f, indent=2, ensure_ascii=False)
        print(f"\nnumeros gravados em {os.path.join(AQUI, 'resultados.json')}")

    print("\n" + "=" * 66)
    if falhas:
        print(f"{len(falhas)} PASSO(S) FALHARAM:")
        for f_ in falhas:
            print("  -", f_)
        sys.exit(1)
    print("a bateria inteira passou")


if __name__ == "__main__":
    main()
