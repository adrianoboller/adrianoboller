#!/usr/bin/env python3
"""Bateria de INJECAO DE SQL, log das tentativas e bloqueio automatico.

    python3 bancada/seguranca/injecao.py

Pergunta do dono, 07/09/2026: *«testes que devem bloquear tentativas de SQL
injector, log das tentativas e bloqueio automatico do ip no firewall colocando
uma regra de blacklist»*.

O tradutor de `crates/phxsql-sql/` **analisa** o texto e reserializa o pedido:
ele nao concatena string, entao a injecao classica nao teria por onde entrar.
Isso e uma afirmacao ate alguem exercitar -- e e o que esta bateria faz, pelo
soquete, contra um `phxsqld` de pe.

# As tres coisas que esta bateria aprendeu na primeira corrida

1. **Recusar nao e o veredito; a tabela em pe e.** Um motor pode recusar a
   frase e ainda assim ter executado metade dela. Cada rodada aqui conta as
   linhas e lista as tabelas ANTES e DEPOIS.
2. **O `acessos.log` guarda a recusa, nao o texto.** Ele registra `op`, `ip`,
   `ok:false` e o codigo -- o SQL tentado nao esta la. Quem quiser o texto
   liga o Profiler, que vem desligado.
3. **Ninguem bloqueia por erro de sintaxe.** As duas gravidades da politica
   sao comando/base proibidos e credencial errada; SQL torto nao e nenhuma
   das duas. A bateria mede quantas tentativas passam por minuto sem bloqueio
   em vez de supor.
"""

import ast
import glob
import http.client
import json
import os
import shutil
import socket
import subprocess
import sys
import time

RAIZ = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
BINARIO = os.path.join(RAIZ, "target/release/phxsqld")
BASE = f"/tmp/phx-f6-{os.getpid()}"

PORTA = int(os.environ.get("PHX_INJ_PORTA", "6605"))
PORTA_REST = PORTA + 1
# O servidor do caso 5b: mesma bateria, `contar_injecao_sql` ligado.
PORTA_LIGADO = PORTA + 2

PLACAR = []


def caso(numero, titulo, esperado, real, veredito):
    PLACAR.append((numero, titulo, veredito))
    print(f"\n[{numero}] {titulo}")
    print(f"    esperado: {esperado}")
    for linha in str(real).splitlines() or [""]:
        print(f"    real:     {linha}")
    print(f"    veredito: {veredito}")


def corte(texto, n=200):
    t = str(texto)
    return t if len(t) <= n else t[:n] + " …"


