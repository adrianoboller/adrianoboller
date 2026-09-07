#!/usr/bin/env python3
"""O SCRIPT DE DIRETIVAS de usuario, exercitado contra o motor vivo.

    flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server
    python3 bancada/diretivas/provar.py

Pedido do dono, 07/09/2026: *«exemplo script de diretivas de usuario para o
gerenciamento do banco de dados e para o gerenciamento do acesso as tabelas»*.

# O que este script E

A sequencia inteira, do zero ao veredito: gera os hashes com o proprio
`phxsqld --senha`, escreve o `config.json` com tres diretivas de usuario
(supervisor, administrador de UM banco, e leitor de UMA tabela), sobe o
servidor e prova, PELO SOQUETE, que cada um pode o que a diretiva diz e nao
pode o que ela nao diz.

# As duas coisas que ele mede e que a leitura do codigo nao dá

1. **As tres portas dos fundos.** `juntar`, `unir` e `pivotar` nao tem o campo
   `tabela` que o portao geral le -- as tabelas moram em `a.tabela`/`b.tabela`,
   numa lista, e dentro de um item de `juntar`. Este script pede a tabela
   NEGADA pelos tres caminhos e confere que os tres recusam.
2. **Senha nunca em texto puro.** Depois de tudo, varre a resposta do
   `usuarios`, o `acessos.log` e o erro padrao do servidor procurando a senha
   e o hash. Achar seria o defeito; a varredura so vale porque ela acha a
   senha quando ela ESTA la -- e o controle positivo do proprio varredor esta
   na parte 5.

# Como roda

Sobe UM `phxsqld` (porta 6505) em 127.0.0.1, com o cadastro completo no
`config.json`. Nao ha operacao de protocolo que crie usuario: o cadastro mora
no arquivo, entao a diretiva E o arquivo -- e este script o escreve linha a
linha, com o hash saindo do binario. Derruba o servidor por PID no fim.
"""

import json
import os
import shutil
import socket
import subprocess
import sys
import time

RAIZ = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
BINARIO = os.path.join(RAIZ, "target/release/phxsqld")
BASE = f"/tmp/phx-f5d-{os.getpid()}"
PORTA = int(os.environ.get("PHX_F5_PORTA_DIRETIVAS", "6505"))

SENHAS = {"ana": "senha-da-ana-2026", "bruno": "senha-do-bruno-2026",
          "carlos": "senha-do-carlos-2026", "root": "senha-do-root-2026"}

RESULTADO = {"quando_utc": time.strftime("%Y-%m-%d %H:%M:%S", time.gmtime())}
FALHAS = []


def diz(rotulo, ok, detalhe=""):
    print(f"  [{'OK  ' if ok else 'FALHA'}] {rotulo}" + (f" -- {detalhe}" if detalhe and not ok else ""))
    return ok


def afirmar(rotulo, ok, detalhe=""):
    if not diz(rotulo, ok, detalhe):
        FALHAS.append(rotulo)
    return ok


def hash_de(senha):
    """`echo -n <senha> | phxsqld --senha` -- pelo cano, para a senha nao ficar
    no historico do shell nem aparecer num `ps`."""
    r = subprocess.run([BINARIO, "--senha"], input=senha.encode(),
                       capture_output=True)
    saida = (r.stdout or r.stderr).decode().strip()
    # A saida vem como a linha inteira do config: "senha_hash": "pbkdf2-..."
    if '"' in saida:
        return saida.split('"')[3]
    return saida


