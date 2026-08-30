#!/usr/bin/env python3
"""Prova os quatro modos de replicacao, por soquete, nas portas 5330-5339.

    cargo build --release
    python3 bancada/replicacao/modos.py [diretorio]

Oito estagios, cada um com o RESULTADO ESPERADO escrito antes de rodar:

    a) modo A (Primary -> Replica) pelas ops que um assistente chamaria;
    c2) o que a supressao de origem POUPA, contado no fio (ver a nota la:
        a hipotese de que ela e o que mata o laco morreu medida);
    b) agendamento: com cada_minutos=1 a alteracao NAO aparece antes da
       janela e aparece depois -- e sem o campo, streaming como sempre (a);
    c) bidirecional: insert em A aparece em B, insert em B aparece em A, e
       NAO volta (prova do laco morto: os eventos param de crescer);
    d) conflito: mesmo registro alterado nos dois lados, vence o carimbo mais
       recente NOS DOIS (convergencia);
    e) tabela sem chave unica: o modo bidirecional recusa com o motivo;
    f) spare: cliente comum nao le nem escreve; `spare_promover` e ele vira
       primario aceitando tudo;
    g) read replica: leitura ok, escrita recusada apontando o primario;
    h) comportamento velho: configs moldadas nos Config_exemplo_02/03 sobem
       e replicam como antes, com a recusa generica de sempre.

Este script SO derruba processos que ele mesmo criou (guarda os Popen) --
nunca `pkill phxsqld`, porque pode haver outros servidores na maquina.
A ultima linha e `RESULTADO <json>`.
"""
import json
import os
import socket
import subprocess
import sys
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
PHXSQLD = os.path.join(RAIZ, "target", "release", "phxsqld")

TOKEN = "modos"
USUARIO = "adm"
SENHA = "segredo1"

PROCESSOS = []          # (Popen, rotulo) -- so o que ESTE script subiu
SOQUETES = []           # para fechar do lado do cliente antes de derrubar
RESULTADO = {}
FALHAS = []


# ---------------------------------------------------------------- utilidades

def hash_da_senha(senha):
    saida = subprocess.run([PHXSQLD, "--senha"], input=senha + "\n",
                           capture_output=True, text=True).stdout
    return saida.split('": "')[1].split('"')[0]


def permissoes():
    return {"*": {"ler": True, "inserir": True, "alterar": True,
                  "excluir": True, "criar": True, "administrar": True,
                  "diario": True, "verificar": True, "replicar": True}}


def config_base(porta, h):
    return {
        "base": "base",
        "bind": f"127.0.0.1:{porta}",
        "token": TOKEN,
        "web": {"ligado": False},
        "usuarios": [{"login": USUARIO, "nome": "Bancada", "id": 10,
                      "senha_hash": h, "bases": permissoes()}],
    }


def origem_para(porta, nome, h, extras=None):
    o = {"nome": nome, "host": "127.0.0.1", "porta": porta, "token": TOKEN,
         "usuario": USUARIO, "senha_hash": h, "databases": ["loja"],
         "reconectar_em": 1}
    o.update(extras or {})
    return o


