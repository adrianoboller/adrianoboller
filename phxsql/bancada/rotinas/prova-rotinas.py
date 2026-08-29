#!/usr/bin/env python3
"""A prova dos gatilhos e dos procedimentos, pelo SOQUETE.

    cargo build --release
    python3 bancada/rotinas/prova-rotinas.py

O que ela prova, na ordem -- e cada passo tem o resultado esperado escrito
ANTES de rodar, que e o que separa prova de demonstracao:

1.  BEFORE INSERT normaliza um campo: entra "  blumenau ", grava "BLUMENAU";
2.  SIGNAL recusa e a linha NAO entra -- a contagem prova, e o erro chega ao
    cliente com a MESSAGE_TEXT, o codigo 3005 e o nome SINAL;
3.  AFTER INSERT grava auditoria noutra tabela, com o NEW como ele FICOU;
4.  UPDATE ve OLD e NEW; DELETE ve OLD e o SIGNAL protege a linha;
5.  procedimento com IN/OUT e WHILE somando devolve 5050 no OUT;
6.  procedimento le o motor com SELECT ... INTO, inclusive COUNT(*);
7.  o lote passa pelo BEFORE linha a linha: a recusada vira erro COM A
    POSICAO e as outras entram;
8.  falha de AFTER nao desfaz a escrita -- vira aviso numa resposta ok,
    porque nao ha transacao (e este e o limite honesto do desenho);
9.  as rotinas sobrevivem ao REINICIO do servidor (gatilhos.json);
10. DROP dos dois, e a escrita volta a ser a crua;
11. **o comportamento velho**: tabela sem gatilho grava exatamente como
    antes, ATE com gatilho em outra tabela -- que e quando o portao esta
    ligado e a procura acontece. E a forma da resposta nao muda.

Sobe um phxsqld PROPRIO nas portas 5301 (dados) e 5701 (web), e mata SO o
processo que ele mesmo criou, pelo PID. Nunca toca em phxsqld de outra
pessoa -- ha um demo no ar em 5199/5599 que nao e nosso.
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
BASE = "/tmp/phx-prova-rotinas"
PORTA, PORTA_WEB = 5301, 5701
TOKEN = "rotinas"

falhas = []


def confere(rotulo, visto, esperado):
    ok = visto == esperado
    print(f"  {'ok  ' if ok else 'ERRO'} {rotulo}: {visto!r}"
          + ("" if ok else f"   (esperava {esperado!r})"))
    if not ok:
        falhas.append(rotulo)


def confere_contem(rotulo, texto, pedaco):
    ok = pedaco.lower() in (texto or "").lower()
    print(f"  {'ok  ' if ok else 'ERRO'} {rotulo}: {texto!r}"
          + ("" if ok else f"   (esperava conter {pedaco!r})"))
    if not ok:
        falhas.append(rotulo)


class Servidor:
    """Um phxsqld nosso. Morre pelo PID, e so ele."""

    def __init__(self, limpar):
        if limpar:
            shutil.rmtree(BASE, ignore_errors=True)
            os.makedirs(BASE, exist_ok=True)
            with open(os.path.join(BASE, "config.json"), "w") as f:
                json.dump({"base": "base", "bind": f"127.0.0.1:{PORTA}",
                           "token": TOKEN,
                           "web": {"ligado": True,
                                   "bind": f"127.0.0.1:{PORTA_WEB}"}}, f, indent=2)
        log = open(os.path.join(BASE, "servidor.log"), "a")
        self.proc = subprocess.Popen([PHXSQLD], cwd=BASE, stdout=log,
                                     stderr=subprocess.STDOUT,
                                     stdin=subprocess.DEVNULL)
        time.sleep(2)

    def parar(self):
        self.proc.terminate()
        self.proc.wait(timeout=10)


class Cliente:
    def __init__(self):
        self.s = socket.create_connection(("127.0.0.1", PORTA))
        self.f = self.s.makefile("rwb")

    def fala(self, p):
        p.setdefault("token", TOKEN)
        self.f.write((json.dumps(p) + "\n").encode())
        self.f.flush()
        return json.loads(self.f.readline().decode())

    def ok(self, p):
        r = self.fala(p)
        if not r.get("ok"):
            sys.exit(f"FALHOU {p.get('op')}: {r.get('erro')}")
        return r["resultado"]

    def sql(self, texto, database="loja"):
        return self.ok({"op": "sql", "database": database, "texto": texto})

    def sql_erro(self, texto, database="loja"):
        r = self.fala({"op": "sql", "database": database, "texto": texto})
        if r.get("ok"):
            sys.exit(f"o comando devia ter falhado: {texto}")
        return r

    def fechar(self):
        self.f.close()
        self.s.close()


def quantas(c, tabela):
    """Linhas que quem consulta VE -- o excluir suave tira da lista sem mexer
    no contador do cabecalho, entao `registros` mentiria aqui."""
    return len(c.ok({"op": "varrer", "database": "loja",
                     "tabela": tabela, "max": 500}).get("linhas", []))


def cidade_do(c, rowid):
    return c.ok({"op": "ler", "database": "loja", "tabela": "clientes",
                 "rowid": rowid})["cidade"]


def montar(c):
    c.ok({"op": "criar_database", "database": "loja"})
    c.ok({"op": "criar_tabela", "database": "loja", "tabela": "clientes",
          "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                      {"nome": "nome", "tipo": "Str(40)"},
                      {"nome": "cidade", "tipo": "Str(40)"},
                      {"nome": "limite", "tipo": "Decimal(15,2)"}],
          "indices": [{"nome": "porId", "colunas": ["id"], "unico": True,
                       "primario": True}]})
    c.ok({"op": "criar_tabela", "database": "loja", "tabela": "auditoria",
          "colunas": [{"nome": "evento", "tipo": "Str(80)"},
                      {"nome": "quem", "tipo": "Str(40)"}]})
    # A tabela que NUNCA tera gatilho: e ela que prova o comportamento velho.
    c.ok({"op": "criar_tabela", "database": "loja", "tabela": "sem_gatilho",
          "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                      {"nome": "cidade", "tipo": "Str(40)"}],
          "indices": [{"nome": "porId", "colunas": ["id"], "unico": True,
                       "primario": True}]})


def main():
    if not os.path.exists(PHXSQLD):
        sys.exit(f"nao achei {PHXSQLD} -- rode `cargo build --release` antes")
    srv = Servidor(limpar=True)
    try:
        c = Cliente()
        montar(c)

        print("\n=== 1. BEFORE INSERT normaliza o campo ===\n")
        c.sql("CREATE TRIGGER normaliza BEFORE INSERT ON clientes FOR EACH ROW "
              "SET NEW.cidade = UPPER(TRIM(NEW.cidade))")
        r = c.ok({"op": "inserir", "database": "loja", "tabela": "clientes",
                  "linha": {"id": 1, "nome": "Ana", "cidade": "  blumenau ",
                            "limite": "1500.00"}})
        confere("cidade normalizada", cidade_do(c, r["rowid"]), "BLUMENAU")
        confere("a resposta nao ganhou campo novo",
                "gatilhos_avisos" in r, False)

        print("\n=== 2. SIGNAL recusa, e a linha NAO entra ===\n")
        c.sql("CREATE TRIGGER exige_nome BEFORE INSERT ON clientes FOR EACH ROW "
              "IF NEW.nome IS NULL OR NEW.nome = '' THEN "
              "  SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'cliente sem nome nao entra'; "
              "END IF")
        antes = quantas(c, "clientes")
        neg = c.fala({"op": "inserir", "database": "loja", "tabela": "clientes",
                      "linha": {"id": 2, "nome": "", "cidade": "x"}})
        confere("recusou", neg.get("ok"), False)
        confere("nome do erro", neg.get("nome"), "SINAL")
        confere("codigo", neg.get("codigo"), 3005)
        confere("nao adianta repetir", neg.get("repetir"), False)
        confere_contem("a MESSAGE_TEXT chega ao cliente", neg.get("erro"),
                       "cliente sem nome nao entra")
        confere_contem("o SQLSTATE viaja junto", neg.get("erro"), "45000")
        confere("a linha recusada NAO entrou", quantas(c, "clientes"), antes)

        print("\n=== 3. AFTER INSERT audita noutra tabela ===\n")
        c.sql("CREATE TRIGGER audita AFTER INSERT ON clientes FOR EACH ROW "
              "INSERT INTO auditoria (evento, quem) "
              "VALUES (CONCAT('entrou ', NEW.nome, ' de ', NEW.cidade), 'gatilho')")
        r = c.ok({"op": "inserir", "database": "loja", "tabela": "clientes",
                  "linha": {"id": 3, "nome": "Maria", "cidade": " joinville",
                            "limite": "900.00"}})
        confere("sem aviso", "gatilhos_avisos" in r, False)
        linhas = c.ok({"op": "varrer", "database": "loja",
                       "tabela": "auditoria", "max": 10})["linhas"]
        confere("uma linha de auditoria", len(linhas), 1)
        # O NEW do AFTER e a linha como FICOU -- ja normalizada pelo BEFORE.
        confere("o AFTER ve o que o BEFORE gravou", linhas[0]["evento"],
                "entrou Maria de JOINVILLE")

        print("\n=== 4. UPDATE ve OLD e NEW; DELETE ve OLD ===\n")
        c.sql("CREATE TRIGGER sem_renomear BEFORE UPDATE ON clientes FOR EACH ROW "
              "IF NEW.nome <> OLD.nome THEN "
              "  SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'nome nao se troca'; "
              "END IF")
        c.sql("CREATE TRIGGER sem_excluir BEFORE DELETE ON clientes FOR EACH ROW "
              "IF OLD.nome = 'Ana' THEN "
              "  SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'esta nao sai'; "
              "END IF")
        # Trocar a cidade pode.
        c.ok({"op": "atualizar", "database": "loja", "tabela": "clientes",
              "rowid": 1, "linha": {"id": 1, "nome": "Ana", "cidade": "Gaspar",
                                    "limite": "1500.00"}})
        confere("a cidade mudou", cidade_do(c, 1), "Gaspar")
        neg = c.fala({"op": "atualizar", "database": "loja", "tabela": "clientes",
                      "rowid": 1, "linha": {"id": 1, "nome": "Outra",
                                            "cidade": "Gaspar", "limite": "1500.00"}})
        confere_contem("trocar o nome recusa", neg.get("erro"), "nome nao se troca")
        antes = quantas(c, "clientes")
        neg = c.fala({"op": "excluir", "database": "loja", "tabela": "clientes",
                      "rowid": 1})
        confere_contem("excluir a protegida recusa", neg.get("erro"), "esta nao sai")
        confere("e ela continua la", quantas(c, "clientes"), antes)
        # A outra sai normalmente.
        c.ok({"op": "excluir", "database": "loja", "tabela": "clientes", "rowid": 2})
        confere("a comum sai", quantas(c, "clientes"), antes - 1)

        print("\n=== 5. procedimento com IN, OUT e WHILE ===\n")
        c.sql("CREATE PROCEDURE somar(IN ate INT, OUT total INT) BEGIN "
              "  DECLARE i INT DEFAULT 1; "
              "  SET total = 0; "
              "  WHILE i <= ate DO "
              "    SET total = total + i; "
              "    SET i = i + 1; "
              "  END WHILE; "
              "END")
        confere("soma de 1 a 100", c.sql("CALL somar(100)")["saida"]["total"], 5050)
        confere("soma de 1 a 10", c.sql("CALL somar(10)")["saida"]["total"], 55)

        print("\n=== 6. procedimento le o motor com SELECT ... INTO ===\n")
        c.sql("CREATE PROCEDURE resumo(IN qual INT, OUT quantos INT, "
              "                        OUT quem VARCHAR(40)) BEGIN "
              "  SELECT COUNT(*) INTO quantos FROM clientes; "
              "  SELECT nome INTO quem FROM clientes WHERE id = qual; "
              "END")
        saida = c.sql("CALL resumo(3)")["saida"]
        # Compara COUNT(*) com COUNT(*), e nao com a varredura: os dois nao
        # sao a mesma pergunta. O COUNT(*) sai do CABECALHO em O(1) e conta o
        # slot da linha excluida SUAVEMENTE; a varredura nao a mostra. Este
        # passo ja apanhou por isso -- e a divergencia e pre-existente e
        # documentada do COUNT(*), nao dos gatilhos.
        direto = c.sql("SELECT COUNT(*) FROM clientes")["contagem"]
        confere("o COUNT(*) chegou ao OUT", saida["quantos"], direto)
        confere("a variavel do WHERE virou literal", saida["quem"], "Maria")
        print(f"  nota  COUNT(*)={direto} e a varredura ve {quantas(c, 'clientes')}: "
              "a exclusao suave deixa o slot no cabecalho")

        print("\n=== 7. o lote passa pelo BEFORE linha a linha ===\n")
        r = c.ok({"op": "inserir_lote", "database": "loja", "tabela": "clientes",
                  "parar_no_erro": False,
                  "linhas": [{"id": 10, "nome": "Bia", "cidade": "  itajai "},
                             {"id": 11, "nome": "", "cidade": "x"},
                             {"id": 12, "nome": "Caio", "cidade": " navegantes"}]})
        confere("duas gravadas", r["gravadas"], 2)
        confere("uma recusada", r["recusadas"], 1)
        confere("a POSICAO da recusada", r["erros"][0]["linha"], 2)
        confere_contem("com o motivo do SIGNAL", r["erros"][0]["erro"],
                       "cliente sem nome nao entra")
        confere("e a normalizacao valeu para as que entraram",
                cidade_do(c, r["primeiro_rowid"]), "ITAJAI")

        print("\n=== 8. falha de AFTER nao desfaz a escrita (nao ha transacao) ===\n")
        c.sql("DROP TRIGGER audita")
        c.sql("CREATE TRIGGER audita_quebrada AFTER INSERT ON clientes FOR EACH ROW "
              "INSERT INTO tabela_que_nao_existe (a) VALUES (1)")
        antes = quantas(c, "clientes")
        r = c.fala({"op": "inserir", "database": "loja", "tabela": "clientes",
                    "linha": {"id": 20, "nome": "Dora", "cidade": "Brusque"}})
        confere("a resposta e ok", r.get("ok"), True)
        confere("a escrita FICOU", quantas(c, "clientes"), antes + 1)
        confere_contem("e o aviso diz qual gatilho falhou",
                       json.dumps(r["resultado"].get("gatilhos_avisos")),
                       "audita_quebrada")
        c.sql("DROP TRIGGER audita_quebrada")

        print("\n=== 9. as rotinas sobrevivem ao reinicio ===\n")
        arquivo = os.path.join(BASE, "base", "loja", "gatilhos.json")
        confere("o gatilhos.json existe", os.path.isfile(arquivo), True)
        c.fechar()
        srv.parar()
        srv = Servidor(limpar=False)
        c = Cliente()
        confere("os gatilhos voltaram",
                c.sql("SHOW TRIGGERS")["total"], 4)
        confere("os procedimentos voltaram",
                c.sql("SHOW PROCEDURES")["total"], 2)
        r = c.ok({"op": "inserir", "database": "loja", "tabela": "clientes",
                  "linha": {"id": 30, "nome": "Eva", "cidade": " gaspar "}})
        confere("e o BEFORE continua normalizando depois do reinicio",
                cidade_do(c, r["rowid"]), "GASPAR")

        print("\n=== 10. DROP tira, e a escrita volta a ser a crua ===\n")
        for nome in ["normaliza", "exige_nome", "sem_renomear", "sem_excluir"]:
            c.sql(f"DROP TRIGGER {nome}")
        c.sql("DROP PROCEDURE somar")
        c.sql("DROP PROCEDURE resumo")
        confere("nenhum gatilho", c.sql("SHOW TRIGGERS")["total"], 0)
        confere("nenhum procedimento", c.sql("SHOW PROCEDURES")["total"], 0)
        confere("o gatilhos.json sumiu junto", os.path.exists(arquivo), False)
        r = c.ok({"op": "inserir", "database": "loja", "tabela": "clientes",
                  "linha": {"id": 40, "nome": "", "cidade": " minusculo "}})
        confere("sem gatilho, o texto entra cru", cidade_do(c, r["rowid"]),
                " minusculo ")
        e = c.sql_erro("DROP TRIGGER normaliza")
        confere_contem("o segundo DROP recusa", e.get("erro"), "nao existe")
        c.sql("DROP TRIGGER IF EXISTS normaliza")
        print("  ok   IF EXISTS passa em silencio")

        print("\n=== 11. O COMPORTAMENTO VELHO: tabela sem gatilho ===\n")
        # O gatilho vive na `clientes`; a `sem_gatilho` nunca teve um. E com
        # ele ligado o portao atomico esta VERDADEIRO -- que e justamente
        # quando a procura acontece e alguem poderia errar a tabela.
        c.sql("CREATE TRIGGER so_na_clientes BEFORE INSERT ON clientes "
              "FOR EACH ROW SET NEW.cidade = UPPER(NEW.cidade)")
        r = c.ok({"op": "inserir", "database": "loja", "tabela": "sem_gatilho",
                  "linha": {"id": 1, "cidade": "  blumenau "}})
        confere("a forma da resposta do inserir NAO mudou",
                sorted(r.keys()), ["registros", "rowid"])
        linha = c.ok({"op": "ler", "database": "loja", "tabela": "sem_gatilho",
                      "rowid": r["rowid"]})
        confere("o dado entrou EXATAMENTE como veio", linha["cidade"],
                "  blumenau ")
        r = c.ok({"op": "atualizar", "database": "loja", "tabela": "sem_gatilho",
                  "rowid": 1, "linha": {"id": 1, "cidade": "outra"}})
        confere("a forma do atualizar NAO mudou",
                sorted(r.keys()), ["rowid", "versao"])
        r = c.ok({"op": "excluir", "database": "loja", "tabela": "sem_gatilho",
                  "rowid": 1})
        confere("a forma do excluir NAO mudou", sorted(r.keys()),
                ["excluido", "modo", "na_lixeira", "reversivel", "rowid"])
        r = c.ok({"op": "inserir_lote", "database": "loja",
                  "tabela": "sem_gatilho",
                  "linhas": [{"id": 100 + i, "cidade": " x "} for i in range(50)]})
        confere("o lote sem gatilho grava tudo", r["gravadas"], 50)
        confere("e sem aviso nenhum", "gatilhos_avisos" in r, False)

        c.fechar()
    finally:
        srv.parar()

    print("\n" + "=" * 60)
    if falhas:
        print(f"FALHOU em {len(falhas)}: {falhas}")
        sys.exit(1)
    print("RESULTADO " + json.dumps({
        "before_normaliza": True, "signal_cancela": True,
        "after_audita": True, "old_em_update_e_delete": True,
        "procedimento_in_out_while": True, "select_into": True,
        "lote_por_linha": True, "after_falho_vira_aviso": True,
        "sobrevive_ao_reinicio": True, "drop_volta_ao_cru": True,
        "comportamento_velho_intacto": True,
    }))


if __name__ == "__main__":
    main()