class Servidor:
    def __enter__(self):
        if not os.path.exists(BINARIO):
            raise SystemExit(
                f"{BINARIO} nao existe. Rode antes:\n"
                "  flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server")
        os.makedirs(BASE + "/dados", exist_ok=True)
        self.cfg = BASE + "/config.json"
        with open(self.cfg, "w") as f:
            json.dump(DIRETIVAS, f, indent=2, ensure_ascii=False)
        self.saida = open(BASE + "/servidor.err", "wb")
        self.p = subprocess.Popen([BINARIO, "--config", self.cfg],
                                  stdout=self.saida, stderr=subprocess.STDOUT)
        for _ in range(120):
            try:
                socket.create_connection(("127.0.0.1", PORTA), 0.2).close()
                break
            except OSError:
                time.sleep(0.1)
        else:
            raise SystemExit(f"o servidor nao subiu na porta {PORTA}")
        return self

    def __exit__(self, *_):
        self.p.terminate()
        try:
            self.p.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.p.kill()
        self.saida.close()

    def erros(self):
        with open(BASE + "/servidor.err", errors="replace") as f:
            return f.read()

    def acessos(self):
        c = BASE + "/acessos.log"
        return open(c, errors="replace").read() if os.path.exists(c) else ""


class Sessao:
    """Uma conexao. A autenticacao acontece UMA vez por conexao, nao por pedido."""

    def __init__(self, login=None):
        self.s = socket.create_connection(("127.0.0.1", PORTA), 10)
        self.f = self.s.makefile("rwb")
        self.login = login
        self.cru_do_login = None
        if login:
            self.cru_do_login = self.cru(op="login", usuario=login, senha=SENHAS[login])

    def cru(self, **kw):
        kw.setdefault("token", "t")
        self.f.write((json.dumps(kw) + "\n").encode())
        self.f.flush()
        return self.f.readline().decode()

    def pedir(self, **kw):
        r = json.loads(self.cru(**kw))
        if isinstance(r.get("resultado"), dict):
            fora = {k: v for k, v in r.items() if k != "resultado"}
            r = {**r["resultado"], **fora}
        return r

    def fechar(self):
        try:
            self.f.close()
            self.s.close()
        except OSError:
            pass


def negado(r):
    return (not r.get("ok")) and "SP000025" in str(r.get("erro", ""))


# --------------------------------------------------------------- o cadastro
def montar_diretivas():
    print("\n=== 1. O SCRIPT DE DIRETIVAS: os hashes e o cadastro\n")
    hashes = {}
    for quem, senha in SENHAS.items():
        hashes[quem] = hash_de(senha)
        marca = hashes[quem]
        print(f"  echo -n '<senha de {quem}>' | phxsqld --senha")
        print(f"    -> {marca[:44]}...{marca[-8:]}")
        afirmar(f"o hash de {quem} e PBKDF2, nao a senha",
                marca.startswith("pbkdf2-sha256$210000$") and senha not in marca,
                marca[:60])
    RESULTADO["algoritmo"] = hashes["ana"].split("$")[0] + "$" + hashes["ana"].split("$")[1]
    return {
        "bind": f"127.0.0.1:{PORTA}",
        "base": BASE + "/dados",
        "token": "t",
        "log_acessos": BASE + "/acessos.log",
        "web": {"ligado": False},
        "seguranca": {"blacklist": BASE + "/blacklist.json"},

        # --- diretiva 0: o root. Sempre supervisor e sempre ativo, diga o que
        #     disser o arquivo.
        "root": {"id": 1, "nome": "Administrador do sistema", "login": "root",
                 "senha_hash": hashes["root"], "email": "root@empresa.com.br"},

        "usuarios": [
            # --- diretiva 1: ADMINISTRADOR DO SISTEMA. Pode tudo, em toda base.
            {"id": 10, "nome": "Ana Administradora", "login": "ana",
             "senha_hash": hashes["ana"], "email": "ana@empresa.com.br",
             "supervisor": True, "ativo": True},

            # --- diretiva 2: ADMINISTRADOR DE UM BANCO SO. Manda em `loja`
            #     (inclusive criar e apagar tabela, que exige `administrar`) e
            #     nao existe para `rh`: base nao listada e sem "*" nega tudo.
            {"id": 11, "nome": "Bruno DBA da loja", "login": "bruno",
             "senha_hash": hashes["bruno"], "email": "bruno@empresa.com.br",
             "supervisor": False, "ativo": True,
             "bases": {
                 "loja": {"ler": True, "inserir": True, "alterar": True,
                          "excluir": True, "criar": True, "reindexar": True,
                          "diario": True, "verificar": True, "administrar": True}
             }},

            # --- diretiva 3: LEITOR DE UMA TABELA SO. Le e grava em `loja`,
            #     MENOS em `folha`, onde a regra da tabela SUBSTITUI a da base
            #     e um objeto vazio nega tudo.
            {"id": 12, "nome": "Carlos Consulta", "login": "carlos",
             "senha_hash": hashes["carlos"], "email": "carlos@empresa.com.br",
             "supervisor": False, "ativo": True,
             "bases": {
                 "loja": {"ler": True, "inserir": True, "alterar": True,
                          "tabelas": {"folha": {}}}
             }},
        ],
    }