class Servidor:
    """Sobe um `phxsqld` de verdade e o derruba no fim, aconteca o que acontecer.

    Copiada de `bancada/alfanumerica/sonda.py`. As duas diferencas: a porta
    REST tambem sobe, e o comando de firewall aponta para um `touch` inofensivo
    -- aplicar `iptables` de verdade num conteiner compartilhado derrubaria a
    rede de quem esta ao lado.
    """

    def __init__(self, nome, porta, porta_rest=None, seguranca=None):
        self.nome, self.porta, self.porta_rest = nome, porta, porta_rest
        # O que se acrescenta a politica de FABRICA -- so o caso 5b usa, para
        # medir o interruptor `contar_injecao_sql` ligado.
        self.seguranca = seguranca or {}
        self.dir = f"{BASE}/{nome}"
        self.cfg = f"{self.dir}/config.json"
        self.log = f"{self.dir}/acessos.log"
        self.blacklist = f"{self.dir}/blacklist.json"

    def __enter__(self):
        if not os.path.exists(BINARIO):
            raise SystemExit(
                f"{BINARIO} nao existe. Rode antes:\n"
                "  flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server"
            )
        os.makedirs(self.dir + "/dados", exist_ok=True)
        # A vitima do teste de shell: se algum `sh -c` rodar o comando de
        # firewall, este arquivo some.
        open(self.dir + "/alvo.txt", "w").write("sobrevivi\n")
        cfg = {
            "bind": f"127.0.0.1:{self.porta}",
            "base": self.dir + "/dados",
            "token": "t",
            "web": {"ligado": False},
            "max_linhas": 200000,
            "log_acessos": self.log,
            "seguranca": {
                # De FABRICA: nenhum comando e nenhuma base proibidos. E o
                # config.json que sai do `--exemplo 1`, e e o que faz a
                # medicao do caso 5 valer para quem nao configurou nada.
                "comandos_proibidos": [],
                "bases_proibidas": [],
                "tentativas_para_bloqueio": 1,
                "tentativas_ate_bloquear": 5,
                "janela_minutos": 10,
                "bloqueio_minutos": 60,
                "whitelist": [],
                "blacklist": self.blacklist,
                "firewall": {
                    "ligado": True,
                    # `; rm alvo.txt` no MEIO do argumento: sem shell, isto e
                    # parte do nome do arquivo e nada apaga nada.
                    "bloquear": ["/usr/bin/touch", "marca-{ip}.txt; rm alvo.txt"],
                    "desbloquear": ["/usr/bin/touch", "solto-{ip}.txt"],
                },
            },
        }
        cfg["seguranca"].update(self.seguranca)
        if self.porta_rest:
            cfg["rest"] = {
                "ligado": True,
                "bind": f"127.0.0.1:{self.porta_rest}",
                "swagger_ligado": False,
            }
        with open(self.cfg, "w") as f:
            json.dump(cfg, f)
        self.p = subprocess.Popen(
            [BINARIO, "--config", self.cfg],
            cwd=self.dir,  # o firewall de mentira escreve AQUI, nunca no repositorio
            stdout=open(self.dir + "/saida.log", "w"),
            stderr=subprocess.STDOUT,
        )
        for _ in range(100):
            try:
                socket.create_connection(("127.0.0.1", self.porta), 0.2).close()
                break
            except OSError:
                time.sleep(0.1)
        else:
            raise SystemExit(f"o servidor nao subiu na porta {self.porta}")
        self.s = socket.create_connection(("127.0.0.1", self.porta), 20)
        self.s.settimeout(20)
        self.f = self.s.makefile("rwb")
        return self

    def __exit__(self, *_):
        try:
            self.f.close()
            self.s.close()
        except OSError:
            pass
        # Pelo PID guardado, nunca por `pkill -f`.
        self.p.terminate()
        try:
            self.p.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.p.kill()

    def pedir(self, **kw):
        kw.setdefault("token", "t")
        self.f.write((json.dumps(kw) + "\n").encode())
        self.f.flush()
        r = json.loads(self.f.readline().decode())
        if isinstance(r.get("resultado"), dict):
            fora = {k: v for k, v in r.items() if k != "resultado"}
            r = {**r["resultado"], **fora}
        return r

    def cru(self, **kw):
        """A linha da resposta como veio, sem desembrulhar `resultado`."""
        kw.setdefault("token", "t")
        self.f.write((json.dumps(kw) + "\n").encode())
        self.f.flush()
        return self.f.readline().decode().strip()

    def solto(self, **kw):
        """Uma conexao nova e descartavel -- para quando o IP pode ser bloqueado."""
        s = socket.create_connection(("127.0.0.1", self.porta), 10)
        s.settimeout(10)
        f = s.makefile("rwb")
        try:
            kw.setdefault("token", "t")
            f.write((json.dumps(kw) + "\n").encode())
            f.flush()
            l = f.readline()
            return l.decode().strip() if l else "<A CONEXAO FECHOU SEM RESPONDER>"
        finally:
            f.close()
            s.close()

    def rest(self, caminho, corpo, token="t"):
        c = http.client.HTTPConnection("127.0.0.1", self.porta_rest, timeout=10)
        try:
            c.request(
                "POST",
                caminho,
                json.dumps(corpo),
                {"Content-Type": "application/json", "Authorization": f"Bearer {token}"},
            )
            r = c.getresponse()
            return r.status, r.read().decode()
        finally:
            c.close()

    def desbloquear(self, ip="127.0.0.1"):
        # cwd no diretorio do servidor: o comando de firewall usa caminho
        # relativo, e sem isto ele escreveria dentro do repositorio.
        r = subprocess.run(
            [BINARIO, "--desbloquear", ip, "--config", self.cfg],
            cwd=self.dir,
            capture_output=True,
        )
        return r.stdout.decode().strip()

    def bloqueios(self):
        if not os.path.exists(self.blacklist):
            return []
        return json.load(open(self.blacklist)).get("bloqueios", [])

    def acessos(self):
        if not os.path.exists(self.log):
            return []
        return [json.loads(l) for l in open(self.log) if l.strip()]


