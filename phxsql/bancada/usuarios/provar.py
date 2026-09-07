#!/usr/bin/env python3
"""O CADASTRO DE USUARIOS PELO PROTOCOLO, exercitado contra o motor vivo.

    ./cargo-da-frente.sh build --release -p phxsql-server --bin phxsqld
    python3 bancada/usuarios/provar.py

Pedido 221: *«Nao existe operacao de protocolo que crie usuario: as diretivas
se escrevem no `config.json` e reiniciam.»* Este script prova que existe --
e prova o que a leitura do codigo NAO da.

# O que ele mede, e que teste unitario nao alcanca

1. **A QUENTE, pelo soquete, sem reiniciar.** O `phxsqld` sobe UMA vez. O
   usuario e criado por um pedido de protocolo, e o login dele acontece numa
   CONEXAO NOVA contra o MESMO processo. Um teste unitario prova a estrutura;
   so o processo vivo prova que nada ficou preso na fotografia do arranque.
2. **A senha em tres lugares onde ela vazaria calada**: o `config.json` no
   disco, o `acessos.log` escrito pelo laco de conexao (que o teste unitario
   nao percorre) e o `perfil.txt` do Profiler LIGADO -- que existe justamente
   para mostrar o texto cru dos pedidos.
3. **A sessao do excluido**, na conexao que ja estava aberta e autenticada:
   ela nao e derrubada, ela deixa de valer no pedido seguinte.
4. **O controle positivo do varredor**: antes de acreditar em "nao achei a
   senha", o script mostra que o mesmo varredor a ACHA quando ela esta la.

# Como roda

Sobe UM `phxsqld` (porta 7501) em 127.0.0.1, com tudo em `/tmp/phx-f5u-<pid>`,
e derruba por PID no fim -- nunca `pkill`. A porta sai de `PHX_F5_PORTA_USUARIOS`.
Os numeros vao para `resultados.json`.
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
BASE = f"/tmp/phx-f5u-{os.getpid()}"
PORTA = int(os.environ.get("PHX_F5_PORTA_USUARIOS", "7501"))

SENHA_ANA = "a-senha-da-ana-2026"
SENHA_CARLOS = "a-senha-do-carlos-2026"
SENHA_NOVA = "a-senha-trocada-2026"
SENHA_SQL = "a-senha-do-sql-2026"
TODAS_AS_SENHAS = [SENHA_ANA, SENHA_CARLOS, SENHA_NOVA, SENHA_SQL]

RESULTADO = {"quando_utc": time.strftime("%Y-%m-%d %H:%M:%S", time.gmtime())}
FALHAS = []


def diz(rotulo, ok, detalhe=""):
    print(f"  [{'OK  ' if ok else 'FALHA'}] {rotulo}"
          + (f" -- {detalhe}" if detalhe and not ok else ""))
    return ok


def afirmar(rotulo, ok, detalhe=""):
    if not diz(rotulo, ok, detalhe):
        FALHAS.append(rotulo)
    return ok


def hash_de(senha):
    """`echo -n <senha> | phxsqld --senha` -- pelo cano, para a senha nao ficar
    no historico do shell nem aparecer num `ps`."""
    r = subprocess.run([BINARIO, "--senha"], input=senha.encode(), capture_output=True)
    saida = (r.stdout or r.stderr).decode().strip()
    return saida.split('"')[3] if '"' in saida else saida


def varrer(texto, senhas=None):
    """Quais senhas aparecem neste texto? E o varredor unico do script -- o
    mesmo que a parte 0 prova que funciona."""
    return [s for s in (senhas or TODAS_AS_SENHAS) if s in texto]


class Servidor:
    def __enter__(self):
        if not os.path.exists(BINARIO):
            raise SystemExit(
                f"{BINARIO} nao existe. Rode antes:\n"
                "  ./cargo-da-frente.sh build --release -p phxsql-server --bin phxsqld")
        os.makedirs(BASE + "/dados", exist_ok=True)
        self.cfg = BASE + "/config.json"
        with open(self.cfg, "w") as f:
            json.dump(CONFIG, f, indent=2, ensure_ascii=False)
        self.saida = open(BASE + "/servidor.err", "wb")
        self.p = subprocess.Popen([BINARIO, "--config", self.cfg],
                                  stdout=self.saida, stderr=subprocess.STDOUT)
        with open(BASE + "/servidor.pid", "w") as f:
            f.write(str(self.p.pid))
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
        return open(BASE + "/servidor.err", errors="replace").read()

    def acessos(self):
        c = BASE + "/acessos.log"
        return open(c, errors="replace").read() if os.path.exists(c) else ""

    def config_no_disco(self):
        return open(self.cfg, errors="replace").read()

    def perfil(self):
        c = BASE + "/perfil.txt"
        return open(c, errors="replace").read() if os.path.exists(c) else ""


class Sessao:
    """Uma conexao. A autenticacao acontece UMA vez por conexao, nao por pedido."""

    def __init__(self, login=None, senha=None):
        self.s = socket.create_connection(("127.0.0.1", PORTA), 10)
        self.f = self.s.makefile("rwb")
        self.login = login
        self.cru_do_login = None
        if login:
            self.cru_do_login = self.cru(op="login", usuario=login, senha=senha)

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

    def fichas(self):
        """A lista de `usuarios`. O envelope traz o resultado numa LISTA, e
        nao num objeto, entao o `pedir` nao o desembrulha."""
        r = json.loads(self.cru(op="usuarios", token="t"))
        bruto = r.get("resultado")
        return (bruto if isinstance(bruto, list) else []), json.dumps(r, ensure_ascii=False)

    def entrou(self):
        return json.loads(self.cru_do_login).get("ok") is True

    def fechar(self):
        try:
            self.f.close()
            self.s.close()
        except OSError:
            pass


CONFIG = None


def montar_config():
    print("\n=== 1. O ponto de partida: UM administrador no config.json\n")
    h = hash_de(SENHA_ANA)
    afirmar("o hash da ana e PBKDF2, nao a senha",
            h.startswith("pbkdf2-sha256$210000$") and SENHA_ANA not in h, h[:60])
    RESULTADO["algoritmo"] = "$".join(h.split("$")[:2])
    print(f"  echo -n '<senha da ana>' | phxsqld --senha\n    -> {h[:44]}...{h[-8:]}")
    print("  o cadastro comeca com UMA pessoa; o resto nasce pelo protocolo.")
    return {
        "bind": f"127.0.0.1:{PORTA}",
        "base": BASE + "/dados",
        "token": "t",
        "log_acessos": BASE + "/acessos.log",
        "web": {"ligado": False},
        "seguranca": {"blacklist": BASE + "/blacklist.json"},
        "profiler": {"arquivo": BASE + "/perfil.txt"},
        "_nota": "comentario que a gravacao do cadastro nao pode comer",
        "usuarios": [
            {"id": 10, "nome": "Ana Administradora", "login": "ana",
             "senha_hash": h, "email": "ana@empresa.com.br", "supervisor": True},
        ],
    }


def parte_0_o_varredor(sv):
    """O CONTROLE POSITIVO. Uma varredura que nunca acha nada protege igual e
    nao prova nada: aqui ela acha a senha num texto que a tem."""
    print("\n=== 0. O varredor, provado antes de se acreditar nele\n")
    fingido = json.dumps({"op": "login", "usuario": "ana", "senha": SENHA_ANA})
    afirmar("o varredor ACHA a senha quando ela esta no texto",
            varrer(fingido) == [SENHA_ANA], fingido[:80])
    afirmar("e nao acha onde ela nao esta", varrer('{"op":"ping"}') == [])


def parte_2_criar(ana, sv):
    print("\n=== 2. `usuario_criar`: o cadastro cresce SEM reiniciar\n")
    r = ana.pedir(op="usuario_criar", login="carlos", senha=SENHA_CARLOS,
                  nome="Carlos Consulta", nivel="leitor",
                  email="carlos@empresa.com.br",
                  bases={"loja": {"ler": True, "verificar": True}})
    print(f"  pedido:   usuario_criar carlos (a senha vai no campo `senha`)")
    print(f"  resposta: {json.dumps(r, ensure_ascii=False)[:260]}")
    afirmar("usuario_criar respondeu ok", r.get("ok"), str(r.get("erro"))[:160])
    afirmar("a resposta traz a ficha e NAO traz senha nem hash",
            not varrer(json.dumps(r)) and "pbkdf2" not in json.dumps(r),
            json.dumps(r)[:200])
    RESULTADO["resposta_criar"] = r

    # A prova a quente: uma conexao NOVA contra o MESMO processo.
    novo = Sessao("carlos", SENHA_CARLOS)
    afirmar("o usuario criado ENTRA, no mesmo processo, sem reiniciar",
            novo.entrou(), novo.cru_do_login[:200])
    afirmar("a resposta do login nao traz a senha",
            not varrer(novo.cru_do_login))
    q = novo.pedir(op="quem_sou")
    afirmar("e o `quem_sou` dele diz o nivel que a diretiva pediu",
            q.get("nivel") == "leitor" and q.get("login") == "carlos",
            json.dumps(q)[:200])
    RESULTADO["quem_sou_carlos"] = q
    return novo


def parte_3_o_arquivo(sv):
    print("\n=== 3. O `config.json` no disco: hash, e o resto intacto\n")
    texto = sv.config_no_disco()
    achadas = varrer(texto)
    afirmar("nenhuma senha em texto puro no config.json", not achadas, str(achadas))
    afirmar("o hash do carlos esta la", texto.count("pbkdf2-sha256$210000$") == 2,
            f"{texto.count('pbkdf2-sha256$210000$')} hashes")
    afirmar("o comentario `_nota` sobreviveu a gravacao",
            "comentario que a gravacao do cadastro nao pode comer" in texto)
    afirmar("o arquivo continua sendo JSON valido", json.loads(texto) is not None)
    RESULTADO["hashes_no_arquivo"] = texto.count("pbkdf2-sha256$210000$")
    import re
    for l in texto.splitlines():
        l = re.sub(r'"senha_hash"\s*:\s*"[^"]*"',
                   '"senha_hash": "pbkdf2-sha256$210000$<sal>$<hash>"', l)
        print("    " + l)
    afirmar("o cadastro no arquivo continua legivel (nao virou uma linha so)",
            texto.count("\n") > 12 and max(len(l) for l in texto.splitlines()) < 200,
            f"{texto.count(chr(10))} linhas, maior com {max(len(l) for l in texto.splitlines())}")


def parte_4_alterar(ana):
    print("\n=== 4. `usuario_alterar`: a senha nova vale, a velha para de valer\n")
    r = ana.pedir(op="usuario_alterar", login="carlos", senha=SENHA_NOVA)
    afirmar("usuario_alterar respondeu ok", r.get("ok"), str(r.get("erro"))[:160])
    afirmar("a resposta do alterar nao traz senha nem hash",
            not varrer(json.dumps(r)) and "pbkdf2" not in json.dumps(r))

    com_a_nova = Sessao("carlos", SENHA_NOVA)
    afirmar("a senha NOVA entra", com_a_nova.entrou(), com_a_nova.cru_do_login[:160])
    com_a_nova.fechar()
    com_a_velha = Sessao("carlos", SENHA_CARLOS)
    afirmar("a senha VELHA nao entra mais", not com_a_velha.entrou(),
            com_a_velha.cru_do_login[:160])
    RESULTADO["recusa_da_senha_velha"] = com_a_velha.cru_do_login.strip()
    com_a_velha.fechar()

    # Alterar o que NAO e senha nao pode apagar o resto da ficha.
    ana.pedir(op="usuario_alterar", login="carlos", telefone="+55 47 90000-0000")
    lista, cru = ana.fichas()
    carlos = [u for u in lista if u.get("login") == "carlos"][0]
    afirmar("alterar mexeu SO no que o pedido trouxe",
            carlos.get("telefone") == "+55 47 90000-0000"
            and carlos.get("nome") == "Carlos Consulta"
            and carlos.get("email") == "carlos@empresa.com.br"
            and carlos.get("nivel") == "leitor",
            json.dumps(carlos, ensure_ascii=False)[:220])
    afirmar("e a lista de `usuarios` nunca devolve hash",
            "pbkdf2" not in cru and not varrer(cru), cru[:200])


def parte_5_as_guardas(ana, carlos_sessao):
    print("\n=== 5. As guardas: quem NAO pode, e o que NAO se faz\n")
    # Um SEGUNDO administrador antes de tudo. Sem ele, as duas guardas de
    # si-mesmo nunca chegam a ser exercidas: a do "sem administrador nenhum"
    # pega primeiro, porque a ana e a unica -- e um teste que passa pela
    # guarda errada nao prova a guarda que ele diz provar.
    r = ana.pedir(op="usuario_criar", login="bruno", senha="a-senha-do-bruno-2026",
                  supervisor=True)
    afirmar("o segundo administrador nasce (so supervisor cria supervisor)",
            r.get("ok"), str(r.get("erro"))[:160])

    r = carlos_sessao.pedir(op="usuario_criar", login="x", senha="12345678")
    print(f"    carlos (leitor) criando usuario: {json.dumps(r, ensure_ascii=False)[:200]}")
    afirmar("leitor nao mexe no cadastro", not r.get("ok"), str(r.get("erro"))[:160])
    RESULTADO["recusa_do_leitor"] = str(r.get("erro"))

    for rotulo, pedido, pedaco in [
        ("nao se apaga a propria conta",
         dict(op="usuario_excluir", login="ana"), "propria conta"),
        ("nao se tira de si o poder de administrar",
         dict(op="usuario_alterar", login="ana", supervisor=False, nivel="leitor"),
         "propria conta"),
        ("o root nao se mexe pelo protocolo",
         dict(op="usuario_alterar", login="root", senha="12345678"), "root"),
        ("senha_hash pronto e recusado",
         dict(op="usuario_criar", login="y", senha_hash="pbkdf2-sha256$1$00$00"),
         "senha_hash"),
        ("login com espaco na ponta e recusado",
         dict(op="usuario_criar", login=" z", senha="12345678"), "espaco"),
        ("login repetido e recusado",
         dict(op="usuario_criar", login="carlos", senha="12345678"), "ja ha"),
    ]:
        r = ana.pedir(**pedido)
        afirmar(rotulo, (not r.get("ok")) and pedaco in str(r.get("erro")),
                str(r.get("erro"))[:180])

    # A guarda do ULTIMO administrador precisa de um caminho proprio: com dois
    # ativos ela nunca dispara, e com um so quem dispara antes e a da propria
    # conta. Entao: o bruno desliga a ana (legitimo -- sobra ele), e so entao
    # tenta desligar a si. Uma guarda provada pelo caminho errado nao esta
    # provada.
    bruno = Sessao("bruno", "a-senha-do-bruno-2026")
    afirmar("o bruno entrou", bruno.entrou(), bruno.cru_do_login[:160])
    r = bruno.pedir(op="usuario_alterar", login="ana", ativo=False)
    afirmar("com DOIS administradores, desligar o outro vale",
            r.get("ok"), str(r.get("erro"))[:160])
    r = bruno.pedir(op="usuario_alterar", login="bruno", ativo=False)
    afirmar("mas desligar o ULTIMO e recusado",
            (not r.get("ok")) and "supervisor ativo" in str(r.get("erro")),
            str(r.get("erro"))[:200])
    RESULTADO["recusa_do_ultimo_admin"] = str(r.get("erro"))
    r = bruno.pedir(op="usuario_alterar", login="ana", ativo=True)
    afirmar("e a ana volta", r.get("ok"), str(r.get("erro"))[:160])
    bruno.fechar()
    # A ana foi desligada e religada no meio: a conexao dela releu a ficha e
    # continua valendo, que e a outra ponta do refresco por geracao.
    q = ana.pedir(op="quem_sou")
    afirmar("e a conexao da ana continua valendo depois do vai-e-volta",
            q.get("login") == "ana", json.dumps(q)[:160])


def parte_6_o_sql(ana):
    print("\n=== 6. CREATE/ALTER/DROP USER pela op `sql`\n")
    r = ana.pedir(op="sql", texto=f"CREATE USER bia PASSWORD '{SENHA_SQL}'")
    print(f"    {json.dumps(r, ensure_ascii=False)[:260]}")
    afirmar("CREATE USER respondeu ok", r.get("ok"), str(r.get("erro"))[:160])
    afirmar("o texto SQL de volta vem REDIGIDO, sem a senha dentro",
            not varrer(json.dumps(r)) and "'***'" in json.dumps(r),
            json.dumps(r)[:220])
    RESULTADO["sql_redigido"] = r.get("sql")

    bia = Sessao("bia", SENHA_SQL)
    afirmar("o usuario do CREATE USER entra", bia.entrou(), bia.cru_do_login[:160])
    bia.fechar()

    r = ana.pedir(op="sql", texto="DROP USER bia")
    afirmar("DROP USER respondeu ok", r.get("ok"), str(r.get("erro"))[:160])
    morta = Sessao("bia", SENHA_SQL)
    afirmar("e depois do DROP USER ela nao entra mais", not morta.entrou())
    morta.fechar()

    # E o que NAO e `USER` continua chegando inteiro a quem o atende: um
    # `ALTER TABLE` tem de dar erro de SINTAXE, e nao ser roubado por aqui.
    r = ana.pedir(op="sql", texto="ALTER TABLE clientes ADD x")
    afirmar("`ALTER` de outra coisa nao e roubado por este caminho",
            not r.get("ok") and "usuario" not in str(r.get("erro")).lower(),
            str(r.get("erro"))[:180])


def parte_7_o_excluido(ana, sv):
    print("\n=== 7. O excluido perde a sessao NO PEDIDO SEGUINTE\n")
    ana.pedir(op="usuario_criar", login="dora", senha="a-senha-da-dora-2026",
              nivel="admin")
    dora = Sessao("dora", "a-senha-da-dora-2026")
    afirmar("a dora entrou", dora.entrou(), dora.cru_do_login[:160])
    antes = dora.pedir(op="bancos")
    afirmar("e ela consegue trabalhar", antes.get("ok"), str(antes.get("erro"))[:120])

    r = ana.pedir(op="usuario_excluir", login="dora")
    afirmar("usuario_excluir respondeu ok", r.get("ok"), str(r.get("erro"))[:160])

    depois = dora.pedir(op="bancos")
    print(f"    o pedido seguinte NA MESMA CONEXAO: {json.dumps(depois, ensure_ascii=False)[:220]}")
    afirmar("a conexao ja aberta deixa de valer no pedido seguinte",
            not depois.get("ok"), json.dumps(depois)[:160])
    afirmar("e a recusa e o `faca login` de sempre, nao um soquete derrubado",
            "login" in str(depois.get("erro")).lower(), str(depois.get("erro"))[:160])
    RESULTADO["recusa_do_excluido"] = str(depois.get("erro"))
    dora.fechar()


def parte_8_o_profiler(ana):
    """O Profiler existe para mostrar o TEXTO CRU dos pedidos. E o unico lugar
    do servidor onde a senha de `usuario_criar` apareceria inteira."""
    print("\n=== 8. O Profiler LIGADO, que e onde a senha apareceria\n")
    r = ana.pedir(op="profiler_ligar")
    afirmar("profiler_ligar respondeu ok", r.get("ok"), str(r.get("erro"))[:160])
    ana.pedir(op="usuario_criar", login="elias", senha=SENHA_SQL, nivel="leitor")
    ana.pedir(op="sql", texto=f"ALTER USER elias PASSWORD '{SENHA_NOVA}'")
    anel = ana.pedir(op="profiler")
    texto = json.dumps(anel, ensure_ascii=False)
    achadas = varrer(texto)
    afirmar("o anel do Profiler nao traz senha nenhuma", not achadas, str(achadas))
    afirmar("e ele MOSTRA o pedido, com o campo tapado",
            "***" in texto and "usuario_criar" in texto, texto[:240])
    RESULTADO["profiler_tapou"] = "***" in texto
    ana.pedir(op="profiler_desligar")


def parte_9_os_arquivos(sv):
    print("\n=== 9. Varredura final: acessos.log, perfil.txt e stderr\n")
    for rotulo, texto in [("acessos.log", sv.acessos()),
                          ("perfil.txt do Profiler", sv.perfil()),
                          ("erro padrao do servidor", sv.erros()),
                          ("config.json", sv.config_no_disco())]:
        achadas = varrer(texto)
        afirmar(f"nenhuma senha no {rotulo}", not achadas,
                f"{achadas} em {len(texto)} bytes")
    log = sv.acessos()
    contadas = {op: log.count(f'"op":"{op}"')
                for op in ("usuario_criar", "usuario_alterar", "usuario_excluir")}
    afirmar("o acessos.log registrou as tres operacoes de cadastro",
            all(v > 0 for v in contadas.values()), str(contadas))
    RESULTADO["no_acessos_log"] = contadas
    print(f"    {contadas}")
    # O rastro no erro padrao: cadastro nao muda sem deixar quem mexeu.
    afirmar("o servidor registrou QUEM mexeu no cadastro",
            "cadastro gravado por ana" in sv.erros(), sv.erros()[-300:])


def main():
    global CONFIG
    commit = subprocess.run(["git", "-C", RAIZ, "rev-parse", "--short", "HEAD"],
                            capture_output=True, text=True).stdout.strip()
    versao = subprocess.run([BINARIO, "-V"], capture_output=True, text=True)
    RESULTADO["commit"] = commit
    RESULTADO["versao"] = (versao.stdout or versao.stderr).strip()
    print(f"# Cadastro de usuarios pelo protocolo -- {RESULTADO['quando_utc']} UTC")
    print(f"# commit {commit} -- {RESULTADO['versao']}")
    print(f"# porta {PORTA}, base {BASE}")

    os.makedirs(BASE, exist_ok=True)
    CONFIG = montar_config()
    try:
        with Servidor() as sv:
            parte_0_o_varredor(sv)
            ana = Sessao("ana", SENHA_ANA)
            afirmar("a ana entrou", ana.entrou(), ana.cru_do_login[:200])
            carlos = parte_2_criar(ana, sv)
            parte_3_o_arquivo(sv)
            parte_4_alterar(ana)
            parte_5_as_guardas(ana, carlos)
            parte_6_o_sql(ana)
            parte_7_o_excluido(ana, sv)
            parte_8_o_profiler(ana)
            carlos.fechar()
            ana.fechar()
            time.sleep(0.3)
            parte_9_os_arquivos(sv)
    finally:
        shutil.rmtree(BASE, ignore_errors=True)

    RESULTADO["falhas"] = FALHAS
    with open(os.path.join(RAIZ, "bancada/usuarios/resultados.json"), "w") as f:
        json.dump(RESULTADO, f, indent=2, ensure_ascii=False)
    print(f"\n=== {len(FALHAS)} afirmacao(oes) falharam: {FALHAS}")
    print("=== resultados em bancada/usuarios/resultados.json")
    return 1 if FALHAS else 0


if __name__ == "__main__":
    sys.exit(main())