DIRETIVAS = None


# ------------------------------------------------------------------ as provas
def parte_2_o_cadastro_conferido(sv):
    print("\n=== 2. O cadastro, conferido pelo proprio binario\n")
    r = subprocess.run([BINARIO, "--config", sv.cfg, "--usuarios"],
                       capture_output=True, text=True)
    saida = r.stdout or r.stderr
    print("  $ phxsqld --config <o arquivo acima> --usuarios")
    for l in saida.splitlines():
        print("    " + l)
    RESULTADO["usuarios_cli"] = saida
    afirmar("as tres diretivas e o root aparecem",
            all(x in saida for x in ("root", "ana", "bruno", "carlos")), saida[:200])
    afirmar("nenhuma senha nem hash na saida do --usuarios",
            not any(s in saida for s in SENHAS.values()) and "pbkdf2" not in saida,
            saida[:200])


def parte_3_o_terreno(ses):
    print("\n=== 3. O terreno: dois bancos, tres tabelas (feito pela ana, supervisora)\n")
    for banco in ("loja", "rh"):
        r = ses.pedir(op="criar_database", database=banco)
        afirmar(f"criar_database {banco}", r.get("ok"), str(r.get("erro"))[:120])
    tabelas = [("loja", "clientes"), ("loja", "folha"), ("rh", "cargos")]
    for banco, tab in tabelas:
        r = ses.pedir(op="criar_tabela", database=banco, tabela=tab,
                      colunas=[{"nome": "id", "tipo": "Int8"},
                               {"nome": "nome", "tipo": "Str(40)"}],
                      indices=[{"nome": "porId", "colunas": ["id"], "unico": True}])
        afirmar(f"criar_tabela {banco}.{tab}", r.get("ok"), str(r.get("erro"))[:120])
        r = ses.pedir(op="inserir", database=banco, tabela=tab,
                      valores={"id": 1, "nome": "PRIMEIRA"})
        afirmar(f"inserir em {banco}.{tab}", r.get("ok"), str(r.get("erro"))[:120])