# --------------------------------------------------------------- o alvo

COLUNAS = [
    {"nome": "id", "tipo": "Int8"},
    {"nome": "nome", "tipo": "Str(60)", "obrigatoria": True},
    {"nome": "cidade", "tipo": "Str(40)"},
]


def semear(sv):
    sv.pedir(op="criar_database", database="loja")
    sv.pedir(
        op="criar_tabela",
        database="loja",
        tabela="clientes",
        colunas=COLUNAS,
        # O indice sobre `nome` existe para que o WHERE por nome tenha por onde
        # responder: sem ele o motor recusa o SELECT e a bateria nunca chegaria
        # a provar que a aspa virou dado.
        indices=[
            {"nome": "porId", "colunas": ["id"], "unico": True},
            {"nome": "porNome", "colunas": ["nome"], "unico": False},
        ],
    )
    for i, nome, cidade in ((1, "Alves", "Blumenau"), (2, "Silva", "Joinville")):
        sv.pedir(op="inserir", database="loja", tabela="clientes",
                 linha={"id": i, "nome": nome, "cidade": cidade})


def retrato(sv):
    """Quantas tabelas, quantas linhas -- o antes e o depois de cada rodada."""
    t = sv.pedir(op="tabelas", database="loja")
    v = sv.pedir(op="varrer", database="loja", tabela="clientes", max=1000)
    nomes = t.get("tabelas") or []
    if nomes and isinstance(nomes[0], dict):
        nomes = [x.get("tabela") or x.get("nome") for x in nomes]
    return sorted(nomes), v.get("registros")


# =====================================================================
ARSENAL = [
    ("aspa solta / tautologia", "SELECT * FROM clientes WHERE nome = '' OR '1'='1'"),
    ("segundo comando: DROP", "SELECT * FROM clientes; DROP TABLE clientes; --"),
    ("segundo comando: DELETE", "SELECT * FROM clientes; DELETE FROM clientes"),
    ("UNION SELECT", "SELECT * FROM clientes UNION SELECT * FROM clientes"),
    ("comentario /* */ no meio", "SELECT /* comentario */ * FROM clientes"),
    ("comentario como separador", "SELECT * FROM clientes/**/WHERE/**/nome='Alves'"),
    ("'; EXEC", "SELECT * FROM clientes WHERE nome = 'x'; EXEC xp_cmdshell('dir')"),
    ("comentario -- no fim", "SELECT * FROM clientes WHERE nome = 'Alves' -- e o resto"),
    ("DROP direto", "DROP TABLE clientes"),
    ("empilhado com ponto-e-virgula solto", "SELECT * FROM clientes ;;; DROP TABLE clientes"),
    ("byte nulo no texto", "SELECT * FROM clientes\x00"),
    ("aspa escapada como DADO", "SELECT * FROM clientes WHERE nome = 'x'' OR ''1''=''1'"),
]


def parte_1_sql(sv):
    print("\n=== 1. Os classicos, pela op `sql`\n")
    antes = retrato(sv)
    linhas = []
    executou_segundo = False
    for nome, texto in ARSENAL:
        r = sv.cru(op="sql", database="loja", texto=texto)
        linhas.append(f"{nome}:")
        linhas.append(f"    {texto!r}")
        linhas.append(f"    -> {corte(r, 190)}")
    depois = retrato(sv)
    positivo = sv.cru(op="sql", database="loja", texto="SELECT nome FROM clientes LIMIT 2")
    linhas.append(f"CONTROLE POSITIVO, SELECT legitimo: {corte(positivo, 200)}")
    linhas.append(f"tabelas/linhas ANTES:  {antes}")
    linhas.append(f"tabelas/linhas DEPOIS: {depois}")
    caso(
        1,
        f"{len(ARSENAL)} injecoes classicas pela op `sql`",
        "ou erro de sintaxe, ou a aspa tratada como dado -- NUNCA o segundo comando; "
        "e `clientes` continua existindo com as mesmas linhas",
        "\n".join(linhas),
        "PASSOU" if antes == depois and '"ok":true' in positivo else "FALHOU",
    )
    return antes == depois


