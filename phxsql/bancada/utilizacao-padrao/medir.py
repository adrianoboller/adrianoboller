#!/usr/bin/env python3
"""Utilizacao padrao: criar base, criar tabela complexa, 20.000 linhas, ler de
volta e CONFERIR -- com e sem binarios e memos.

    flock /tmp/phx-cargo.lock cargo build --release --bin phxsqld
    python3 bancada/utilizacao-padrao/medir.py [n_linhas]

# O que esta bancada mede, e o que ela NAO mede

Ela percorre o caminho de quem USA o banco, pelo protocolo, contra o `phxsqld`
de pe. Nao ha uma linha de Rust chamando o motor por dentro: tudo passa pelo
soquete, como passa o driver ODBC, a tela e qualquer cliente.

O eixo e «com e sem binarios e memos», e ele tem uma armadilha que a
`bancada/LEIA-ME.md` ja descreve com dois numeros: **bancada compara trabalho
igual, nao so pergunta igual**. Uma tabela com `Bin` e `Memo` grava em dois
arquivos que a outra nem abre. Publicar a razao entre as duas como «o custo do
motor» seria o terceiro erro da serie -- depois do `WHERE id IN (…)` contra
vinte mil buscas (41x a favor do outro motor) e do `COUNT(*)+SUM` sobre
1.250.000 linhas contra a leitura de 20.000 (5x a favor do nosso).

Entao a diferenca e decomposta em TRES lados, e nao dois:

  sem     11 colunas de dado, cinco indices. Nenhum arquivo externo.
  com     as mesmas 11 mais `observacao` (Memo) e `foto` (Bin).
  largo   as mesmas 11 mais `observacao` e `foto` com os MESMOS NOMES e os
          MESMOS VALORES, declaradas `Str(n)`. O pedido no fio e byte a byte
          identico ao do lado `com`; o que muda e onde o dado para -- no slot
          de largura fixa do `.reg`, e nao no `.bin`/`.memo`.

Com isso cada diferenca responde uma pergunta so:

  sem  -> largo   o que o PESO NO FIO custa (o mesmo JSON, sem arquivo externo)
  largo -> com    o que o `.bin`/`.memo` custa (o mesmo fio, outro destino)

Sem o lado `largo`, «com blob e N vezes mais lento» juntaria as duas coisas num
numero so, e ninguem saberia qual metade e qual.

O que esta bancada NAO mede: transacao (nao ha `BEGIN` aqui -- e o caminho de
carga comum), concorrencia (uma conexao), e durabilidade de queda de energia.

# Carga que nao confere o que gravou mede o soquete

Toda linha volta e e comparada campo a campo, e os dois blobs sao comparados
byte a byte. E o comparador tem CONTROLE POSITIVO na mesma corrida: no fim,
uma copia do esperado e estragada de proposito e o mesmo comparador tem de
ACUSAR. Esta casa ja publicou zero com um medidor cego.

A ultima linha e `RESULTADO <json>`, e `resultado.json` fica ao lado.
"""
import datetime
import json
import os
import subprocess
import sys
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, AQUI)
import oficina  # noqa: E402

RAIZ = oficina.RAIZ
PORTA = int(os.environ.get("PHX_PORTA_UTIL", "6321"))
BASE = os.environ.get("PHX_UTIL_BASE", "/tmp/phx-utilizacao-%d" % os.getpid())
DB = "comercial"
POR_LOTE = 5_000
POR_PAGINA = 2_000
RESULTADO = os.environ.get("PHX_SAIDA",
                          os.path.join(AQUI, "resultado.json"))

LADOS = ["sem", "com", "largo"]

# ------------------------------------------------------------------ o esquema