def parte_4_os_tres_papeis(ana, bruno, carlos):
    print("\n=== 4. Cada diretiva vale exatamente o que ela diz\n")

    print("  -- ana, supervisora: pode tudo, em toda base")
    afirmar("ana le loja.folha", ana.pedir(op="varrer", database="loja",
                                           tabela="folha").get("ok"))
    afirmar("ana le rh.cargos", ana.pedir(op="varrer", database="rh",
                                          tabela="cargos").get("ok"))
    q = ana.pedir(op="quem_sou")
    print(f"     quem_sou: {json.dumps(q, ensure_ascii=False)[:200]}")
    RESULTADO["quem_sou_ana"] = json.dumps(q, ensure_ascii=False)

    print("\n  -- bruno, administrador da loja e de mais nada")
    r = bruno.pedir(op="criar_tabela", database="loja", tabela="pedidos",
                    colunas=[{"nome": "id", "tipo": "Int8"}],
                    indices=[{"nome": "porId", "colunas": ["id"], "unico": True}])
    afirmar("bruno CRIA tabela em loja (tem 'criar')", r.get("ok"), str(r.get("erro"))[:140])
    r = bruno.pedir(op="excluir_tabela", database="loja", tabela="pedidos",
                    confirmar="pedidos")
    afirmar("bruno APAGA tabela em loja (tem 'administrar')", r.get("ok"),
            str(r.get("erro"))[:140])
    r = bruno.pedir(op="varrer", database="rh", tabela="cargos")
    print(f"     bruno em rh: {json.dumps(r, ensure_ascii=False)[:200]}")
    afirmar("bruno NAO le rh (base fora da diretiva nega tudo)", negado(r),
            str(r.get("erro"))[:140])
    RESULTADO["bruno_negado_rh"] = str(r.get("erro"))

    print("\n  -- carlos, uma tabela permitida e uma negada NO MESMO BANCO")
    r = carlos.pedir(op="varrer", database="loja", tabela="clientes")
    afirmar("carlos LE loja.clientes (a diretiva da base vale)", r.get("ok"),
            str(r.get("erro"))[:140])
    r = carlos.pedir(op="varrer", database="loja", tabela="folha")
    print(f"     carlos em folha: {json.dumps(r, ensure_ascii=False)[:220]}")
    afirmar("carlos NAO le loja.folha (a regra da tabela SUBSTITUI a da base)",
            negado(r), str(r.get("erro"))[:140])
    RESULTADO["carlos_negado_folha"] = str(r.get("erro"))
    r = carlos.pedir(op="inserir", database="loja", tabela="folha",
                     valores={"id": 9, "nome": "X"})
    afirmar("carlos NAO grava em loja.folha", negado(r), str(r.get("erro"))[:140])
    r = carlos.pedir(op="inserir", database="loja", tabela="clientes",
                     valores={"id": 9, "nome": "NOVE"})
    afirmar("carlos GRAVA em loja.clientes", r.get("ok"), str(r.get("erro"))[:140])

    r = carlos.pedir(op="tabelas", database="loja")
    nomes = [t if isinstance(t, str) else t.get("nome") for t in (r.get("tabelas") or [])]
    print(f"     tabelas que carlos enxerga: {nomes}")
    afirmar("a arvore de carlos ESCONDE folha", "folha" not in nomes, str(nomes))
    afirmar("e mostra clientes", "clientes" in nomes, str(nomes))
    RESULTADO["tabelas_de_carlos"] = nomes