def parte_1b_aspa_como_dado(sv):
    print("\n=== 1b. A aspa vira DADO, e o dado volta inteiro\n")
    veneno = "'; DROP TABLE clientes; --"
    sv.pedir(op="inserir", database="loja", tabela="clientes",
             linha={"id": 3, "nome": veneno, "cidade": "x"})
    # O mesmo texto, agora como literal SQL: aspa simples dobrada.
    literal = veneno.replace("'", "''")
    r = sv.pedir(op="sql", database="loja", texto=f"SELECT * FROM clientes WHERE nome = '{literal}'")
    achadas = r.get("linhas") or []
    tabelas, registros = retrato(sv)
    caso(
        "1b",
        "o texto de injecao como VALOR: gravado pelo protocolo, achado pelo SQL",
        "o SELECT com a aspa dobrada acha exatamente a linha gravada, e a tabela continua de pe",
        f"gravado pelo protocolo: {veneno!r}\n"
        f"SQL: SELECT * FROM clientes WHERE nome = '{literal}'\n"
        f"linhas achadas: {len(achadas)} -> {corte(json.dumps(achadas, ensure_ascii=False), 240)}\n"
        f"tabelas: {tabelas}   registros: {registros}",
        "PASSOU"
        if len(achadas) == 1 and achadas[0]["nome"] == veneno and "clientes" in tabelas
        else "FALHOU",
    )


def parte_2_protocolo(sv):
    print("\n=== 2. Pelo protocolo JSON: valor, nome de coluna e nome de tabela\n")
    antes = retrato(sv)
    valor = sv.cru(op="inserir", database="loja", tabela="clientes",
                   linha={"id": 4, "nome": "'; DROP TABLE clientes; --", "cidade": "Curitiba"})
    coluna = sv.cru(op="inserir", database="loja", tabela="clientes",
                    linha={"id": 5, "nome": "ok", "cidade'); DROP TABLE clientes; --": "x"})
    tabela = sv.cru(op="varrer", database="loja", tabela="clientes; DROP TABLE clientes", max=1)
    base = sv.cru(op="varrer", database="loja; DROP DATABASE loja", tabela="clientes", max=1)
    depois = retrato(sv)
    caso(
        2,
        "injecao no VALOR, no nome da COLUNA, no nome da TABELA e no da BASE",
        "o valor vira dado; o nome de coluna com pontuacao e RECUSADO por nome; "
        "tabela e base com pontuacao nao viram comando -- nao ha concatenacao de "
        "texto em lugar nenhum",
        f"valor:  {corte(valor, 150)}\n"
        f"coluna: {corte(coluna, 150)}\n"
        f"tabela: {corte(tabela, 190)}\n"
        f"base:   {corte(base, 190)}\n"
        f"tabelas/linhas ANTES:  {antes}\n"
        f"tabelas/linhas DEPOIS: {depois}   (a linha nova E a do valor envenenado)",
        "PASSOU" if depois[0] == antes[0] and depois[1] == antes[1] + 1 else "FALHOU",
    )


def parte_3_rest(sv):
    print("\n=== 3. Pela porta REST\n")
    antes = retrato(sv)
    st_ok, c_ok = sv.rest("/v1/sql", {"database": "loja", "texto": "SELECT nome FROM clientes LIMIT 1"})
    st_inj, c_inj = sv.rest("/v1/sql", {"database": "loja",
                                        "texto": "SELECT * FROM clientes; DROP TABLE clientes"})
    st_sem, c_sem = sv.rest("/v1/sql", {"database": "loja", "texto": "SELECT * FROM clientes"},
                            token="token-errado")
    st_cam, c_cam = sv.rest("/v1/ping", {"op": "excluir_tabela", "database": "loja",
                                         "tabela": "clientes", "confirmar": "clientes"})
    depois = retrato(sv)
    caso(
        3,
        "o REST recebe SQL? e o corpo consegue mandar mais do que o caminho?",
        "POST /v1/sql existe e passa pelo MESMO despachar: a injecao recusa igual; "
        "token errado da 401; `op` no corpo diferente do caminho e recusado",
        f"CONTROLE POSITIVO POST /v1/sql legitimo: {st_ok} {corte(c_ok, 170)}\n"
        f"POST /v1/sql com injecao:             {st_inj} {corte(c_inj, 200)}\n"
        f"POST /v1/sql com token errado:        {st_sem} {corte(c_sem, 150)}\n"
        f"POST /v1/ping com op=excluir_tabela no corpo: {st_cam} {corte(c_cam, 190)}\n"
        f"tabelas/linhas ANTES:  {antes}\n"
        f"tabelas/linhas DEPOIS: {depois}",
        "PASSOU"
        if st_ok == 200 and st_inj == 400 and st_sem == 401 and antes == depois
        else "FALHOU",
    )