# As onze colunas de dado que os tres lados tem em comum. Cada tipo esta aqui
# porque exercita um caminho diferente do motor, e nao para engrossar a lista:
#
#   filial   Int2       metade da chave COMPOSTA
#   id       Int8       a outra metade
#   codigo   Str(24)    indice UNICO proprio -- o motor le antes de gravar
#   nome     Str(60)    indice SEM CAIXA (`nocase`), que compara dobrado
#   cidade   Str(30)    indice de baixa cardinalidade (oito valores)
#   nascimento Date     inteiro de dias, com texto ISO no fio
#   criado_em DateTime  inteiro de ms, com texto no fio -- formatos diferentes
#   saldo    Decimal    i128 escalado; recusa numero em JSON, exige texto
#   ativo    Bool       um byte
#   categoria_id Int8   a chave ESTRANGEIRA, que nasce conferida
#   memo/bin            so nos lados `com` e `largo`
COLUNAS_BASE = [
    {"nome": "filial", "tipo": "Int2", "obrigatoria": True},
    {"nome": "id", "tipo": "Int8", "obrigatoria": True},
    {"nome": "codigo", "tipo": "Str(24)", "obrigatoria": True},
    {"nome": "nome", "tipo": "Str(60)"},
    {"nome": "cidade", "tipo": "Str(30)"},
    {"nome": "nascimento", "tipo": "Date"},
    {"nome": "criado_em", "tipo": "DateTime"},
    {"nome": "saldo", "tipo": "Decimal(15,2)"},
    {"nome": "ativo", "tipo": "Bool"},
    {"nome": "categoria_id", "tipo": "Int8"},
]

# O `nocase` e o `unico` estao aqui de proposito: sao os dois indices que
# custam mais que um indice comum, e o que se mede e uma tabela real e nao a
# mais barata que se conseguiria montar.
INDICES = [
    {"nome": "porFilialId", "colunas": ["filial", "id"], "unico": True,
     "primario": True},
    {"nome": "porCodigo", "colunas": ["codigo"], "unico": True},
    {"nome": "porNome", "colunas": ["nome nocase"]},
    {"nome": "porCidade", "colunas": ["cidade"]},
    # A chave conferida precisa de indice DOS DOIS LADOS: na mae para
    # responder «existe este pai?» ao gravar a filha, e na filha para
    # responder «alguem aponta para esta linha?» ao apagar a mae.
    {"nome": "porCategoria", "colunas": ["categoria_id"]},
]

FK = [{"nome": "fk_categoria", "colunas": ["categoria_id"],
       "tabela_ref": "categorias", "colunas_ref": ["id"],
       "ao_excluir": "restringir", "ao_alterar": "cascata"}]

BIN_BYTES = 256
MEMO_CHARS = 600


def colunas_do_lado(lado):
    cols = [dict(c) for c in COLUNAS_BASE]
    if lado == "com":
        cols += [{"nome": "observacao", "tipo": "Memo"},
                 {"nome": "foto", "tipo": "Bin"}]
    elif lado == "largo":
        # MESMOS NOMES e MESMOS VALORES do lado `com`: e o que faz o pedido no
        # fio sair byte a byte igual. So o tipo declarado muda.
        cols += [{"nome": "observacao", "tipo": "Str(%d)" % (MEMO_CHARS + 40)},
                 {"nome": "foto", "tipo": "Str(%d)" % (BIN_BYTES * 2 + 8)}]
    return cols


# ------------------------------------------------------------------- os dados

CIDADES = ["Blumenau", "Joinville", "Itajai", "Curitiba",
           "Chapeco", "Lages", "Florianopolis", "Criciuma"]
SOBRENOMES = ["Silva", "Souza", "Boller", "Andrade", "Zimmermann",
              "Oliveira", "Ávila", "Éder", "Nunes", "Xavier"]
CATEGORIAS = 50
EPOCA = datetime.datetime(1970, 1, 1)


def hex_da_foto(i):
    """Bytes deterministicos e DIFERENTES a cada linha.

    Diferentes importa: vinte mil blobs iguais deixariam passar um defeito que
    grava sempre o mesmo bloco, e a conferencia byte a byte nao acusaria nada.
    """
    return bytes(((i * 31 + j * 17) % 251) for j in range(BIN_BYTES)).hex()


def texto_do_memo(i):
    corpo = "linha %d: %s " % (i, SOBRENOMES[i % len(SOBRENOMES)])
    return (corpo * 40)[:MEMO_CHARS]