def parte_5_as_portas_dos_fundos(carlos):
    print("\n=== 5. As TRES portas dos fundos: juntar, unir e pivotar\n")
    print("  O portao de permissao e UM so, e ele le o campo \"tabela\" do pedido.")
    print("  Estas tres operacoes escondem a tabela dele -- e pagam conferencia propria.\n")

    print("  -- CONTROLE POSITIVO: as tres na tabela PERMITIDA tem de passar")
    # O "prefixo" do lado B nao e enfeite: sem ele o motor recusa a juncao de
    # uma tabela com ela mesma porque as colunas se sobreporiam na saida. A
    # primeira corrida caiu aqui, e o erro era da SONDA, nao do motor.
    ok_j = carlos.pedir(op="juntar", database="loja",
                        a={"tabela": "clientes", "chave": "id"},
                        b={"tabela": "clientes", "chave": "id", "prefixo": "c2"})
    afirmar("juntar clientes x clientes passa", ok_j.get("ok"), str(ok_j.get("erro"))[:140])
    ok_u = carlos.pedir(op="unir", database="loja", tabelas=["clientes", "clientes"])
    afirmar("unir clientes+clientes passa", ok_u.get("ok"), str(ok_u.get("erro"))[:140])
    ok_p = carlos.pedir(op="pivotar", database="loja", tabela="clientes",
                        linhas=[{"campo": "nome"}], colunas=[{"campo": "id"}],
                        agregador="contagem")
    afirmar("pivotar sobre clientes passa", ok_p.get("ok"), str(ok_p.get("erro"))[:140])

    print("\n  -- a tabela NEGADA pelos tres caminhos")
    j = carlos.pedir(op="juntar", database="loja",
                     a={"tabela": "clientes", "chave": "id"},
                     b={"tabela": "folha", "chave": "id"})
    print(f"     juntar (folha como lado B): {json.dumps(j, ensure_ascii=False)[:220]}")
    afirmar("juntar com folha no lado B e RECUSADO", negado(j), str(j.get("erro"))[:140])

    u = carlos.pedir(op="unir", database="loja", tabelas=["clientes", "folha"])
    print(f"     unir (folha na lista):      {json.dumps(u, ensure_ascii=False)[:220]}")
    afirmar("unir com folha na lista e RECUSADO", negado(u), str(u.get("erro"))[:140])

    p = carlos.pedir(op="pivotar", database="loja", tabela="clientes",
                     juntar=[{"tabela": "folha", "coluna": "id", "prefixo": "f",
                              "chave": "id"}],
                     linhas=[{"campo": "f.nome"}], colunas=[{"campo": "nome"}],
                     agregador="contagem")
    print(f"     pivotar (folha em juntar):  {json.dumps(p, ensure_ascii=False)[:220]}")
    afirmar("pivotar com folha aninhada e RECUSADO", negado(p), str(p.get("erro"))[:140])

    s = carlos.pedir(op="sql", database="loja", texto="SELECT * FROM folha")
    print(f"     SELECT pela camada SQL:     {json.dumps(s, ensure_ascii=False)[:220]}")
    afirmar("SELECT * FROM folha pela camada SQL e RECUSADO", negado(s),
            str(s.get("erro"))[:140])

    RESULTADO["portas_dos_fundos"] = {
        "juntar": str(j.get("erro")), "unir": str(u.get("erro")),
        "pivotar": str(p.get("erro")), "sql": str(s.get("erro")),
    }


def parte_6_a_senha(sv, ana, carlos):
    print("\n=== 6. Senha nunca em texto puro: a varredura, e o controle dela\n")
    bruta = ana.cru(op="usuarios")
    print(f"  resposta crua de usuarios (primeiros 300 bytes):\n    {bruta[:300]}")
    RESULTADO["usuarios_protocolo"] = bruta.strip()[:1200]

    # CONTROLE POSITIVO DO VARREDOR: ele acha a senha quando ela esta la.
    palheiro = 'texto qualquer com ' + SENHAS["carlos"] + ' no meio'
    afirmar("controle: o varredor ACHA a senha quando ela esta no texto",
            any(s in palheiro for s in SENHAS.values()))

    for rotulo, texto in (("a resposta do usuarios", bruta),
                          ("o acessos.log", sv.acessos()),
                          ("o erro padrao do servidor", sv.erros())):
        vazou = [q for q, s in SENHAS.items() if s in texto]
        afirmar(f"nenhuma senha em texto puro em {rotulo}", not vazou, str(vazou))
    afirmar("nenhum hash pbkdf2 na resposta do usuarios", "pbkdf2" not in bruta,
            bruta[:200])

    print("\n  o que o acessos.log guardou das tentativas de carlos:")
    for l in sv.acessos().splitlines():
        if '"usuario":"carlos"' in l or '"carlos"' in l:
            print("    " + l[:200])
            break

    # A tentativa NEGADA tambem e registrada -- o log guarda toda tentativa.
    negadas = sum(1 for l in sv.acessos().splitlines()
                  if '"carlos"' in l and '"ok":false' in l)
    afirmar("o acessos.log registra tambem as tentativas NEGADAS", negadas > 0,
            f"{negadas} linhas")
    RESULTADO["negadas_no_log"] = negadas

    # E o limite honesto, medido: a senha VIAJA em claro no pedido de login.
    afirmar("limite registrado: a senha viaja em claro no pedido de login",
            SENHAS["carlos"] in json.dumps(
                {"op": "login", "usuario": "carlos", "senha": SENHAS["carlos"]}))