def parte_4_log(sv):
    print("\n=== 4. O log das tentativas: o que fica registrado\n")
    marca = len(sv.acessos())
    sv.pedir(op="profiler_ligar")
    tentativa = "SELECT * FROM clientes; DROP TABLE clientes; --"
    sv.pedir(op="sql", database="loja", texto=tentativa)
    prof = sv.pedir(op="profiler")
    novas = sv.acessos()[marca:]
    do_sql = [a for a in novas if a["op"] == "sql"]
    eventos = [e for e in (prof.get("eventos") or []) if e["op"] == "sql"]
    tem_texto_no_acessos = any(tentativa in json.dumps(a) for a in do_sql)
    tem_texto_no_profiler = any(tentativa in json.dumps(e) for e in eventos)
    cravado = '"token":"t"'
    token_em_claro = any(cravado in json.dumps(e) for e in eventos)
    caso(
        4,
        "a tentativa de injecao aparece no acessos.log e no profiler?",
        "o acessos.log registra a RECUSA (ip, quando, op, ok:false, codigo) mas NAO "
        "o texto tentado; o Profiler registra o texto inteiro, com o token redigido",
        f"acessos.log:  {corte(json.dumps(do_sql[-1], ensure_ascii=False), 300)}\n"
        f"o texto tentado esta no acessos.log? {tem_texto_no_acessos}\n"
        f"profiler:     {corte(json.dumps(eventos[-1], ensure_ascii=False), 340)}\n"
        f"o texto tentado esta no profiler?    {tem_texto_no_profiler}\n"
        f"o token aparece em claro no profiler? {token_em_claro}",
        "ACHADO" if not tem_texto_no_acessos and tem_texto_no_profiler else "FALHOU",
    )
    sv.pedir(op="profiler_desligar")


def parte_5_bloqueio(sv):
    print("\n=== 5. Injecao sintatica bloqueia o IP? (a medicao, nao o palpite)\n")
    antes = sv.bloqueios()
    janela = 2.0

    def medir(manda):
        n = 0
        recusas = 0
        fim = time.time() + janela
        while time.time() < fim:
            if '"ok":false' in manda(n):
                recusas += 1
            n += 1
        return n, recusas

    texto = "SELECT * FROM clientes; DROP TABLE clientes; -- "
    mesma, r1 = medir(lambda i: sv.cru(op="sql", database="loja", texto=texto + str(i)))
    nova, r2 = medir(lambda i: sv.solto(op="sql", database="loja", texto=texto + str(i)))
    depois = sv.bloqueios()
    ainda = sv.cru(op="ping")
    caso(
        5,
        f"injecao a jato por {janela:.0f}s em cada caminho: alguem bloqueia?",
        "com o interruptor DESLIGADO -- que e o padrao de fabrica -- ninguem bloqueia, "
        "e e o comportamento de sempre: erro de sintaxe nao e comando proibido nem "
        "credencial errada. O caso 5b mede o outro lado",
        f"pela MESMA conexao:      {mesma} tentativas em {janela:.0f}s "
        f"({int(mesma / janela * 60)} por minuto), {r1} recusadas\n"
        f"uma CONEXAO por tentativa: {nova} tentativas em {janela:.0f}s "
        f"({int(nova / janela * 60)} por minuto), {r2} recusadas\n"
        f"bloqueios antes: {antes}   depois: {depois}\n"
        f"o IP continua entrando: {corte(ainda, 120)}",
        "PASSOU" if not depois and '"ok":true' in ainda else "FALHOU",
    )
    return int(nova / janela * 60)