def subir(base, rotulo, cfg):
    d = os.path.join(base, rotulo)
    os.makedirs(d, exist_ok=True)
    caminho = os.path.join(d, "base")
    if os.path.exists(caminho):
        subprocess.run(["rm", "-rf", caminho], check=False)
    for lixo in ["replicacao-posicoes.json"]:
        try:
            os.remove(os.path.join(d, lixo))
        except OSError:
            pass
    with open(os.path.join(d, "config.json"), "w") as f:
        json.dump(cfg, f, indent=2)
    log = open(os.path.join(d, "servidor.log"), "a")
    p = subprocess.Popen([PHXSQLD], cwd=d, stdout=log,
                         stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
    PROCESSOS.append((p, rotulo))
    return p


def derrubar(*rotulos):
    """Mata SO os processos deste script (os rotulados, ou todos os nossos).

    Fecha os soquetes do lado do CLIENTE primeiro: quem fecha primeiro fica
    com o TIME_WAIT, e ele tem de ficar do nosso lado -- senao a porta do
    servidor nao volta a aceitar `bind` quando um estagio a reusa.
    """
    for s, f in SOQUETES:
        try:
            f.close()
            s.close()
        except OSError:
            pass
    SOQUETES.clear()
    for p, rot in list(PROCESSOS):
        if rotulos and rot not in rotulos:
            continue
        if p.poll() is None:
            p.terminate()
            try:
                p.wait(timeout=5)
            except subprocess.TimeoutExpired:
                p.kill()
        PROCESSOS.remove((p, rot))
    time.sleep(0.5)


def liga(porta, tentativas=100):
    ultimo = None
    for _ in range(tentativas):
        try:
            s = socket.create_connection(("127.0.0.1", porta), timeout=5)
            f = s.makefile("rwb")
            SOQUETES.append((s, f))

            def fala(pedido):
                pedido.setdefault("token", TOKEN)
                f.write((json.dumps(pedido) + "\n").encode())
                f.flush()
                return json.loads(f.readline().decode())

            r = fala({"op": "login", "usuario": USUARIO, "senha": SENHA})
            if not r.get("ok"):
                raise SystemExit(f"login na porta {porta}: {r}")
            return fala
        except OSError as e:
            ultimo = e
            time.sleep(0.2)
    raise SystemExit(f"porta {porta} nunca respondeu: {ultimo}")


def criar_tabela(fala, tabela, com_chave=True):
    fala({"op": "criar_database", "database": "loja"})
    pedido = {"op": "criar_tabela", "database": "loja", "tabela": tabela,
              "motivo_obrigatorio": False,
              "colunas": [{"nome": "id", "tipo": "Int4", "obrigatoria": True},
                          {"nome": "nome", "tipo": "Str(60)", "obrigatoria": True},
                          {"nome": "cidade", "tipo": "Str(30)"}]}
    if com_chave:
        pedido["indices"] = [{"nome": "porId", "colunas": ["id"],
                              "unico": True, "primario": True}]
    r = fala(pedido)
    if not r.get("ok"):
        raise SystemExit(f"criar {tabela}: {r}")


def inserir(fala, tabela, id_, nome, cidade="Blumenau"):
    r = fala({"op": "inserir", "database": "loja", "tabela": tabela,
              "linha": {"id": id_, "nome": nome, "cidade": cidade}})
    if not r.get("ok"):
        raise SystemExit(f"inserir em {tabela}: {r}")


def linhas_por_id(fala, tabela):
    saida, depois = {}, 0
    while True:
        r = fala({"op": "varrer", "database": "loja", "tabela": tabela,
                  "max": 1000, "depois": depois, "visao": "todas"})
        if not r.get("ok"):
            return None  # tabela nao existe (ainda)
        d = r["resultado"]
        for l in d["linhas"]:
            saida[l["id"]] = l["nome"]
        if not d["ha_mais"] or not d["linhas"]:
            return saida
        depois = d["cursor_fim"]


def eventos_de(fala, tabela):
    r = fala({"op": "posicao", "database": "loja"})
    if not r.get("ok"):
        return None
    t = r["resultado"]["tabelas"].get(tabela)
    return None if t is None else t["eventos"]


def esperar(condicao, segundos=30, passo=0.2):
    t0 = time.perf_counter()
    while time.perf_counter() - t0 < segundos:
        if condicao():
            return time.perf_counter() - t0
        time.sleep(passo)
    return None


def estagio(letra, esperado):
    print()
    print(f"--- estagio ({letra})")
    print(f"    esperado: {esperado}")


def medir(letra, ok, texto):
    marca = "ok" if ok else "FALHOU"
    print(f"    medido:   {texto}  [{marca}]")
    RESULTADO[letra] = {"ok": ok, "medido": texto}
    if not ok:
        FALHAS.append(letra)


# ------------------------------------------------------------------ estagios

def estagio_a(base, h):
    estagio("a", "B alcanca os 50 eventos de A, retratos iguais, e A continua "
                 "com 50 eventos (B nao devolve nada); posicao expoe a chave")
    subir(base, "a-source", {**config_base(5330, h), "replicacao": {
        "papel": "source", "id_servidor": "a-source", "imagem_da_linha": True}})
    a = liga(5330)
    criar_tabela(a, "clientes")
    for i in range(1, 51):
        inserir(a, "clientes", i, f"Cliente {i:03d}")

    subir(base, "a-replica", {**config_base(5331, h), "somente_leitura": True,
                              "replicacao": {"papel": "replica",
                                             "id_servidor": "a-replica",
                                             "origens": [origem_para(5330, "a-source", h)]}})
    b = liga(5331)
    t = esperar(lambda: eventos_de(b, "clientes") == 50, 30)
    de_a = linhas_por_id(a, "clientes")
    de_b = linhas_por_id(b, "clientes")
    chave = a({"op": "posicao", "database": "loja"})["resultado"]["tabelas"]["clientes"].get("chave")
    # "B nao devolve": os eventos de A sao so os 50 que A mesmo escreveu.
    eventos_a = eventos_de(a, "clientes")
    ok = t is not None and de_a == de_b and eventos_a == 50 and chave == "id"
    medir("a", ok, f"alcancou em {t and round(t,1)}s; iguais={de_a == de_b}; "
                   f"eventos_de_A={eventos_a}; chave={chave!r}")
    RESULTADO["a"]["alcance_s"] = None if t is None else round(t, 2)
    derrubar("a-source", "a-replica")


def estagio_b(base, h):
    estagio("b", "com cada_minutos=1, a linha gravada depois da 1a janela NAO "
                 "aparece na replica em 35s e aparece em ate 150s")
    subir(base, "b-source", {**config_base(5332, h), "replicacao": {
        "papel": "source", "id_servidor": "b-source", "imagem_da_linha": True}})
    c = liga(5332)
    criar_tabela(c, "clientes")
    inserir(c, "clientes", 1, "antes da janela")

    subir(base, "b-replica", {**config_base(5333, h), "somente_leitura": True,
                              "replicacao": {"papel": "replica",
                                             "id_servidor": "b-replica",
                                             "origens": [origem_para(5332, "b-source", h,
                                                                     {"cada_minutos": 1})]}})
    d = liga(5333)
    # A primeira rodada e no arranque: a linha 1 chega ja.
    t0 = esperar(lambda: eventos_de(d, "clientes") == 1, 30)
    # A rodada de verificacao (a que devolve "nada a fazer") ainda pode estar
    # no ar; tres segundos garantem que a proxima chance e SO a janela.
    time.sleep(3)
    # Agora grava DEPOIS da rodada do arranque: so a proxima janela traz.
    inserir(c, "clientes", 2, "depois da janela")
    cedo = esperar(lambda: (eventos_de(d, "clientes") or 0) >= 2, 35)
    tarde = None if cedo is not None else esperar(
        lambda: (eventos_de(d, "clientes") or 0) >= 2, 150)
    ok = t0 is not None and cedo is None and tarde is not None
    medir("b", ok, f"arranque={t0 and round(t0,1)}s; antes_da_janela="
                   f"{'APARECEU' if cedo is not None else 'nao apareceu'}; "
                   f"depois={tarde and round(tarde+35,1)}s")
    RESULTADO["b"]["janela_s"] = None if tarde is None else round(tarde + 35, 1)
    derrubar("b-source", "b-replica")


def subir_par_multi(base, h):
    subir(base, "m-alfa", {**config_base(5334, h), "replicacao": {
        "papel": "multi", "id_servidor": "alfa", "imagem_da_linha": True,
        "origens": [origem_para(5335, "beta", h)]}})
    subir(base, "m-beta", {**config_base(5335, h), "replicacao": {
        "papel": "multi", "id_servidor": "beta", "imagem_da_linha": True,
        "origens": [origem_para(5334, "alfa", h)]}})
    return liga(5334), liga(5335)


def estagio_cde(base, h):
    estagio("c", "k1 de alfa aparece em beta, k2 de beta aparece em alfa, e "
                 "NADA volta: os eventos param em 2 de cada lado")
    e, f = subir_par_multi(base, h)
    criar_tabela(e, "clientes")
    inserir(e, "clientes", 1, "nascida em alfa")
    t1 = esperar(lambda: (linhas_por_id(f, "clientes") or {}).get(1) == "nascida em alfa", 30)
    inserir(f, "clientes", 2, "nascida em beta")
    t2 = esperar(lambda: (linhas_por_id(e, "clientes") or {}).get(2) == "nascida em beta", 30)

    # A prova do laco morto: conta os eventos, espera 6 ciclos, conta de novo.
    ev_e, ev_f = eventos_de(e, "clientes"), eventos_de(f, "clientes")
    time.sleep(6)
    ev_e2, ev_f2 = eventos_de(e, "clientes"), eventos_de(f, "clientes")
    parado = (ev_e, ev_f) == (ev_e2, ev_f2)
    ok = t1 is not None and t2 is not None and parado and ev_e2 == 2 and ev_f2 == 2
    medir("c", ok, f"alfa->beta {t1 and round(t1,1)}s; beta->alfa {t2 and round(t2,1)}s; "
                   f"eventos alfa {ev_e}->{ev_e2}, beta {ev_f}->{ev_f2} "
                   f"({'parados' if parado else 'CRESCENDO: LACO VIVO'})")
    RESULTADO["c"]["eventos"] = [ev_e2, ev_f2]

    # (c2) O QUE A SUPRESSAO COMPRA, medido no fio -- e nao suposto.
    #
    # Medir isto foi consequencia de uma hipotese que MORREU: repor o defeito
    # (tirar o filtro do `para`) nao fez o laco viver, nem com a segunda
    # guarda fora tambem. A razao esta no estagio (d): a regra do conflito e
    # IDEMPOTENTE -- carimbo e origem iguais nao vencem --, entao o evento que
    # volta e descartado sem gerar escrita. O que a supressao evita, entao,
    # nao e o laco infinito: e o evento atravessar a rede e ser decodificado
    # de volta a toa. Aqui esta o numero.
    def quantos(fala, para):
        pedido = {"op": "replicar", "database": "loja", "tabela": "clientes",
                  "desde": 0, "max": 500}
        if para:
            pedido["para"] = para
        return len(fala(pedido)["resultado"]["eventos"])

    com = quantos(e, "beta")       # beta perguntando ao diario de alfa
    sem = quantos(e, None)         # uma replica classica, sem o campo
    outro = quantos(e, "gama")     # um terceiro, que nao escreveu nada
    poupado = 0 if sem == 0 else round(100 * (sem - com) / sem)
    ok2 = com < sem and sem == outro
    medir("c2", ok2, f"do diario de alfa: beta leva {com} evento(s), uma replica "
                     f"sem `para` leva {sem}, um terceiro leva {outro} -- a "
                     f"supressao poupa {poupado}% do trafego de volta, e nao "
                     f"muda o que os outros recebem")
    RESULTADO["c2"]["eventos"] = {"para_beta": com, "sem_para": sem, "para_gama": outro}

    estagio("d", "mesma chave alterada nos dois lados: vence o carimbo mais "
                 "recente NOS DOIS servidores (convergencia)")
    # k1: altera em alfa, espera o relogio andar, altera em beta -> beta vence.
    rowid_e = e({"op": "buscar", "database": "loja", "tabela": "clientes",
                 "indice": "porId", "chave": [1]})["resultado"]["linhas"][0]["rowid"]
    rowid_f = f({"op": "buscar", "database": "loja", "tabela": "clientes",
                 "indice": "porId", "chave": [1]})["resultado"]["linhas"][0]["rowid"]
    e({"op": "atualizar", "database": "loja", "tabela": "clientes",
       "rowid": rowid_e, "linha": {"id": 1, "nome": "de alfa, mais velha", "cidade": "Curitiba"}})
    time.sleep(1.2)
    f({"op": "atualizar", "database": "loja", "tabela": "clientes",
       "rowid": rowid_f, "linha": {"id": 1, "nome": "de beta, mais nova", "cidade": "Bruxelas"}})
    conv = esperar(lambda: (linhas_por_id(e, "clientes") or {}).get(1) == "de beta, mais nova"
                   and (linhas_por_id(f, "clientes") or {}).get(1) == "de beta, mais nova", 30)
    # E no sentido contrario, sobre k2: beta primeiro, alfa por ultimo.
    rowid_f2 = f({"op": "buscar", "database": "loja", "tabela": "clientes",
                  "indice": "porId", "chave": [2]})["resultado"]["linhas"][0]["rowid"]
    rowid_e2 = e({"op": "buscar", "database": "loja", "tabela": "clientes",
                  "indice": "porId", "chave": [2]})["resultado"]["linhas"][0]["rowid"]
    f({"op": "atualizar", "database": "loja", "tabela": "clientes",
       "rowid": rowid_f2, "linha": {"id": 2, "nome": "de beta, mais velha", "cidade": "Gent"}})
    time.sleep(1.2)
    e({"op": "atualizar", "database": "loja", "tabela": "clientes",
       "rowid": rowid_e2, "linha": {"id": 2, "nome": "de alfa, mais nova", "cidade": "Itajai"}})
    conv2 = esperar(lambda: (linhas_por_id(e, "clientes") or {}).get(2) == "de alfa, mais nova"
                    and (linhas_por_id(f, "clientes") or {}).get(2) == "de alfa, mais nova", 30)
    ok = conv is not None and conv2 is not None
    medir("d", ok, f"k1: beta venceu nos dois em {conv and round(conv,1)}s; "
                   f"k2: alfa venceu nos dois em {conv2 and round(conv2,1)}s")

    estagio("e", "tabela SEM chave unica: o bidirecional recusa com o motivo, "
                 "e a linha nao atravessa")
    criar_tabela(e, "log_livre", com_chave=False)
    inserir(e, "log_livre", 1, "nao devia viajar")
    time.sleep(4)
    em_f = linhas_por_id(f, "log_livre")
    estado = f({"op": "replicacao_estado"})["resultado"]
    recusas = estado["origens"].get("alfa", {}).get("recusas", {})
    motivo = recusas.get("loja/log_livre", "")
    ok = (not em_f) and "chave unica" in motivo
    medir("e", ok, f"linhas em beta: {em_f}; motivo da recusa: {motivo!r}")
    derrubar("m-alfa", "m-beta")


def estagio_f(base, h):
    estagio("f", "spare: varrer e inserir recusados com SPARE_EM_ESPERA (4004); "
                 "apos spare_promover, papel=source e os dois passam")
    subir(base, "f-source", {**config_base(5336, h), "replicacao": {
        "papel": "source", "id_servidor": "f-source", "imagem_da_linha": True}})
    g = liga(5336)
    criar_tabela(g, "clientes")
    inserir(g, "clientes", 1, "so no primario")

    subir(base, "f-spare", {**config_base(5337, h), "somente_leitura": True,
                            "replicacao": {"papel": "spare",
                                           "id_servidor": "f-spare",
                                           "origens": [origem_para(5336, "f-source", h)]}})
    hh = liga(5337)
    esperar(lambda: eventos_de(hh, "clientes") == 1, 30)

    ler = hh({"op": "varrer", "database": "loja", "tabela": "clientes"})
    gravar = hh({"op": "inserir", "database": "loja", "tabela": "clientes",
                 "linha": {"id": 9, "nome": "cliente comum", "cidade": "x"}})
    recusou = (not ler.get("ok") and ler.get("nome") == "SPARE_EM_ESPERA"
               and not gravar.get("ok") and gravar.get("codigo") == 4004)
    # Monitoramento continua enxergando.
    ping = hh({"op": "ping"})["resultado"]["papel"]
    soma = hh({"op": "checksum", "database": "loja", "tabela": "clientes"}).get("ok")

    promo = hh({"op": "spare_promover"})
    papel2 = hh({"op": "ping"})["resultado"]["papel"]
    ler2 = hh({"op": "varrer", "database": "loja", "tabela": "clientes"})
    gravar2 = hh({"op": "inserir", "database": "loja", "tabela": "clientes",
                  "linha": {"id": 9, "nome": "agora sim", "cidade": "x"}})
    ok = (recusou and ping == "spare" and soma and promo.get("ok")
          and papel2 == "source" and ler2.get("ok") and gravar2.get("ok"))
    medir("f", ok, f"antes: varrer={ler.get('nome')}, inserir={gravar.get('codigo')}, "
                   f"ping={ping}, checksum={'ok' if soma else 'NEGADO'}; "
                   f"depois: papel={papel2}, varrer={'ok' if ler2.get('ok') else 'NEGADO'}, "
                   f"inserir={'ok' if gravar2.get('ok') else 'NEGADO'}")
    derrubar("f-source", "f-spare")


def estagio_g(base, h):
    estagio("g", "read replica: leitura ok; escrita recusada com "
                 "REDIRECIONA (4003) apontando 127.0.0.1:5338")
    subir(base, "g-source", {**config_base(5338, h), "replicacao": {
        "papel": "source", "id_servidor": "g-source", "imagem_da_linha": True}})
    i = liga(5338)
    criar_tabela(i, "clientes")
    inserir(i, "clientes", 1, "para o relatorio")

    subir(base, "g-rr", {**config_base(5339, h), "somente_leitura": True,
                         "replicacao": {"papel": "read_replica",
                                        "id_servidor": "g-rr",
                                        "origens": [origem_para(5338, "g-source", h)]}})
    j = liga(5339)
    esperar(lambda: eventos_de(j, "clientes") == 1, 30)
    ler = j({"op": "varrer", "database": "loja", "tabela": "clientes"})
    gravar = j({"op": "inserir", "database": "loja", "tabela": "clientes",
                "linha": {"id": 2, "nome": "nao entra", "cidade": "x"}})
    # Os DOIS nomes, e nao um. O erro se chamava `ESCRITA_NA_REPLICA` e virou
    # `REDIRECIONA` quando o cluster passou a usar o mesmo desvio (`error.rs`,
    # `PhxError::Redireciona`) -- e esta prova ficou para tras, reprovando o
    # estagio (g) sem ninguem saber, porque ela nao estava em portao nenhum.
    # Foi a bateria unica (`provar.py`) que a trouxe para a luz.
    #
    # Aceitar os dois nao e frouxidao: o `replica.rs:142` tambem le os dois do
    # fio, de proposito, para uma replica de hoje entender um source antigo. A
    # prova que exigisse so o nome novo passaria a mentir contra esse mesmo
    # servidor antigo, e e ele que o codigo promete atender.
    ok = (ler.get("ok") and len(ler["resultado"]["linhas"]) == 1
          and not gravar.get("ok")
          and gravar.get("nome") in ("REDIRECIONA", "ESCRITA_NA_REPLICA")
          and gravar.get("codigo") == 4003 and "127.0.0.1:5338" in gravar.get("erro", ""))
    medir("g", ok, f"leitura={'ok' if ler.get('ok') else 'NEGADA'}; escrita: "
                   f"{gravar.get('nome')} {gravar.get('codigo')} -> {gravar.get('erro','')!r}")
    derrubar("g-source", "g-rr")


def estagio_h(base, h):
    estagio("h", "configs moldadas nos Config_exemplo_02/03 (source/replica de "
                 "hoje) replicam como antes, e a recusa de escrita e a "
                 "ACESSO_NEGADO generica de sempre")
    # O molde dos exemplos: papel source com replicas_autorizadas, replica
    # multi-source-capaz com somente_leitura -- os campos que os exemplos usam.
    subir(base, "h-source", {**config_base(5330, h),
                             "ips_permitidos": ["127.0.0.1"],
                             "replicacao": {"papel": "source",
                                            "id_servidor": "curitiba-01",
                                            "replicas_autorizadas": ["127.0.0.1"]}})
    a = liga(5330)
    criar_tabela(a, "clientes")
    for k in range(1, 21):
        inserir(a, "clientes", k, f"Cliente {k}")
    subir(base, "h-replica", {**config_base(5331, h), "somente_leitura": True,
                              "replicacao": {"papel": "replica",
                                             "id_servidor": "belgica-01",
                                             "origens": [origem_para(5330, "curitiba", h,
                                                                     {"reconectar_em": 2})]}})
    b = liga(5331)
    t = esperar(lambda: eventos_de(b, "clientes") == 20, 30)
    iguais = linhas_por_id(a, "clientes") == linhas_por_id(b, "clientes")
    gravar = b({"op": "inserir", "database": "loja", "tabela": "clientes",
                "linha": {"id": 99, "nome": "x", "cidade": "x"}})
    velho = (not gravar.get("ok") and gravar.get("nome") == "ACESSO_NEGADO"
             and "somente leitura" in gravar.get("erro", ""))
    ok = t is not None and iguais and velho
    medir("h", ok, f"alcancou em {t and round(t,1)}s; iguais={iguais}; recusa "
                   f"antiga={gravar.get('nome')} {gravar.get('erro','')!r}")
    derrubar("h-source", "h-replica")


def main():
    base = sys.argv[1] if len(sys.argv) > 1 else "/tmp/phx-modos"
    if not os.path.exists(PHXSQLD):
        sys.exit(f"nao achei {PHXSQLD} -- rode `cargo build --release` antes")
    h = hash_da_senha(SENHA)
    try:
        estagio_a(base, h)
        estagio_b(base, h)
        estagio_cde(base, h)
        estagio_f(base, h)
        estagio_g(base, h)
        estagio_h(base, h)
    finally:
        derrubar()
    print()
    print("RESULTADO " + json.dumps(RESULTADO, ensure_ascii=False))
    if FALHAS:
        sys.exit(f"estagios com falha: {FALHAS}")


if __name__ == "__main__":
    main()