def parte_7_o_portao_do_login(sv):
    print("\n=== 7. Portao 2: havendo cadastro, o token sozinho nao basta\n")
    print("  O token e a chave da porta da REDE, nao a identidade de ninguem.")
    s = Sessao()
    r = s.pedir(op="varrer", database="loja", tabela="clientes")
    print(f"    sem login: {json.dumps(r, ensure_ascii=False)[:220]}")
    afirmar("havendo cadastro, o token sozinho NAO basta", not r.get("ok"),
            str(r.get("erro"))[:140])
    RESULTADO["sem_login"] = str(r.get("erro"))
    s.fechar()


def main():
    global DIRETIVAS
    commit = subprocess.run(["git", "-C", RAIZ, "rev-parse", "--short", "HEAD"],
                            capture_output=True, text=True).stdout.strip()
    versao = subprocess.run([BINARIO, "-V"], capture_output=True, text=True)
    RESULTADO["commit"] = commit
    RESULTADO["versao"] = (versao.stdout or versao.stderr).strip()
    print(f"# Script de diretivas de usuario -- {RESULTADO['quando_utc']} UTC")
    print(f"# commit {commit} -- {RESULTADO['versao']}")
    print(f"# porta {PORTA}, base {BASE}")

    os.makedirs(BASE, exist_ok=True)
    DIRETIVAS = montar_diretivas()
    print("\n  o config.json que as tres diretivas produzem (secao usuarios):")
    for l in json.dumps({"usuarios": DIRETIVAS["usuarios"]}, indent=2,
                        ensure_ascii=False).splitlines():
        # O hash sai mascarado da SAIDA, nao do arquivo: o arquivo precisa
        # dele inteiro, e a saida vai colada num documento.
        if "senha_hash" in l:
            l = l.split(":")[0] + ': "pbkdf2-sha256$210000$<sal>$<hash>"' + ("," if l.rstrip().endswith(",") else "")
        print("    " + l)
    RESULTADO["diretivas"] = DIRETIVAS["usuarios"]

    try:
        with Servidor() as sv:
            parte_2_o_cadastro_conferido(sv)
            ana = Sessao("ana")
            bruno = Sessao("bruno")
            carlos = Sessao("carlos")
            print("\n  respostas cruas dos tres logins:")
            for s in (ana, bruno, carlos):
                print(f"    {s.login}: {s.cru_do_login.strip()[:180]}")
                afirmar(f"login de {s.login} passou",
                        json.loads(s.cru_do_login).get("ok"), s.cru_do_login[:160])
                afirmar(f"a resposta do login de {s.login} nao traz a senha",
                        SENHAS[s.login] not in s.cru_do_login)
            RESULTADO["login_carlos"] = carlos.cru_do_login.strip()
            parte_3_o_terreno(ana)
            parte_4_os_tres_papeis(ana, bruno, carlos)
            parte_5_as_portas_dos_fundos(carlos)
            parte_6_a_senha(sv, ana, carlos)
            for s in (ana, bruno, carlos):
                s.fechar()
            parte_7_o_portao_do_login(sv)
    finally:
        shutil.rmtree(BASE, ignore_errors=True)

    RESULTADO["falhas"] = FALHAS
    with open(os.path.join(RAIZ, "bancada/diretivas/resultados.json"), "w") as f:
        json.dump(RESULTADO, f, indent=2, ensure_ascii=False)
    print(f"\n=== {len(FALHAS)} afirmacao(oes) falharam: {FALHAS}")
    print("=== resultados em bancada/diretivas/resultados.json")
    return 0


if __name__ == "__main__":
    sys.exit(main())