def sql_legitimo_do_exercitar():
    """Os comandos SQL LEGITIMOS, lidos do fonte de `bancada/sql-exemplos/`.

    A lista sai do CODIGO, e nao digitada aqui: *quando um gerador depende de
    uma lista, a lista tem de sair do codigo*. Uma copia envelheceria calada no
    dia em que alguem acrescentasse um comando la.
    """
    fonte = os.path.join(RAIZ, "bancada", "sql-exemplos", "exercitar.py")
    arv = ast.parse(open(fonte, encoding="utf-8").read())
    comandos = []
    for no in ast.walk(arv):
        # As tuplas (rotulo, sql, "ok"/"erro") da secao A.
        if isinstance(no, ast.Tuple) and len(no.elts) == 3:
            e = no.elts
            if all(isinstance(x, ast.Constant) and isinstance(x.value, str) for x in e):
                if e[2].value in ("ok", "erro"):
                    comandos.append(e[1].value)
        # E o que vai por `sv.sql("...")` -- procedimentos e gatilhos, cujo
        # CORPO e cheio de ponto-e-virgula legitimo.
        if isinstance(no, ast.Call) and isinstance(no.func, ast.Attribute) \
                and no.func.attr == "sql":
            for a in no.args:
                if isinstance(a, ast.Constant) and isinstance(a.value, str):
                    comandos.append(a.value)
    return comandos


def parte_5b_falso_positivo(sv_lig):
    """O falso positivo e o que mataria a guarda do pedido 215.

    O servidor deste caso tolera UMA tentativa leve: qualquer comando legitimo
    classificado como injecao bloqueia na hora e aparece aqui.
    """
    print("\n=== 5b. Com o interruptor LIGADO, o SQL legitimo continua passando?\n")
    semear(sv_lig)
    comandos = sql_legitimo_do_exercitar()
    recusados = 0
    for texto in comandos:
        r = sv_lig.solto(op="sql", database="loja", texto=texto)
        if '"ok":false' in r:
            recusados += 1
    bloqueios = sv_lig.bloqueios()
    caso(
        "5b",
        f"{len(comandos)} comandos SQL legitimos com `contar_injecao_sql` LIGADO",
        "nenhum bloqueio: o que a guarda conta e a recusa «sobrou X depois do fim do "
        "comando», e nao o ponto-e-virgula do corpo de um procedimento",
        f"comandos exercitados: {len(comandos)} (lidos do fonte de bancada/sql-exemplos/)\n"
        f"recusados pelo motor (tabela que nao existe, clausula que falta...): {recusados}\n"
        f"a politica deste servidor tolera 1 tentativa leve -- um falso positivo so "
        f"ja bloquearia\n"
        f"bloqueios: {json.dumps(bloqueios)}",
        "PASSOU" if not bloqueios else "FALHOU",
    )


def parte_5c_interruptor(sv_lig):
    """O `seguranca.contar_injecao_sql` LIGADO -- o outro lado do caso 5.

    Servidor proprio, e nao `config_gravar`: a secao `seguranca` nao e editavel
    pela tela de proposito (uma sessao roubada nao abre o firewall).
    """
    print("\n=== 5c. O mesmo jato com `seguranca.contar_injecao_sql` LIGADO\n")
    antes = sv_lig.bloqueios()
    texto = "SELECT * FROM clientes; DROP TABLE clientes; -- "
    tentativas = 0
    fim = time.time() + 2.0
    while time.time() < fim and not sv_lig.bloqueios():
        sv_lig.solto(op="sql", database="loja", texto=texto + str(tentativas))
        tentativas += 1
    depois = sv_lig.bloqueios()
    ainda = sv_lig.solto(op="ping")
    caso(
        "5c",
        "com o interruptor LIGADO, quantas injecoes ate o IP cair na blacklist?",
        "o comando empilhado conta pela politica LEVE que ja existe "
        "(`tentativas_ate_bloquear`), e o IP para de entrar",
        f"tentativas ate o bloqueio: {tentativas} "
        f"(a politica deste servidor tolera 1 na janela)\n"
        f"bloqueios antes: {json.dumps(antes)}\n"
        f"bloqueios depois: {json.dumps(depois)}\n"
        f"o `ping` seguinte: {corte(ainda, 170)}",
        "PASSOU"
        if depois
        and "empilhado" in depois[0].get("motivo", "")
        and '"ok":true' not in ainda
        else "FALHOU",
    )
    sv_lig.desbloquear()


