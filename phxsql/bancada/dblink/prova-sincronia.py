#!/usr/bin/env python3
"""A prova da sincronia de tabelas primas, contra um MySQL(R) de verdade.

O que ela prova, na ordem -- e cada passo tem o resultado esperado escrito
ANTES de rodar, que e o que separa prova de demonstracao:

1. `dblink_ligar` cria a tabela local espelhando a remota (tipos, chave);
2. a primeira rodada PUXA tudo (5 novas, 0 conflito);
3. linha nova AQUI vai para la; linha nova LA vem para ca;
4. a MESMA linha mudada dos dois lados: o DONO vence (aqui);
5. exclusao NAO viaja: a linha apagada aqui REAPARECE, vinda de la --
   e o limite documentado, e a prova confere que ele e verdade;
6. a rodada e reentravel: rodar de novo sem mudanca da 0/0/0.

Requisitos: mysqld local com o banco `crm` montado (ver DBLINK.md) e o
phxsqld do demo no ar (porta 5599, token segredo, adriano/demo123).
"""
import json, socket, subprocess, sys

def mysql(sql):
    subprocess.run(["mysql", "crm", "-e", sql], check=True, capture_output=True)

def mysql_uma(sql):
    r = subprocess.run(["mysql", "-N", "crm", "-e", sql], check=True, capture_output=True, text=True)
    return r.stdout.strip()

s = socket.create_connection(("127.0.0.1", 5599)); f = s.makefile("rwb")
def fala(p):
    p.setdefault("token", "segredo")
    f.write((json.dumps(p) + "\n").encode()); f.flush()
    r = json.loads(f.readline().decode())
    if not r.get("ok"):
        sys.exit(f"FALHOU {p['op']}: {r.get('erro')}")
    return r.get("resultado", r)

def sincroniza():
    return fala({"op": "dblink_sincronizar", "dblink": "crm"})["sincronizadas"][0]

def confere(rotulo, visto, esperado):
    ok = visto == esperado
    print(f"  {'ok ' if ok else 'ERRO'} {rotulo}: {visto}" + ("" if ok else f" (esperava {esperado})"))
    if not ok:
        sys.exit(1)

fala({"op": "login", "usuario": "adriano", "senha": "demo123"})

# O comeco conhecido: 5 linhas la, ligacao gravavel, espelho local limpo.
mysql("DELETE FROM clientes WHERE id > 5")
mysql("UPDATE clientes SET cidade='Blumenau', limite=4200.50 WHERE id=2")
fala({"op": "dblink_salvar", "nome": "crm", "motor": "mysql", "host": "127.0.0.1",
      "porta": 3306, "usuario": "phx", "senha": "ponte123", "database": "crm",
      "somente_leitura": False})
fala({"op": "excluir_tabela", "database": "espelho", "tabela": "clientes", "de_vez": True}) \
    if False else None

print("== 1. ligar cria a tabela local espelhando a remota ==")
r = fala({"op": "dblink_ligar", "dblink": "crm",
          "tabelas": [{"remota": "clientes", "local_database": "espelho",
                        "sentido": "dois", "dono": "aqui"}]})["ligadas"][0]
confere("chave detectada", r["chave"], "id")

print("== 2. a primeira rodada puxa tudo ==")
r = sincroniza()
confere("puxadas novas", r["puxadas_novas"], 5)
confere("conflitos", r["conflitos"], 0)

print("== 3. linha nova de cada lado atravessa ==")
fala({"op": "inserir", "database": "espelho", "tabela": "clientes",
      "valores": {"id": 100, "nome": "Novo Daqui", "cidade": "Curitiba",
                   "limite": "10.00", "desde": "2026-08-29"}})
mysql("INSERT INTO clientes VALUES (6,'Novo De La','Lages',55.00,'2026-08-29')")
r = sincroniza()
confere("veio de la", r["puxadas_novas"], 1)
confere("foi para la", r["empurradas"], 1)
confere("id 100 chegou la", mysql_uma("SELECT nome FROM clientes WHERE id=100"), "Novo Daqui")

print("== 4. conflito: o dono (aqui) vence ==")
mysql("UPDATE clientes SET cidade='ERRADA' WHERE id=1")
# O rowid local se ACHA pela chave -- supor que a ordem da puxada e a ordem
# dos ids foi o primeiro defeito que esta prova pegou, e era da prova.
achado = fala({"op": "buscar", "database": "espelho", "tabela": "clientes",
               "indice": "porChave", "chave": [1]})
rowid = achado["linhas"][0]["rowid"] if "linhas" in achado else achado["rowids"][0]
fala({"op": "atualizar", "database": "espelho", "tabela": "clientes", "rowid": rowid,
      "valores": {"id": 1, "nome": "Adriano Boller", "cidade": "Curitiba-PR",
                   "limite": "15000.00", "desde": "2019-03-12"}})
r = sincroniza()
confere("um conflito visto", r["conflitos"], 1)
confere("a linha do dono venceu la", mysql_uma("SELECT cidade FROM clientes WHERE id=1"), "Curitiba-PR")

print("== 5. exclusao NAO viaja: a linha apagada reaparece ==")
achado = fala({"op": "buscar", "database": "espelho", "tabela": "clientes",
               "indice": "porChave", "chave": [2]})
rowid = achado["linhas"][0]["rowid"] if "linhas" in achado else achado["rowids"][0]
fala({"op": "excluir", "database": "espelho", "tabela": "clientes", "rowid": rowid,
      "fisico": True, "motivo": "prova do limite documentado"})
r = sincroniza()
confere("ela voltou de la", r["puxadas_novas"], 1)

print("== 6. reentravel: sem mudanca, rodada vazia ==")
r = sincroniza()
confere("nada a puxar", r["puxadas_novas"] + r["puxadas_alteradas"], 0)
confere("nada a empurrar", r["empurradas"], 0)
confere("sete iguais", r["iguais"], 7)

print("== 7. o job roda a sincronia sozinho ==")
fala({"op": "job_salvar", "nome": "sincronia-crm",
      "descricao": "convergencia com o MySQL(R) da bancada",
      "cada_minutos": 5, "ligado": True, "usuario": "adriano",
      "pedido": {"op": "dblink_sincronizar", "dblink": "crm"}})
mysql("INSERT INTO clientes VALUES (7,'Chegou Pelo Job','Itajai',1.00,'2026-08-29')")
r = fala({"op": "job_rodar", "nome": "sincronia-crm"})
corrida = r.get("resultado", r)
print(f"  ok  job rodou: {json.dumps(corrida, ensure_ascii=False)[:100]}")
achado = fala({"op": "buscar", "database": "espelho", "tabela": "clientes",
               "indice": "porChave", "chave": [7]})
confere("a linha do job chegou aqui", achado["linhas"][0]["nome"], "Chegou Pelo Job")

print("\nPROVA COMPLETA: os dois lados convergem, o dono vence, o limite da")
print("exclusao e real, e o job faz a rodada sozinho -- cada afirmacao acima")
print("foi conferida contra o resultado, nao so impressa.")
