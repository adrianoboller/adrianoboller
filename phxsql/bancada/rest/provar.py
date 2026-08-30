#!/usr/bin/env python3
"""A prova REAL do webservice REST, pelo soquete e com um cliente HTTP proprio.

    python3 bancada/rest/provar.py

# Por que um cliente HTTP escrito aqui

Porque `urllib` esconde justamente o que precisa ser provado. Ele segue
redirecionamento, decide sozinho o que fazer com 401, normaliza cabecalho e
levanta excecao em vez de devolver o codigo -- e o codigo E o resultado destes
testes. Aqui o pedido e montado byte a byte num soquete cru, e o que volta e
lido como o servidor mandou.

E teste unitario nao prova servidor: os vinte testes de `rest.rs` passam sem
que uma porta tenha subido. Estes passos so existem porque ha um `phxsqld` de
verdade escutando.

# Portas

7500 a 7549, que e a faixa desta frente. As portas de FABRICA sao outras --
6000 para o REST e 7000 para o explorador --, e sobrescreve-las pelo
`config.json` ja e um teste util por si: prova que a porta e configuravel e
nao cravada.

# O que so este script acha

1. o servidor SEM a secao `rest` nao escuta -- o comportamento velho, provado
   contra o sistema operacional e nao contra uma struct;
2. o portao de permissao continua sendo UM: usuario sem direito leva 403 numa
   tabela QUE ESTA na lista de expostas;
3. a porta dos fundos da juncao: a tabela escondida como lado B;
4. a especificacao servida bate, rota a rota, com o que a porta atende.

Mata so os PIDs que ele mesmo subiu. Nunca `pkill -f`.
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
RAIZ = sys.argv[1] if len(sys.argv) > 1 else os.path.abspath(os.path.join(AQUI, "..", ".."))
PHXSQLD = os.path.join(RAIZ, "target", "release", "phxsqld")
BASE = "/tmp/phx-rest-prova"

PORTA_DADOS = 7510
PORTA_REST = 7511
PORTA_SWAGGER = 7512
PORTA_MUDO = 7513          # o servidor SEM a secao rest
PORTA_REST_MUDO = 6000     # a porta de fabrica, que ele NAO pode abrir
PORTA_SWAGGER_MUDO = 7000  # idem, a do explorador

TOKEN = "restprova"
TOKEN_DO_REST = "so-para-o-rest"
DONO = "adm"
VISITA = "visita"
SENHA = "segredo1"

PIDS = []
FALHAS = []
PASSOS = []


def ok(nome, condicao, detalhe=""):
    PASSOS.append((nome, bool(condicao), detalhe))
    if not condicao:
        FALHAS.append(f"{nome}: {detalhe}")
    print(("  OK   " if condicao else "  FALHA") +
          f"  {nome}" + (f"  -- {detalhe}" if detalhe else ""))


# --------------------------------------------------------- o cliente HTTP
def http(porta, metodo, caminho, corpo=None, cabecalhos=None, prazo=10):
    """Um pedido HTTP montado byte a byte. Devolve (codigo, cabecalhos, corpo)."""
    dados = b"" if corpo is None else json.dumps(corpo).encode()
    linhas = [f"{metodo} {caminho} HTTP/1.1", "Host: 127.0.0.1", "Connection: close"]
    for k, v in (cabecalhos or {}).items():
        linhas.append(f"{k}: {v}")
    if corpo is not None:
        linhas.append("Content-Type: application/json")
        linhas.append(f"Content-Length: {len(dados)}")
    pedido = ("\r\n".join(linhas) + "\r\n\r\n").encode() + dados

    s = socket.create_connection(("127.0.0.1", porta), prazo)
    s.settimeout(prazo)
    try:
        s.sendall(pedido)
        bruto = b""
        while True:
            pedaco = s.recv(65536)
            if not pedaco:
                break
            bruto += pedaco
    finally:
        s.close()

    cabeca, _, resto = bruto.partition(b"\r\n\r\n")
    primeira = cabeca.split(b"\r\n")[0].decode("utf-8", "replace")
    codigo = int(primeira.split()[1]) if len(primeira.split()) > 1 else 0
    cabs = {}
    for linha in cabeca.split(b"\r\n")[1:]:
        k, _, v = linha.decode("utf-8", "replace").partition(":")
        cabs[k.strip().lower()] = v.strip()
    return codigo, cabs, resto.decode("utf-8", "replace")


def rest(caminho, corpo=None, token=TOKEN_DO_REST, sessao=None, porta=PORTA_REST):
    cab = {}
    if token is not None:
        cab["Authorization"] = f"Bearer {token}"
    if sessao:
        cab["X-Sessao"] = sessao
    codigo, _, texto = http(porta, "POST", caminho, corpo if corpo is not None else {}, cab)
    try:
        return codigo, json.loads(texto)
    except ValueError:
        return codigo, {"__bruto": texto[:200]}


# ------------------------------------------------------------- o servidor
def hash_da_senha(senha):
    saida = subprocess.run([PHXSQLD, "--senha"], input=senha + "\n",
                           capture_output=True, text=True).stdout
    return saida.split('": "')[1].split('"')[0]


def tudo():
    return {"ler": True, "inserir": True, "alterar": True, "excluir": True,
            "criar": True, "administrar": True, "diario": True,
            "verificar": True, "replicar": True, "reindexar": True}


def escrever_config(h):
    """Dois servidores: um com o REST ligado, e um SEM a secao -- o velho."""
    os.makedirs(os.path.join(BASE, "servico"), exist_ok=True)
    os.makedirs(os.path.join(BASE, "mudo"), exist_ok=True)
    usuarios = [
        {"login": DONO, "nome": "Dono", "id": 10, "senha_hash": h,
         "bases": {"*": tudo()}},
        # A visita LE `clientes` e nao le `salarios` -- e `salarios` ESTA na
        # lista de tabelas expostas. E a unica forma de provar que a lista nao
        # alarga: se ela desse direito, esta conta leria.
        {"login": VISITA, "nome": "Visita", "id": 11, "senha_hash": h,
         "bases": {"loja": {"ler": True, "tabelas": {"salarios": {"ler": False}}}}},
    ]
    with open(os.path.join(BASE, "servico", "config.json"), "w") as f:
        json.dump({
            "base": "base",
            "bind": f"127.0.0.1:{PORTA_DADOS}",
            "token": TOKEN,
            "web": {"ligado": False},
            "rest": {
                "ligado": True,
                "bind": f"127.0.0.1:{PORTA_REST}",
                "nome": "Loja",
                "database": "loja",
                "tabelas": ["clientes", "salarios"],
                "token": TOKEN_DO_REST,
                "swagger_ligado": True,
                "swagger_bind": f"127.0.0.1:{PORTA_SWAGGER}",
            },
            "usuarios": usuarios,
        }, f, indent=2)
    # O VELHO: nem a palavra `rest` aparece no arquivo.
    with open(os.path.join(BASE, "mudo", "config.json"), "w") as f:
        json.dump({
            "base": "base",
            "bind": f"127.0.0.1:{PORTA_MUDO}",
            "token": TOKEN,
            "web": {"ligado": False},
            "usuarios": [usuarios[0]],
        }, f, indent=2)


def subir(nome):
    d = os.path.join(BASE, nome)
    log = open(os.path.join(d, "servidor.log"), "a")
    p = subprocess.Popen([PHXSQLD], cwd=d, stdout=log, stderr=subprocess.STDOUT,
                         stdin=subprocess.DEVNULL)
    PIDS.append(p.pid)
    return p


def esperar_porta(porta, prazo=20):
    fim = time.time() + prazo
    while time.time() < fim:
        try:
            socket.create_connection(("127.0.0.1", porta), 0.5).close()
            return True
        except OSError:
            time.sleep(0.2)
    return False


def porta_muda(porta):
    """A porta esta fechada? Uma conexao que casa e a prova do contrario."""
    try:
        socket.create_connection(("127.0.0.1", porta), 0.5).close()
        return False
    except OSError:
        return True


def falar_pelo_soquete(porta, pedido):
    s = socket.create_connection(("127.0.0.1", porta), 5)
    f = s.makefile("rwb")
    pedido.setdefault("token", TOKEN)
    f.write((json.dumps(pedido) + "\n").encode())
    f.flush()
    r = json.loads(f.readline().decode())
    f.close()
    s.close()
    return r


def criar_escondida_pela_porta_de_dados():
    """A tabela que o REST NAO expoe, criada pela porta que nao estreita nada.

    Ela precisa existir de verdade: sem isso, o 404 do REST seria verdade por
    acidente -- a tabela nao existiria mesmo -- e o teste do estreitamento
    passaria por engano, que e pior que teste que falta.
    """
    import hashlib
    import hmac
    s = socket.create_connection(("127.0.0.1", PORTA_DADOS), 5)
    f = s.makefile("rwb")

    def fala(p):
        p.setdefault("token", TOKEN)
        f.write((json.dumps(p) + "\n").encode())
        f.flush()
        return json.loads(f.readline().decode())

    d = fala({"op": "desafio", "usuario": DONO})["resultado"]
    chave = hashlib.pbkdf2_hmac("sha256", SENHA.encode(),
                                bytes.fromhex(d["sal"]), d["iteracoes"])
    nonce_cliente = "abcdef" * 5 + "1234"
    prova = hmac.new(chave, f"{d['nonce']},{nonce_cliente},{DONO}".encode(),
                     hashlib.sha256).hexdigest()
    fala({"op": "login", "usuario": DONO, "prova": prova,
          "nonce_cliente": nonce_cliente})
    r = fala({"op": "criar_tabela", "database": "loja", "tabela": "escondida",
              "colunas": [{"nome": "id", "tipo": "Int8"}]})
    f.close()
    s.close()
    return r


def derrubar():
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


def entrar(login, porta=PORTA_REST):
    """O login pelo REST: desafio, prova, e a sessao que volta no envelope."""
    import hashlib
    import hmac
    codigo, r = rest("/v1/desafio", {"usuario": login}, porta=porta)
    if codigo != 200 or not r.get("ok"):
        return None, (codigo, r)
    d = r["resultado"]
    sessao = r.get("sessao")
    chave = hashlib.pbkdf2_hmac("sha256", SENHA.encode(),
                                bytes.fromhex(d["sal"]), d["iteracoes"])
    nonce_cliente = "c0ffee" * 5 + "cafe"
    # A mensagem e `nonce_servidor,nonce_cliente,usuario` -- os dois nonces e o
    # login, separados por virgula. Escrita aqui de novo, e nao importada de
    # lugar nenhum: um cliente de prova que reusasse o codigo do servidor
    # provaria que o servidor concorda consigo mesmo.
    mensagem = f"{d['nonce']},{nonce_cliente},{login}".encode()
    prova = hmac.new(chave, mensagem, hashlib.sha256).hexdigest()
    codigo, r = rest("/v1/login",
                     {"usuario": login, "prova": prova, "nonce_cliente": nonce_cliente},
                     sessao=sessao, porta=porta)
    if codigo != 200 or not r.get("ok"):
        return None, (codigo, r)
    return r.get("sessao"), (codigo, r)


def main():
    if not os.path.exists(PHXSQLD):
        sys.exit(f"nao achei {PHXSQLD} -- cargo build --release")
    shutil.rmtree(BASE, ignore_errors=True)
    escrever_config(hash_da_senha(SENHA))
    subir("servico")
    subir("mudo")
    if not esperar_porta(PORTA_DADOS) or not esperar_porta(PORTA_MUDO):
        derrubar()
        sys.exit("os servidores nao subiram")

    print("\n== 1. o comportamento VELHO: sem a secao rest, nada escuta ==")
    # O servidor mudo esta NO AR -- a porta de dados dele responde. O que ele
    # nao pode e ter aberto porta nenhuma a mais.
    r = falar_pelo_soquete(PORTA_MUDO, {"op": "ping"})
    ok("o servidor sem a secao rest esta no ar", r.get("ok"), json.dumps(r)[:120])
    ok(f"e NAO escuta a porta de fabrica do REST ({PORTA_REST_MUDO})",
       porta_muda(PORTA_REST_MUDO))
    ok(f"e NAO escuta a porta de fabrica do explorador ({PORTA_SWAGGER_MUDO})",
       porta_muda(PORTA_SWAGGER_MUDO))

    print("\n== 2. a porta configurada sobe, e e a do config.json ==")
    ok("o REST escuta a porta que o arquivo mandou", esperar_porta(PORTA_REST))
    ok("o explorador escuta a dele", esperar_porta(PORTA_SWAGGER))

    print("\n== 3. o token da porta ==")
    codigo, r = rest("/v1/ping", token="chute")
    ok("Bearer errado leva 401", codigo == 401 and not r.get("ok"),
       f"{codigo} {json.dumps(r)[:120]}")
    codigo, r = rest("/v1/ping", token=TOKEN)
    ok("o token do PROTOCOLO nao abre a porta que tem token proprio",
       codigo == 401, f"{codigo} {json.dumps(r)[:120]}")
    codigo, r = rest("/v1/ping")
    ok("o token do REST abre", codigo == 200 and r.get("ok"),
       f"{codigo} {json.dumps(r)[:160]}")
    ok("e a resposta traz a versao", r.get("resultado", {}).get("phxsql"),
       json.dumps(r)[:160])

    print("\n== 4. o caminho manda, e o corpo nao pode discordar ==")
    codigo, r = rest("/v1/ping", {"op": "excluir"})
    ok("corpo com outra `op` leva 400 em vez de virar outra coisa",
       codigo == 400 and not r.get("ok"), f"{codigo} {json.dumps(r)[:160]}")
    codigo, r = rest("/v1/nao_existe_essa")
    ok("rota que nao e operacao leva 404", codigo == 404, str(codigo))

    print("\n== 5. o dado, montado pelo dono ==")
    sessao, detalhe = entrar(DONO)
    ok("login do dono pelo REST", sessao is not None, json.dumps(detalhe)[:200])
    codigo, r = rest("/v1/criar_database", {"database": "loja"}, sessao=sessao)
    ok("criar_database", r.get("ok"), f"{codigo} {json.dumps(r)[:160]}")
    for tabela in ("clientes", "salarios", "escondida"):
        codigo, r = rest("/v1/criar_tabela",
                         {"database": "loja", "tabela": tabela,
                          "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                                      {"nome": "nome", "tipo": "Str(40)"}]},
                         sessao=sessao)
        # `escondida` NAO esta em rest.tabelas: criar por aqui tem de falhar,
        # e e a primeira prova do estreitamento.
        if tabela == "escondida":
            ok("tabela fora da lista nem pode ser criada pelo REST",
               codigo == 404, f"{codigo} {json.dumps(r)[:160]}")
        else:
            ok(f"criar_tabela {tabela}", r.get("ok"), f"{codigo} {json.dumps(r)[:160]}")
    # A escondida existe de verdade -- criada pela porta de DADOS, que nao tem
    # estreitamento nenhum. E ela que prova que o REST a esconde sem mentir.
    r = criar_escondida_pela_porta_de_dados()
    ok("a escondida existe, criada pela porta de dados", r.get("ok"),
       json.dumps(r)[:160])
    for tabela in ("clientes", "salarios"):
        codigo, r = rest("/v1/inserir",
                         {"database": "loja", "tabela": tabela,
                          "valores": {"id": 1, "nome": "Blumenau"}}, sessao=sessao)
        ok(f"inserir em {tabela}", r.get("ok"), f"{codigo} {json.dumps(r)[:160]}")

    print("\n== 6. o pedido sem `database` recebe o do config.json ==")
    codigo, r = rest("/v1/ler", {"tabela": "clientes", "rowid": 1}, sessao=sessao)
    ok("ler sem dizer o banco funciona", codigo == 200 and r.get("ok"),
       f"{codigo} {json.dumps(r)[:200]}")
    ok("e o dado nao foi mexido -- «Blumenau» continua «Blumenau»",
       r.get("resultado", {}).get("nome") == "Blumenau",
       json.dumps(r.get("resultado", {}))[:200])

    print("\n== 7. ESTREITA: tabela fora da lista nao existe ==")
    codigo, r = rest("/v1/ler", {"tabela": "escondida", "rowid": 1}, sessao=sessao)
    ok("tabela fora da lista leva 404", codigo == 404, f"{codigo} {json.dumps(r)[:200]}")
    ok("e a recusa nao conta que ela existe",
       "permiss" not in json.dumps(r).lower() and "direito" not in json.dumps(r).lower(),
       json.dumps(r)[:200])
    # A porta dos fundos: pedir a escondida como o lado B de uma juncao.
    codigo, r = rest("/v1/juntar",
                     {"database": "loja",
                      "a": {"tabela": "clientes"}, "b": {"tabela": "escondida"},
                      "em": [["id", "id"]]}, sessao=sessao)
    ok("nem como lado B de uma juncao", codigo == 404, f"{codigo} {json.dumps(r)[:200]}")
    codigo, r = rest("/v1/unir",
                     {"database": "loja", "tabelas": ["clientes", "escondida"]},
                     sessao=sessao)
    ok("nem dentro da lista de uma uniao", codigo == 404, f"{codigo} {json.dumps(r)[:200]}")
    # E o banco de fora: `rest.database` e "loja".
    codigo, r = rest("/v1/tabelas", {"database": "phxsys"}, sessao=sessao)
    ok("banco fora do config.json tambem nao existe", codigo == 404,
       f"{codigo} {json.dumps(r)[:200]}")

    print("\n== 8. NAO ALARGA: estar na lista nao da direito ==")
    sessao_v, detalhe = entrar(VISITA)
    ok("login da visita pelo REST", sessao_v is not None, json.dumps(detalhe)[:200])
    codigo, r = rest("/v1/ler", {"tabela": "clientes", "rowid": 1}, sessao=sessao_v)
    ok("a visita LE clientes, que ela pode", codigo == 200 and r.get("ok"),
       f"{codigo} {json.dumps(r)[:200]}")
    codigo, r = rest("/v1/ler", {"tabela": "salarios", "rowid": 1}, sessao=sessao_v)
    ok("e leva 403 em salarios, QUE ESTA NA LISTA -- o portao continua sendo um",
       codigo == 403 and not r.get("ok"), f"{codigo} {json.dumps(r)[:250]}")
    ok("e a recusa e de direito, e nao de inexistencia",
       r.get("nome") == "ACESSO_NEGADO", json.dumps(r)[:250])
    # Sem sessao nenhuma, o token de servico nao chama operacao que pede poder.
    codigo, r = rest("/v1/ler", {"tabela": "clientes", "rowid": 1})
    ok("sem login, operacao que pede direito e recusada", codigo == 403,
       f"{codigo} {json.dumps(r)[:200]}")

    print("\n== 9. a especificacao, servida ==")
    codigo, cabs, texto = http(PORTA_REST, "GET", "/openapi.json")
    ok("GET /openapi.json responde 200", codigo == 200, str(codigo))
    ok("com tipo JSON", "json" in cabs.get("content-type", ""), cabs.get("content-type", ""))
    espec = {}
    try:
        espec = json.loads(texto)
    except ValueError as e:
        ok("a especificacao servida e JSON", False, str(e))
    if espec:
        ok("a especificacao servida e JSON", True, f"{len(texto)} bytes")
        ok("declara OpenAPI 3.1", espec.get("openapi", "").startswith("3.1"),
           espec.get("openapi", ""))
        ok("o titulo e o nome do config.json",
           espec.get("info", {}).get("title") == "Loja",
           json.dumps(espec.get("info", {}))[:160])
        ok("a descricao conta o estreitamento",
           "clientes" in espec.get("info", {}).get("description", ""),
           espec.get("info", {}).get("description", "")[:160])
        ok("o esquema de seguranca diz o limite do Bearer em claro",
           "claro" in json.dumps(espec.get("components", {}), ensure_ascii=False))
        ok("e o token NAO aparece na especificacao",
           TOKEN_DO_REST not in texto and TOKEN not in texto)

        rotas = list(espec.get("paths", {}).keys())
        ok("ha mais de cem rotas documentadas", len(rotas) > 100, str(len(rotas)))
        # O laco fechado contra o servidor VIVO: cada rota documentada tem de
        # ser atendida. 404 aqui seria a especificacao mentindo.
        # O laco fechado contra o servidor VIVO: cada rota documentada tem de
        # ser ROTEADA. A sonda vai com o token BOM e SEM SESSAO -- e as duas
        # escolhas custaram uma rodada cada:
        #
        # * com o token errado, a quinta sonda bloqueia o IP (violacao leve) e
        #   da sexta em diante a bancada mede o bloqueio, nao o roteamento;
        # * com sessao, `servico_parar` e `esvaziar_lixeira` EXECUTAM, e a
        #   bancada derruba o servidor que esta medindo -- foi o que aconteceu.
        #
        # Sem sessao, num servidor com cadastro, toda operacao que pede direito
        # para em «faca login» antes de tocar em nada. As seis que nao pedem
        # (`ping`, `login`, `desafio`, `quem_sou`, `sair`, `catalogo`) sao
        # justamente as que nao fazem estrago.
        fora = []
        for rota in rotas:
            c, corpo = rest(f"/v1{rota}", {})
            if c == 404 and "webservice" not in json.dumps(corpo):
                fora.append((rota, c, json.dumps(corpo)[:80]))
        ok("toda rota documentada e roteada pelo servidor -- nenhuma «rota "
           "desconhecida»", not fora, str(fora[:5]))

    print("\n== 10. o explorador, na porta dele ==")
    codigo, cabs, pagina = http(PORTA_SWAGGER, "GET", "/")
    ok("GET / responde 200", codigo == 200, str(codigo))
    ok("e HTML", "text/html" in cabs.get("content-type", ""), cabs.get("content-type", ""))
    ok("com o nome do servico no titulo", "<title>Loja</title>" in pagina)
    ok("a politica de seguranca nao deixa origem de fora entrar",
       "anthropic" not in cabs.get("content-security-policy", ""),
       cabs.get("content-security-policy", "")[:120])
    ok("nao ha texto de tela cravado na moldura -- os rotulos vem de /idiomas",
       "Operações" not in pagina and "Operations" not in pagina)
    codigo, _, idiomas = http(PORTA_SWAGGER, "GET", "/idiomas?idioma=Ingles")
    ok("o explorador serve os textos da fabrica", codigo == 200, str(codigo))
    try:
        t = json.loads(idiomas)["textos"]
        ok("e em ingles quando pedido", t.get("tela.api_operacoes") == "Operations",
           t.get("tela.api_operacoes", ""))
        ok("com o aviso do Bearer em claro traduzido",
           "cleartext" in t.get("tela.api_aviso_claro", ""),
           t.get("tela.api_aviso_claro", "")[:80])
    except (ValueError, KeyError) as e:
        ok("os textos do explorador chegam", False, str(e))
    codigo, _, _ = http(PORTA_SWAGGER, "POST", "/v1/excluir", {})
    ok("a porta do explorador NAO despacha operacao nenhuma", codigo == 405,
       str(codigo))

    print("\n== 11. o codigo HTTP conta o que aconteceu ==")
    codigo, r = rest("/v1/ler", {"tabela": "clientes", "rowid": 9999}, sessao=sessao)
    ok("registro que nao existe leva 404", codigo == 404, f"{codigo} {json.dumps(r)[:160]}")
    codigo, r = rest("/v1/inserir",
                     {"tabela": "clientes", "valores": {"id": "isto nao e numero"}},
                     sessao=sessao)
    ok("dado que o esquema recusa leva 400", codigo == 400,
       f"{codigo} {json.dumps(r)[:200]}")
    ok("e o envelope traz codigo, nome e classe",
       all(k in r for k in ("codigo", "nome", "classe")), json.dumps(r)[:200])

    print("\n== 12. o acesso ficou no log, com a operacao certa ==")
    # Pelo proprio REST, com a sessao do dono: o log e uma operacao como
    # qualquer outra, e le-lo por aqui prova de uma vez que ela funciona e que
    # o que o REST atendeu foi anotado.
    codigo, r = rest("/v1/acessos", {"limite": 500}, sessao=sessao)
    ops = [a.get("op") for a in r.get("resultado", {}).get("acessos", [])]
    ok("o log de acessos abre pelo REST", codigo == 200 and ops,
       f"{codigo} {json.dumps(r)[:160]}")
    ok("o REST anota no MESMO log de acessos", "ler" in ops, str(sorted(set(ops))[:12]))

    print("\n== 13. a recusa da lista negra CHEGA em quem foi barrado ==")
    # Cinco tokens errados bloqueiam o IP -- e e o comportamento certo. O que
    # se prova aqui e o pedido SEGUINTE: ele tem de trazer a recusa 403 com o
    # motivo, e nao um `Connection reset`. A bancada achou exatamente isso na
    # primeira rodada: o servidor escrevia a recusa e o RST a engolia, porque
    # o soquete fechava com o corpo do pedido por ler.
    for _ in range(6):
        try:
            rest("/v1/ping", {"enchimento": "x" * 2000}, token="chute")
        except OSError:
            pass
    barrado, recusa = None, {}
    try:
        barrado, recusa = rest("/v1/ping", {"enchimento": "x" * 20000}, token="chute")
    except OSError as e:
        ok("a recusa por bloqueio chega em vez de a conexao ser cortada",
           False, f"{type(e).__name__}: {e}")
    if barrado is not None:
        ok("a recusa por bloqueio chega em vez de a conexao ser cortada",
           barrado == 403, f"{barrado} {json.dumps(recusa)[:200]}")
        ok("e ela diz o motivo e ate quando",
           "ate" in json.dumps(recusa) or "bloque" in json.dumps(recusa).lower(),
           json.dumps(recusa)[:200])
    # Este passo e o ULTIMO de proposito, e a razao ficou clara medindo: a
    # lista negra vale para o servidor INTEIRO, entao depois dela nem a porta
    # de dados atende esta maquina -- nem para pedir `desbloquear`. Poe-lo no
    # meio fazia os passos seguintes medirem o bloqueio em vez do que eles
    # queriam medir.

    print("\n== resumo ==")
    print(f"  {sum(1 for _, o, _ in PASSOS if o)}/{len(PASSOS)} passos")
    for f in FALHAS:
        print("  FALHA:", f)
    return 1 if FALHAS else 0


if __name__ == "__main__":
    codigo = 1
    try:
        codigo = main()
    finally:
        derrubar()
    sys.exit(codigo)