def parte_6_firewall(sv):
    print("\n=== 6. A regra de firewall e a exportacao da lista\n")
    # Um bloqueio de VERDADE, pelo caminho de fabrica: cinco credenciais erradas.
    for _ in range(6):
        sv.solto(op="ping", token="token-errado")
    b = sv.bloqueios()
    marcas = sorted(os.path.basename(x) for x in glob.glob(sv.dir + "/marca-*"))
    alvo = os.path.exists(sv.dir + "/alvo.txt")
    caso(
        6,
        "firewall ligado: o comando roda SEM shell e o `{ip}` e trocado",
        "o comando `touch 'marca-{ip}.txt; rm alvo.txt'` cria UM arquivo com esse nome "
        "inteiro, e `alvo.txt` sobrevive -- se houvesse `sh -c`, ele teria sumido",
        f"blacklist.json: {corte(json.dumps(b, ensure_ascii=False), 300)}\n"
        f"arquivos criados pelo comando: {marcas}\n"
        f"alvo.txt sobreviveu? {alvo}",
        "PASSOU"
        if b
        and b[0]["firewall"]
        and alvo
        and marcas == ["marca-127.0.0.1.txt; rm alvo.txt"]
        else "FALHOU",
    )

    saidas = {}
    for fm in ("texto", "iptables", "nftables", "fail2ban"):
        # Pela conexao ja aberta: ela nasceu antes do bloqueio, e a lista de
        # bloqueio e conferida na CONEXAO. Uma conexao nova aqui seria recusada
        # -- e a bateria mediria o bloqueio em vez da exportacao.
        r = sv.cru(op="bloqueios_exportar", formato=fm)
        saidas[fm] = json.loads(r).get("resultado", {}).get("texto", r)
    root = os.geteuid() == 0
    caso(
        "6b",
        "`bloqueios_exportar` nos quatro formatos (SEGURANCA.md secao 5.1)",
        "uma linha por IP ATIVO, no texto que um firewall de verdade consome",
        "\n".join(f"{fm:9} -> {v!r}" for fm, v in saidas.items())
        + f"\nesta bateria NAO aplica a regra: rodar iptables num conteiner compartilhado "
        f"derruba a rede de quem esta ao lado (euid={os.geteuid()}, root={root})",
        "PASSOU" if all(saidas.values()) else "FALHOU",
    )
    print(f"\n    --desbloquear 127.0.0.1 -> {sv.desbloquear()}")
    soltos = sorted(os.path.basename(x) for x in glob.glob(sv.dir + "/solto-*"))
    print(f"    o comando de desbloqueio tambem rodou: {soltos}")


def main():
    print("=" * 78)
    print("BATERIA DE INJECAO DE SQL, LOG E BLOQUEIO -- PhxSql")
    print(f"UTC {time.strftime('%Y-%m-%d %H:%M:%S', time.gmtime())}   "
          f"commit {subprocess.run(['git', '-C', RAIZ, 'rev-parse', '--short', 'HEAD'], capture_output=True).stdout.decode().strip()}")
    print(f"portas {PORTA} (dados), {PORTA_REST} (REST) e {PORTA_LIGADO} "
          f"(interruptor ligado)   base {BASE}")
    print("=" * 78)
    shutil.rmtree(BASE, ignore_errors=True)
    with Servidor("injecao", PORTA, PORTA_REST) as sv, \
         Servidor("ligado", PORTA_LIGADO,
                  seguranca={"contar_injecao_sql": True,
                             # UMA tentativa leve: qualquer falso positivo do
                             # caso 5b bloqueia na hora e nao passa despercebido.
                             "tentativas_ate_bloquear": 1}) as sv_lig:
        semear(sv)
        parte_1_sql(sv)
        parte_1b_aspa_como_dado(sv)
        parte_2_protocolo(sv)
        parte_3_rest(sv)
        parte_4_log(sv)
        parte_5_bloqueio(sv)
        parte_5b_falso_positivo(sv_lig)
        parte_5c_interruptor(sv_lig)
        parte_6_firewall(sv)

    print("\n" + "=" * 78)
    passou = sum(1 for _, _, v in PLACAR if v == "PASSOU")
    falhou = sum(1 for _, _, v in PLACAR if v == "FALHOU")
    achado = sum(1 for _, _, v in PLACAR if v == "ACHADO")
    print(f"PLACAR: {len(PLACAR)} casos -- {passou} passou, {falhou} falhou, {achado} achado")
    for n, t, v in PLACAR:
        print(f"  [{n:>3}] {v:7} {t}")
    print("=" * 78)
    shutil.rmtree(BASE, ignore_errors=True)
    return 1 if falhou else 0


if __name__ == "__main__":
    sys.exit(main())