def linha(i, lado):
    """A linha `i`, identica nos tres lados a menos das duas colunas do fim.

    Previsivel, sem sorteio: os tres lados recebem exatamente as mesmas linhas,
    que e a primeira das quatro regras da `bancada/LEIA-ME.md`."""
    l = {
        "filial": (i % 7) + 1,
        "id": i,
        "codigo": "PX%08d" % i,
        "nome": "%s %05d" % (SOBRENOMES[i % len(SOBRENOMES)], i),
        "cidade": CIDADES[i % len(CIDADES)],
        "nascimento": (datetime.date(1950, 1, 1)
                       + datetime.timedelta(days=i % 20_000)).isoformat(),
        "criado_em": 1_700_000_000_000 + i * 1_000,
        "saldo": "%d.%02d" % (i * 7 // 100, i % 100),
        "ativo": (i % 3) != 0,
        "categoria_id": (i % CATEGORIAS) + 1,
    }
    if lado in ("com", "largo"):
        l["observacao"] = texto_do_memo(i)
        l["foto"] = hex_da_foto(i)
    return l


def esperado(i, lado):
    """O que a linha `i` tem de VOLTAR pelo fio.

    Tres campos voltam num formato diferente do que foram mandados, e os tres
    sao conferidos contra uma conta feita AQUI, em Python -- dois codigos sem
    uma linha em comum, que e como a bancada ja provou a soma da varredura."""
    e = dict(linha(i, lado))
    # AS COLUNAS DE SISTEMA ENTRAM NA CONFERENCIA, e nao so na contagem de
    # colunas do esquema. Foi coluna de sistema nova que quebrou todo salvar e
    # todo incluir pela tela uma vez, e conferir o numero de colunas nao teria
    # pego aquilo -- o que pega e comparar o VALOR. Aqui a tabela nao e
    # particionada, entao a linha `i` foi a i-esima digitada e o `rownum` dela
    # e `i`; e nenhuma linha desta carga foi excluida, entao `softdeleted` e
    # falso em todas.
    e["rownum"] = i
    e["softdeleted"] = False
    ms = e["criado_em"]
    q = EPOCA + datetime.timedelta(milliseconds=ms)
    e["criado_em"] = "%s %02d:%02d:%02d,%03d" % (
        q.date().isoformat(), q.hour, q.minute, q.second, ms % 1000)
    return e


# ---------------------------------------------------------------- as fases

def com_portao(d, nome, fn):
    """Roda uma fase de TEMPO com o portao consultado antes e depois DELA.

    Por fase, e nao pela corrida inteira: a corrida leva um minuto e meio, e
    numa maquina com vizinho ativo a chance de o portao acusar em algum momento
    dela e alta -- o que jogaria fora tambem o tempo das fases que rodaram
    numa janela limpa. Cada fase carrega o proprio veredito, e o gerador
    publica so as que couberam inteiras no silencio."""
    antes, quem_a = oficina.portao_de_medicao()
    saida = fn()
    depois, quem_d = oficina.portao_de_medicao()
    d.setdefault("portao", {})[nome] = {
        "antes": antes, "depois": depois, "publicavel": not (antes or depois),
        "quem": quem_a or quem_d,
    }
    return saida

def criar_tudo(c, n):
    """Cria a base, a mae da chave estrangeira e os tres lados.

    Devolve o que o SERVIDOR respondeu para cada `criar_tabela` -- e nao o que
    este script pediu. A diferenca e o ponto: as colunas de sistema
    (`softdeleted`, `rownum`) entram sozinhas, e quem conta as declaradas
    publica um numero que a tabela nao tem."""
    c.ok({"op": "criar_database", "database": DB})
    # A mae da chave estrangeira. O indice unico dela nao e enfeite: sem ele o
    # motor RECUSA a chave conferida, dizendo qual lado falta.
    c.ok({"op": "criar_tabela", "database": DB, "tabela": "categorias",
          "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                      {"nome": "nome", "tipo": "Str(30)", "obrigatoria": True}],
          "indices": [{"nome": "porId", "colunas": ["id"], "unico": True,
                       "primario": True}]})
    for k in range(1, CATEGORIAS + 1):
        c.ok({"op": "inserir", "database": DB, "tabela": "categorias",
              "linha": {"id": k, "nome": "categoria %02d" % k}})
    criadas = {}
    for lado in LADOS:
        criadas[lado] = c.ok({"op": "criar_tabela", "database": DB,
                              "tabela": lado, "colunas": colunas_do_lado(lado),
                              "indices": INDICES, "chaves_estrangeiras": FK})
        # A SEGUNDA CORRIDA da carga, em tabela propria. O nome comeca por
        # `r2` e nao termina por `_r2` de proposito: o somador de disco conta
        # `nome` e `nome_*` como a mesma tabela (e assim que ele soma os
        # volumes de uma tabela paginada), entao `sem_r2` entraria na conta de
        # `sem` e dobraria o numero em silencio.
        #
        # Uma corrida so nao
        # separa «igual» de «parecido»: a diferenca entre os lados `com` e
        # `largo` e de por cento, e sem duas corridas nao da para dizer se ela
        # existe. Tabela nova, e nao a mesma esvaziada, porque o slot excluido
        # nunca se reaproveita -- gravar por cima seria outro trabalho.
        c.ok({"op": "criar_tabela", "database": DB, "tabela": "r2" + lado,
              "colunas": colunas_do_lado(lado), "indices": INDICES,
              "chaves_estrangeiras": FK})
    return criadas


def carregar(c, lado, n):
    """As `n` linhas em lotes de 5.000.

    Devolve o tempo de PAREDE, os bytes nos dois sentidos e o `ms` que o
    servidor carimbou nas respostas. Os dois tempos nao medem a mesma coisa, e
    e por isso que os dois voltam: o de parede inclui montar o JSON no cliente,
    o fio e a analise da volta; o do servidor e so o que aconteceu dentro do
    motor com a trava de dados na mao."""
    ls = [linha(i, lado) for i in range(1, n + 1)]
    c.zerar()
    t = time.perf_counter()
    for i in range(0, n, POR_LOTE):
        c.ok({"op": "inserir_lote", "database": DB, "tabela": lado,
              "linhas": ls[i:i + POR_LOTE]})
    s = time.perf_counter() - t
    return s, c.enviados, c.recebidos, c.ms


def divergencias(volta, esp, lado):
    """Compara UMA linha voltada com o que se esperava. Devolve a lista de
    campos que nao batem -- vazia quer dizer igual."""
    ruins = []
    for campo, valor in esp.items():
        veio = volta.get(campo, "<ausente>")
        if veio != valor:
            ruins.append(campo)
    return ruins


def ler_de_volta(c, lado, n):
    """Le as `n` linhas pelo cursor e confere TODAS, campo a campo.

    Pelo cursor (`depois`) e nao por `pular`: e o caminho que custa o tamanho
    da pagina em vez do tamanho da tabela, e e o que a tela usa."""
    c.zerar()
    t = time.perf_counter()
    vistas = {}
    cursor = 0
    paginas = 0
    while True:
        r = c.ok({"op": "varrer", "database": DB, "tabela": lado,
                  "depois": cursor, "max": POR_PAGINA})
        if not r["linhas"]:
            break
        paginas += 1
        for l in r["linhas"]:
            vistas[l["id"]] = l
        cursor = r["cursor_fim"]
        if not r["ha_mais"]:
            break
    s = time.perf_counter() - t

    ruins = []
    for i in range(1, n + 1):
        volta = vistas.get(i)
        if volta is None:
            ruins.append((i, ["<linha ausente>"]))
            continue
        d = divergencias(volta, esperado(i, lado), lado)
        if d:
            ruins.append((i, d))
    return {
        "s": s, "paginas": paginas, "linhas_lidas": len(vistas),
        "enviados": c.enviados, "recebidos": c.recebidos,
        "divergentes": len(ruins),
        "amostra_divergente": ruins[:5],
    }


def controle_do_comparador(c, lado):
    """O CONTROLE POSITIVO: o mesmo comparador, na mesma corrida, tem de
    ACUSAR o caso oposto.

    Sem isto, «zero divergencias» pode ser um comparador cego -- e esta casa ja
    publicou zero com um medidor cego. Estraga-se uma copia do ESPERADO (nunca
    o que esta gravado) e pergunta-se ao mesmo `divergencias`."""
    r = c.ok({"op": "varrer", "database": DB, "tabela": lado, "max": 1})
    volta = r["linhas"][0]
    i = volta["id"]
    limpo = esperado(i, lado)
    acusou = {}

    # 1) um escalar trocado
    sujo = dict(limpo)
    sujo["cidade"] = limpo["cidade"] + "x"
    acusou["escalar"] = divergencias(volta, sujo, lado)

    # 2) a COLUNA DE SISTEMA: o `rownum` fora da ordem de digitacao
    sujo = dict(limpo)
    sujo["rownum"] = limpo["rownum"] + 1
    acusou["rownum_da_coluna_de_sistema"] = divergencias(volta, sujo, lado)

    # 3) um decimal com um centavo a menos -- o erro que um `f64` produziria
    sujo = dict(limpo)
    inteiro, centavos = limpo["saldo"].split(".")
    sujo["saldo"] = "%s.%02d" % (inteiro, (int(centavos) + 1) % 100)
    acusou["decimal"] = divergencias(volta, sujo, lado)

    # 4) UM BYTE do blob, que e a conferencia que a carga existe para fazer
    if lado in ("com", "largo"):
        sujo = dict(limpo)
        h = limpo["foto"]
        trocado = "0" if h[0] != "0" else "1"
        sujo["foto"] = trocado + h[1:]
        acusou["um_byte_do_blob"] = divergencias(volta, sujo, lado)
        sujo = dict(limpo)
        sujo["observacao"] = limpo["observacao"][:-1] + "!"
        acusou["ultimo_char_do_memo"] = divergencias(volta, sujo, lado)

    # E o negativo: sem estrago, o mesmo comparador cala.
    acusou["sem_estrago"] = divergencias(volta, limpo, lado)
    return {"rowid_usado": volta["rowid"], "acusou": acusou}


def integridade(c):
    """A regra primordial, pela porta de dados: nunca se mata o pai que tem
    filhos -- nem de vez, nem de forma suave."""
    r = c.ok({"op": "buscar", "database": DB, "tabela": "categorias",
              "indice": "porId", "chave": [1]})
    rowid_com_filhas = r["linhas"][0]["rowid"]
    saida = {
        "suave": c.erro({"op": "excluir", "database": DB, "tabela": "categorias",
                         "rowid": rowid_com_filhas}),
        "de_vez": c.erro({"op": "excluir", "database": DB, "tabela": "categorias",
                          "rowid": rowid_com_filhas, "fisico": True}),
        "orfa": c.erro({"op": "inserir", "database": DB, "tabela": "sem",
                        "linha": dict(linha(999_001, "sem"),
                                      categoria_id=CATEGORIAS + 900)}),
        "codigo_repetido": c.erro({"op": "inserir", "database": DB,
                                   "tabela": "sem",
                                   "linha": dict(linha(999_002, "sem"),
                                                 codigo="PX00000001")}),
    }
    # O CONTROLE: a mesma operacao passa quando nao ha filha. Sem isto,
    # «recusou» poderia ser «recusa sempre».
    c.ok({"op": "inserir", "database": DB, "tabela": "categorias",
          "linha": {"id": 9_999, "nome": "sem filhas"}})
    r = c.ok({"op": "buscar", "database": DB, "tabela": "categorias",
              "indice": "porId", "chave": [9_999]})
    saida["controle_sem_filhas"] = c.ok(
        {"op": "excluir", "database": DB, "tabela": "categorias",
         "rowid": r["linhas"][0]["rowid"], "fisico": True})["excluido"]
    return saida


def coluna_com_padrao(c, lado, n):
    """A UNICA forma de «coluna com padrao» que o motor tem, exercitada.

    O esquema NAO guarda valor padrao por coluna -- `Column` tem id, nome,
    caption, descricao, mascara, tipo, nullable e dado_pessoal, e mais nada.
    O `padrao` existe so no `acrescentar_coluna`, e ali ele e um valor de
    PREENCHIMENTO das linhas que ja existem, nao uma regra que valha para as
    proximas. Esta funcao mede esse caminho e confere as duas pontas."""
    r = c.ok({"op": "acrescentar_coluna", "database": DB, "tabela": lado,
              "coluna": {"nome": "situacao", "tipo": "Str(12)"},
              "padrao": "ativo"})
    conferidas = []
    for alvo in (1, n):
        v = c.ok({"op": "buscar", "database": DB, "tabela": lado,
                  "indice": "porCodigo", "chave": ["PX%08d" % alvo]})
        conferidas.append(v["linhas"][0].get("situacao"))
    # E a ponta que o nome «padrao» faria supor e que NAO existe: a linha nova
    # nao ganha o valor, porque o esquema nao guarda padrao nenhum.
    c.ok({"op": "inserir", "database": DB, "tabela": lado,
          "linha": linha(999_100 + LADOS.index(lado), lado)})
    nova = c.ok({"op": "buscar", "database": DB, "tabela": lado,
                 "indice": "porCodigo",
                 "chave": ["PX%08d" % (999_100 + LADOS.index(lado))]})
    return {
        "slots_reescritos": r["slots_reescritos"],
        "ms": round(r["ms"], 3),
        "indices_refeitos": r["indices_refeitos"],
        "nas_linhas_que_ja_existiam": conferidas,
        "na_linha_inserida_depois": nova["linhas"][0].get("situacao"),
    }


# --------------------------------- a chave conferida, e o controle da posicao

def custo_da_chave_conferida(n):
    """Quanto a chave estrangeira CONFERIDA custa na gravacao -- e, de quebra,
    se a primeira carga de uma serie e mais lenta que as seguintes.

    # As duas perguntas, e por que elas cabem na mesma corrida

    A tabela complexa desta bancada tem uma chave estrangeira que **nasce
    conferida**, e conferir e uma leitura a mais no laco quente: para cada
    linha gravada, o motor pergunta a mae se o pai existe. `docs/DESEMPENHO.md`
    SS15 ja mediu isso por dentro (`--example custo-da-fk`); aqui a mesma
    pergunta e feita pela PORTA DE DADOS, com a tabela inteira -- cinco
    indices, blobs, o fio.

    A segunda pergunta e o CONTROLE do laco principal: la, a primeira carga de
    um lado com blob sai mais lenta que a segunda do mesmo lado. Aqui as tres
    cargas tem o MESMO esquema e as mesmas linhas, uma atras da outra -- se a
    lentidao fosse de «ser a primeira», ela apareceria aqui tambem.

    Os dois lados diferem em UMA coisa: a declaracao da chave. O indice
    `porCategoria` existe nos dois, entao o que se mede e a conferencia, e nao
    o indice.
    """
    saida = {}
    for com_fk in (True, False):
        base = "%s-fk%d" % (BASE, int(com_fk))
        porta = PORTA + 2 + int(com_fk)
        subprocess.run(["rm", "-rf", base], check=False)
        p = oficina.subir(base, porta)
        try:
            c = oficina.Conexao(porta)
            c.ok({"op": "criar_database", "database": DB})
            if com_fk:
                c.ok({"op": "criar_tabela", "database": DB, "tabela": "categorias",
                      "colunas": [{"nome": "id", "tipo": "Int8",
                                   "obrigatoria": True},
                                  {"nome": "nome", "tipo": "Str(30)",
                                   "obrigatoria": True}],
                      "indices": [{"nome": "porId", "colunas": ["id"],
                                   "unico": True, "primario": True}]})
                for k in range(1, CATEGORIAS + 1):
                    c.ok({"op": "inserir", "database": DB, "tabela": "categorias",
                          "linha": {"id": k, "nome": "categoria %02d" % k}})
            extra = {"chaves_estrangeiras": FK} if com_fk else {}
            corridas = []
            for nome in ("c1", "c2", "c3"):
                c.ok(dict({"op": "criar_tabela", "database": DB, "tabela": nome,
                           "colunas": colunas_do_lado("com"),
                           "indices": INDICES}, **extra))
            for nome in ("c1", "c2", "c3"):
                seg, _, _, ms = carregar(c, nome, n)
                corridas.append({"parede_s": round(seg, 3),
                                 "motor_s": round(ms / 1000, 3)})
            saida["com_fk" if com_fk else "sem_fk"] = corridas
            c.fechar()
        finally:
            oficina.baixar(p)
            subprocess.run(["rm", "-rf", base], check=False)
    return saida


# ------------------------------------------------------------------- o fsync

def fsync_por_acao(n_pequeno=500):
    """Quantos `fsync`, e em qual arquivo, cada acao provoca em cada lado.

    # Por que `por_operacao` e nao a configuracao de fabrica

    Porque a de fabrica (`por_lote`, 200 operacoes ou 200 ms) fecha a janela
    pelo RELOGIO, e ai a contagem passa a depender de quantas vezes o relogio
    bateu no meio da carga -- e nao da acao. Medido assim numa primeira
    corrida, o mesmo lote deu 2 `fsync` no `.reg` para um lado e 1 para o
    outro, e a diferenca era o relogio. Em `por_operacao` a janela fecha ao fim
    de CADA operacao, entao o numero e um atributo da acao, e se refaz igual.

    Contagem de chamada de sistema e deterministica: ela vale com a maquina
    ocupada, e por isso e a parte desta bancada que sempre se publica.

    Corrida separada e pequena de proposito: `strace` anexado durante a carga
    inteira mudaria o tempo que a outra metade desta bancada mede.
    """
    import importlib.util
    spec = importlib.util.spec_from_file_location(
        "fecho", os.path.join(RAIZ, "bancada", "durabilidade", "prova-do-fecho.py"))
    fecho = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(fecho)

    base = BASE + "-fsync"
    subprocess.run(["rm", "-rf", base], check=False)
    cfg = oficina.config(PORTA + 1)
    cfg["recursos"] = {"durabilidade": "por_operacao"}
    p = oficina.subir(base, PORTA + 1, cfg)
    por_lado = {}
    try:
        c = oficina.Conexao(PORTA + 1)
        criar_tudo(c, n_pequeno)

        def contar(rotulo, lado, acao):
            saida = os.path.join(base, "strace-%s-%s.txt" % (lado, rotulo))
            st = fecho.anexar(p.pid, saida)
            acao()
            time.sleep(0.4)
            fecho.soltar(st)
            conta = {}
            for _ts, caminho, ok in fecho.eventos_fsync(saida):
                if not ok:
                    continue
                nome = os.path.basename(caminho)
                ext = nome.rsplit(".", 1)[-1] if "." in nome else "(sem)"
                conta[ext] = conta.get(ext, 0) + 1
            return conta

        for lado in LADOS:
            ls = [linha(i, lado) for i in range(1, n_pequeno + 1)]
            por_lado[lado] = {
                "um_lote_de_%d" % n_pequeno: contar(
                    "lote", lado,
                    lambda: c.ok({"op": "inserir_lote", "database": DB,
                                  "tabela": lado, "linhas": ls})),
                "uma_linha": contar(
                    "uma", lado,
                    lambda: c.ok({"op": "inserir", "database": DB,
                                  "tabela": lado,
                                  "linha": linha(n_pequeno + 1, lado)})),
            }
        c.fechar()
    finally:
        oficina.baixar(p)
        subprocess.run(["rm", "-rf", base], check=False)
    return {"linhas_do_lote": n_pequeno, "regime": "por_operacao",
            "por_lado": por_lado}


# --------------------------------------------------------------------- corrida

def main():
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 20_000
    if not os.path.exists(oficina.PHXSQLD):
        sys.exit("nao achei %s -- rode `cargo build --release --bin phxsqld`"
                 % oficina.PHXSQLD)

    ocupada_antes, quem_antes = oficina.portao_de_medicao()
    subprocess.run(["rm", "-rf", BASE], check=False)
    p = oficina.subir(BASE, PORTA)
    d = {"linhas": n, "por_lote": POR_LOTE, "por_pagina": POR_PAGINA,
         "bin_bytes": BIN_BYTES, "memo_chars": MEMO_CHARS,
         "indices": [i["nome"] for i in INDICES],
         "lados": {}, "esta_medindo_antes": ocupada_antes,
         "esta_medindo_quem_antes": quem_antes}
    try:
        c = oficina.Conexao(PORTA)
        print("=== utilizacao padrao: %d linhas, tabela complexa ===\n" % n)
        criadas = criar_tudo(c, n)
        d["colunas_declaradas"] = {l: len(colunas_do_lado(l)) for l in LADOS}
        d["colunas_no_esquema"] = {l: criadas[l]["colunas"] for l in LADOS}
        d["indices_no_esquema"] = {l: criadas[l]["indices"] for l in LADOS}
        d["cidades"] = len(CIDADES)
        d["categorias"] = CATEGORIAS

        # AS SEIS CARGAS PRIMEIRO, SEM NADA NO MEIO.
        #
        # A primeira versao deste laco fazia carga, leitura, conferencia e
        # `acrescentar_coluna` de um lado antes de comecar o proximo -- e ai a
        # carga de cada lado media tambem o rastro do lado anterior. O sintoma
        # foi grosseiro: a primeira carga de um lado saia ate 1,9x mais lenta
        # que a segunda do MESMO lado, com o mesmo esquema e as mesmas linhas,
        # e a diferenca aparecia inclusive no `ms` do servidor. Com as seis
        # seguidas, cada numero mede a carga e mais nada, e a segunda corrida
        # de cada lado passa a ser o que ela devia ser desde o comeco: a
        # medida do RUIDO, e nao um segundo fenomeno.
        # `PHX_ORDEM_INVERTIDA=1` inverte a ORDEM das cargas sem mudar mais
        # nada. E o controle da posicao: se um lado sai lento por ser o
        # segundo a carregar, e nao por ser o que ele e, inverter troca quem
        # sai lento. Sem esse interruptor, «o lado com blob custa 1,5x» e uma
        # afirmacao que ninguem consegue separar de «a segunda carga custa
        # 1,5x».
        ordem = list(reversed(LADOS)) if os.environ.get(
            "PHX_ORDEM_INVERTIDA") == "1" else list(LADOS)
        d["ordem_de_carga"] = ordem + ["r2" + l for l in ordem]
        def as_seis_cargas():
            feito = {}
            for lado in ordem:
                feito[lado] = carregar(c, lado, n)
            for lado in ordem:
                feito["r2" + lado] = carregar(c, "r2" + lado, n)
            return feito

        carga = com_portao(d, "cargas", as_seis_cargas)

        for lado in LADOS:
            s, env, rec, ms = carga[lado]
            s2, _, _, ms2 = carga["r2" + lado]
            leitura = ler_de_volta(c, lado, n)
            rel = c.ok({"op": "verificar", "database": DB, "tabela": lado})
            disco = oficina.bytes_no_disco(BASE, DB, lado)
            ctrl = controle_do_comparador(c, lado)
            padrao = coluna_com_padrao(c, lado, n)
            d["lados"][lado] = {
                "carga_s": round(s, 3),
                "carga_s_2": round(s2, 3),
                "carga_ms_servidor": round(ms, 1),
                "carga_ms_servidor_2": round(ms2, 1),
                "linhas_por_s": round(n / s),
                "linhas_por_s_2": round(n / s2),
                "linhas_por_s_servidor": round(n / (ms / 1000.0)),
                "linhas_por_s_servidor_2": round(n / (ms2 / 1000.0)),
                "fio_enviado": env, "fio_recebido": rec,
                "fio_por_linha": round((env + rec) / n, 1),
                "leitura": leitura,
                "registros_verificados": rel.get("registros"),
                "disco": disco,
                "disco_total": sum(disco.values()),
                "disco_por_linha": round(sum(disco.values()) / n, 1),
                "controle_do_comparador": ctrl,
                "coluna_com_padrao": padrao,
            }
            print("  %-6s parede %5.2f/%5.2f s   motor %5.2f/%5.2f s   "
                  "fio %7.1f B/l  disco %7.1f B/l  div %d"
                  % (lado, s, s2, ms / 1000.0, ms2 / 1000.0, (env + rec) / n,
                     sum(disco.values()) / n, leitura["divergentes"]))

        d["integridade"] = integridade(c)
        c.fechar()
    finally:
        oficina.baixar(p)

    d["chave_conferida"] = com_portao(
        d, "chave_conferida", lambda: custo_da_chave_conferida(n))
    # O `strace` pode nao conseguir anexar (ptrace ocupado, processo que morreu
    # antes). Isso NAO derruba a corrida inteira -- mas tambem nao vira zero:
    # o resultado guarda o erro, e o gerador escreve «nao medido» em vez de uma
    # tabela de zeros, que e a diferenca entre um medidor honesto e um cego.
    try:
        d["fsync"] = fsync_por_acao()
    except SystemExit as e:
        d["fsync"] = {"erro": str(e)}
        print("  fsync NAO medido: %s" % e)
    ocupada_depois, quem_depois = oficina.portao_de_medicao()
    d["esta_medindo_depois"] = ocupada_depois
    d["esta_medindo_quem_depois"] = quem_depois
    # O veredito da corrida inteira continua existindo -- ele e o que o
    # `resultado.json` guarda para quem quiser saber se a maquina estava livre
    # do comeco ao fim. Mas quem decide o que se PUBLICA e o portao por fase.
    d["tempo_publicavel"] = d["portao"]["cargas"]["publicavel"]
    d["corrida_inteira_limpa"] = not (ocupada_antes or ocupada_depois)
    d["versao"] = subprocess.run([oficina.PHXSQLD, "--version"],
                                 capture_output=True, text=True).stdout.strip()
    subprocess.run(["rm", "-rf", BASE], check=False)

    with open(RESULTADO, "w") as f:
        json.dump(d, f, indent=2, ensure_ascii=False)
    print("\nRESULTADO " + json.dumps(
        {k: v for k, v in d.items() if k not in ("lados",)}, ensure_ascii=False))
    print("gravei %s" % RESULTADO)
    return 0


if __name__ == "__main__":
    sys.exit(main())
