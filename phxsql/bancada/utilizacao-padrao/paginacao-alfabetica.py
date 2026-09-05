#!/usr/bin/env python3
"""A particao alfabetica pela PORTA DE DADOS -- o que os testes do motor nao
alcancam.

    flock /tmp/phx-cargo.lock cargo build --release --bin phxsqld
    python3 bancada/utilizacao-padrao/paginacao-alfabetica.py

# Por que esta prova existe

`crates/phxsql-store/tests/alfanumerica.rs` prova a particao por dentro,
chamando `Table` direto. O que ele nao alcanca e o caminho de FORA: criar a
tabela pelo protocolo, gravar pelo protocolo, e paginar pelo `varrer` -- que e
o que a tela, o driver ODBC e qualquer cliente fazem.

A diferenca nao e formal. Achado nesta bancada, e nao lendo o codigo: o
`varrer` monta o campo `ha_antes` com `pagina_antes_de`, que andava de um em um
para tras com o `ler` cru -- e na alfanumerica o slot entre o fim de um balde e
o comeco do proximo NAO EXISTE, entao o `ler` responde `NaoEncontrado` em vez
de `None`. Toda pagina que comecasse no primeiro slot de um balde voltava
`[SP000018] rowid N nao existe` em vez de linhas. Os 16 testes do motor
provavam a IDA (`pagina_depois_de`, pelo `proximo_ativo`); nenhum provava a
volta.

# O metodo

Cada afirmacao tem um CONTROLE na mesma corrida: uma recusa so vale quando a
mesma operacao PASSA no caso irmao, e uma ausencia so vale quando o mesmo
instrumento ACHA o caso oposto. Nada aqui e tempo, com uma excecao rotulada
(o salto pelo vazio, onde o que se mede e «terminou», e a diferenca entre
terminar e nao terminar e de minutos).

A ultima linha e `RESULTADO <json>`, e `resultado-alfabetica.json` fica ao lado.
"""
import json
import os
import subprocess
import sys
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, AQUI)
import oficina  # noqa: E402

PORTA = int(os.environ.get("PHX_PORTA_ALFA", "6325"))
BASE = os.environ.get("PHX_ALFA_BASE", "/tmp/phx-alfabetica-%d" % os.getpid())
DB = "cadastro"
RESULTADO = os.path.join(AQUI, "resultado-alfabetica.json")

AFIRMACOES = []


def afirmar(chave, frase, esperado, medido, controle=None, nota=None):
    ok = medido == esperado
    AFIRMACOES.append({"chave": chave, "frase": frase, "esperado": esperado,
                       "medido": medido, "ok": ok, "controle": controle,
                       "nota": nota})
    print("  %-4s %-44s %s" % ("ok" if ok else "NAO", chave,
                               json.dumps(medido, ensure_ascii=False)[:120]))
    if controle:
        print("       controle: %s" % controle)
    return ok


def criar(c, tabela, por_arquivo):
    return c.ok({"op": "criar_tabela", "database": DB, "tabela": tabela,
                 "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                             {"nome": "nome", "tipo": "Str(40)",
                              "obrigatoria": True}],
                 "indices": [{"nome": "porId", "colunas": ["id"],
                              "unico": True, "primario": True}],
                 "registros_por_arquivo": por_arquivo,
                 "particao": "letra", "particao_coluna": "nome"})


def arquivos(tabela):
    """Os sufixos dos `.reg` que existem NO DISCO. A resposta do servidor nao
    serve aqui: o que se afirma e o nome do arquivo."""
    pasta = os.path.join(BASE, "base", DB)
    if not os.path.isdir(pasta):
        return []
    saida = []
    for nome in sorted(os.listdir(pasta)):
        if nome.startswith(tabela + "_") and nome.endswith(".reg"):
            saida.append(nome[len(tabela) + 1:-4])
    return saida


def varrer(c, tabela, **kw):
    return c.ok(dict({"op": "varrer", "database": DB, "tabela": tabela}, **kw))


# ------------------------------------------------------- 1. o arquivo da letra

def a_linha_cai_no_arquivo_da_letra(c):
    criar(c, "porletra", 1_000)
    # A ordem de digitacao e embaralhada de proposito: e ela que o `rownum`
    # tem de guardar, e o arquivo NAO guarda.
    entrada = ["Silva", "Adriano", "Zeus", "Bruno", "Álvaro", "Alvaro",
               "0800", "#etc", "", "   ", "日本", "Çelik", "Éder", "Mendes"]
    rowids = []
    for i, nome in enumerate(entrada):
        rowids.append(c.ok({"op": "inserir", "database": DB,
                            "tabela": "porletra",
                            "linha": {"id": i + 1, "nome": nome}})["rowid"])
    c.ok({"op": "verificar", "database": DB, "tabela": "porletra"})

    achados = arquivos("porletra")
    afirmar("o_nome_do_arquivo_e_a_letra",
            "cada balde ocupado vira `porletra_<letra>.reg` no disco",
            ["0", "A", "B", "C", "E", "M", "Outros", "S", "Z"], achados,
            controle="o mesmo listador nao acha `_Q` nem `_X`, que nao "
                     "receberam linha: %s" % [x for x in ("Q", "X")
                                              if x in achados])
    afirmar("balde_vazio_nao_ganha_arquivo",
            "letra que nunca recebeu linha nao cria arquivo vazio",
            [], [x for x in ("Q", "X", "Y") if x in achados],
            controle="na mesma listagem, `A` e `Z` aparecem: %s"
                     % [x for x in ("A", "Z") if x in achados])

    # O acento: «Álvaro» e «Alvaro» sao a mesma pessoa digitada por duas
    # pessoas. Isto e DECISAO, e a decisao esta provada aqui pelo endereco --
    # os dois rowids caem na faixa 1..1000, que e o balde A.
    alvaro_com, alvaro_sem = rowids[4], rowids[5]
    afirmar("acento_cai_na_letra_sem_acento",
            "«Álvaro» e «Alvaro» vao para o MESMO arquivo (`_A`), e «Çelik» "
            "para o `_C`, e «Éder» para o `_E`",
            [True, True, True, True],
            [1 <= alvaro_com <= 1_000, 1 <= alvaro_sem <= 1_000,
             2_001 <= rowids[11] <= 3_000, 4_001 <= rowids[12] <= 5_000],
            controle="o mesmo endereco poe «Bruno» em 1001..2000 (balde B): %s"
                     % (1_001 <= rowids[3] <= 2_000))
    afirmar("digito_tem_balde_proprio_nao_e_Outros",
            "«0800» NAO cai em Outros: os dez algarismos tem balde proprio, e "
            "o arquivo se chama `porletra_0.reg`",
            True, 26_001 <= rowids[6] <= 27_000,
            controle="na mesma corrida, `#etc` cai em Outros (36001..37000): %s"
                     % (36_001 <= rowids[7] <= 37_000))
    afirmar("outros_recebe_o_que_nao_e_letra_nem_algarismo",
            "`#etc`, vazio, so espacos e `日本` vao todos para `_Outros`",
            [True] * 4,
            [36_001 <= r <= 37_000 for r in rowids[7:11]],
            controle="e o mesmo criterio deixa `Mendes` fora de Outros: %s"
                     % (12_001 <= rowids[13] <= 13_000))
    return entrada, rowids


# --------------------------------------------- 2. a ordem de digitacao no rownum

def a_ordem_de_digitacao_nao_se_perdeu(c, entrada, rowids):
    linhas = varrer(c, "porletra", max=100)["linhas"]
    por_id = {l["id"]: l for l in linhas}
    rownums = [por_id[i + 1]["rownum"] for i in range(len(entrada))]
    afirmar("a_ordem_de_digitacao_esta_no_rownum",
            "o `rownum` de cada linha e a posicao em que ela foi DIGITADA, e "
            "nao a posicao dela no arquivo",
            list(range(1, len(entrada) + 1)), rownums,
            controle="na mesma corrida os rowids NAO sao crescentes na ordem "
                     "de digitacao (o primeiro digitado tem rowid %d e o "
                     "segundo tem %d)" % (rowids[0], rowids[1]))
    afirmar("a_varredura_sai_em_ordem_de_balde",
            "a leitura sai na ordem dos baldes (alfabetica), que e outra "
            "ordem que a de digitacao",
            sorted([l["rowid"] for l in linhas]),
            [l["rowid"] for l in linhas],
            controle="e o `rownum` da mesma lista sai fora de ordem: %s"
                     % [l["rownum"] for l in linhas][:6])


# ------------------------------------------------- 3. o salto pelos vazios

def a_varredura_salta_os_vazios(c):
    # Um milhao de slots por balde: entre o `_A` e o `_Z` ha 25 milhoes de
    # slots que nunca existiram. Andar por eles nao terminaria nesta vida.
    criar(c, "esparsa", 1_000_000)
    for i, nome in enumerate(["Adriano", "Mendes", "Zeus"]):
        c.ok({"op": "inserir", "database": DB, "tabela": "esparsa",
              "linha": {"id": i + 1, "nome": nome}})
    t = time.perf_counter()
    r = varrer(c, "esparsa", max=100)
    s = time.perf_counter() - t
    afirmar("a_varredura_salta_os_vazios_entre_baldes",
            "com 1.000.000 de slots por balde, a varredura devolve as tres "
            "linhas pelos enderecos calculados, sem andar pelos 25 milhoes de "
            "slots vazios",
            [1, 12_000_001, 25_000_001], [l["rowid"] for l in r["linhas"]],
            controle="terminou em %.3f s; andar de um em um seriam 25.000.000 "
                     "de leituras, que nao cabem em segundo nenhum "
                     "(este e o UNICO numero de tempo desta prova, e ele e um "
                     "limite, nao um desempenho)" % s)
    # A pagina ANTERIOR e o irmao que tinha ficado: era ela que reprovava a
    # varredura inteira, porque o `varrer` a chama para dizer `ha_antes`.
    r2 = varrer(c, "esparsa", antes=25_000_001, max=10)
    afirmar("a_pagina_anterior_atravessa_os_mesmos_vazios",
            "pedir a pagina ANTERIOR a Zeus atravessa os mesmos vazios e "
            "devolve Adriano e Mendes",
            [1, 12_000_001], [l["rowid"] for l in r2["linhas"]],
            controle="e perguntar o que vem antes da primeira linha devolve "
                     "lista vazia, e nao erro: %d linha(s)"
                     % len(varrer(c, "esparsa", antes=1, max=10)["linhas"]))
    return s


# --------------------------------------- 4. alterar a coluna de referencia

def alterar_a_coluna_de_referencia(c):
    linhas = varrer(c, "porletra", max=100)["linhas"]
    silva = [l for l in linhas if l["nome"] == "Silva"][0]
    erro = c.erro({"op": "atualizar", "database": DB, "tabela": "porletra",
                   "rowid": silva["rowid"],
                   "linha": {"id": silva["id"], "nome": "Andrade"}})
    afirmar("alterar_a_coluna_de_referencia_e_recusado",
            "trocar «Silva» por «Andrade» muda o arquivo em que a linha mora, "
            "e a gravacao recusa",
            True, "balde" in erro,
            controle="a recusa DIZ o que fazer: %r" % erro)
    afirmar("a_recusa_diz_o_caminho",
            "e a mensagem manda excluir e inserir, em vez de so dizer «nao»",
            True, "xclua e insira" in erro)
    # O CONTROLE: dentro do MESMO balde a alteracao passa. Sem isto, «recusou»
    # poderia ser «recusa toda alteracao».
    r = c.ok({"op": "atualizar", "database": DB, "tabela": "porletra",
              "rowid": silva["rowid"],
              "linha": {"id": silva["id"], "nome": "Silveira"}})
    # `com_versao` porque sem ele a resposta e a linha crua, e nao um objeto
    # com a linha dentro -- e quem le a resposta errada acha que o campo sumiu.
    depois = c.ok({"op": "ler", "database": DB, "tabela": "porletra",
                   "rowid": silva["rowid"], "com_versao": True})
    afirmar("alterar_dentro_do_mesmo_balde_passa",
            "«Silva» -> «Silveira» fica no `_S` e e aceita",
            ["Silveira", silva["rowid"]],
            [depois["linha"]["nome"], r.get("rowid", silva["rowid"])],
            controle="e o rowid nao mudou, que e o que a recusa acima protege")
    return erro


# ------------------------------------ 5. o `pular`/`max` do varrer com baldes

def a_paginacao_atravessa_balde(c):
    linhas = varrer(c, "porletra", max=100)["linhas"]
    ordem = [l["rowid"] for l in linhas]
    n = len(ordem)

    # Toda pagina de tamanho 3, uma a uma. O que se afirma e que a lista
    # concatenada e IGUAL a varredura inteira -- inclusive nas paginas que
    # comecam no primeiro slot de um balde, que eram as que reprovavam.
    juntas, saltos, comecos_de_balde = [], set(), 0
    for pular in range(0, n, 3):
        r = varrer(c, "porletra", pular=pular, max=3)
        juntas += [l["rowid"] for l in r["linhas"]]
        saltos.add(r["salto"])
        if r["linhas"] and (r["linhas"][0]["rowid"] - 1) % 1_000 == 0:
            comecos_de_balde += 1
    afirmar("a_pagina_2_atravessa_balde",
            "paginar de tres em tres com `pular`/`max` devolve exatamente a "
            "mesma lista da varredura inteira, atravessando os baldes",
            ordem, juntas,
            controle="%d das %d paginas comecam no PRIMEIRO slot de um balde "
                     "-- que e o caso que voltava «rowid N nao existe» antes "
                     "do conserto" % (comecos_de_balde, (n + 2) // 3))
    afirmar("na_alfanumerica_o_pular_anda",
            "a posicao NAO e o `rownum` aqui, entao o `pular` anda em vez de "
            "bissetar -- e a resposta diz isso",
            ["passo"], sorted(saltos),
            controle="numa tabela sem particao por letra o mesmo campo diz "
                     "«bisseccao»; aqui bissetar devolveria a linha errada em "
                     "silencio, porque o `rownum` nao cresce com o rowid")

    # E o cursor (`depois`), que e o caminho que a tela usa para «proxima».
    juntas, cursor = [], 0
    while True:
        r = varrer(c, "porletra", depois=cursor, max=3)
        if not r["linhas"]:
            break
        juntas += [l["rowid"] for l in r["linhas"]]
        cursor = r["cursor_fim"]
        if not r["ha_mais"]:
            break
    afirmar("o_cursor_tambem_atravessa_balde",
            "o cursor `depois`/`cursor_fim` percorre a tabela inteira "
            "atravessando os baldes",
            ordem, juntas,
            controle="sao %d paginas de 3 para %d linhas" % ((n + 2) // 3, n))

    # E o `ha_antes`, que e o campo que a recusa vinha embutida.
    faixa = []
    for pular in range(0, n, 3):
        r = varrer(c, "porletra", pular=pular, max=3)
        faixa.append(r["ha_antes"])
    afirmar("ha_antes_responde_em_toda_pagina",
            "`ha_antes` responde em todas as paginas -- e e falso so na "
            "primeira",
            [False] + [True] * (len(faixa) - 1), faixa,
            controle="era exatamente este campo que derrubava a varredura: "
                     "ele chama a pagina ANTERIOR para saber se ela existe")


# ------------------------------ 6. o `desde_rownum` numa tabela por letra

def o_cursor_por_rownum(c):
    """O que a paginacao por `rownum` faz aqui, medido em vez de suposto.

    O `rowid_do_rownum` ja sabe que nao pode bissetar na alfanumerica e varre.
    O que ele NAO faz -- e isso e comportamento, nao defeito -- e continuar na
    ordem de digitacao depois de achar a linha: o `pagina_desde_rownum` entrega
    o resto na ordem dos BALDES."""
    linhas = varrer(c, "porletra", max=100)["linhas"]
    por_rownum = {l["rownum"]: l for l in linhas}
    alvo = 5
    r = varrer(c, "porletra", desde_rownum=alvo, max=4)
    afirmar("desde_rownum_acha_a_linha_certa",
            "`desde_rownum` acha a linha cujo numero de ordem e o pedido, e "
            "nao a que estiver naquela posicao do arquivo",
            por_rownum[alvo]["rowid"], r["linhas"][0]["rowid"],
            controle="a linha de rownum %d e %r" % (alvo,
                                                    por_rownum[alvo]["nome"]))
    seguintes = [l["rownum"] for l in r["linhas"]]
    afirmar("desde_rownum_continua_na_ordem_do_ARQUIVO",
            "e o resto da pagina sai na ordem dos BALDES, nao na ordem de "
            "digitacao -- comportamento, e nao defeito: o `rownum` e o ponto "
            "de partida, e a leitura continua sendo a do arquivo",
            True, seguintes != sorted(seguintes),
            controle="os numeros de ordem da pagina saem assim: %s"
                     % seguintes)


# --------------------------------------------------------------------- corrida

def main():
    if not os.path.exists(oficina.PHXSQLD):
        sys.exit("nao achei %s -- rode `cargo build --release --bin phxsqld`"
                 % oficina.PHXSQLD)
    ocupada, quem = oficina.portao_de_medicao()
    subprocess.run(["rm", "-rf", BASE], check=False)
    p = oficina.subir(BASE, PORTA)
    salto_s = None
    try:
        c = oficina.Conexao(PORTA)
        print("=== a particao alfabetica pela porta de dados ===\n")
        c.ok({"op": "criar_database", "database": DB})
        entrada, rowids = a_linha_cai_no_arquivo_da_letra(c)
        a_ordem_de_digitacao_nao_se_perdeu(c, entrada, rowids)
        salto_s = a_varredura_salta_os_vazios(c)
        a_paginacao_atravessa_balde(c)
        o_cursor_por_rownum(c)
        alterar_a_coluna_de_referencia(c)
        arqs = arquivos("porletra")
        c.fechar()
    finally:
        oficina.baixar(p)

    d = {"afirmacoes": AFIRMACOES,
         "n_afirmacoes": len(AFIRMACOES),
         "falhas": sum(1 for a in AFIRMACOES if not a["ok"]),
         "arquivos_no_disco": arqs,
         "segundos_do_salto": round(salto_s, 3) if salto_s else None,
         "esta_medindo": ocupada, "esta_medindo_quem": quem,
         "versao": subprocess.run([oficina.PHXSQLD, "--version"],
                                  capture_output=True, text=True).stdout.strip()}
    subprocess.run(["rm", "-rf", BASE], check=False)
    with open(RESULTADO, "w") as f:
        json.dump(d, f, indent=2, ensure_ascii=False)
    print("\n%d afirmacoes, %d sem confirmar" % (d["n_afirmacoes"], d["falhas"]))
    print("RESULTADO " + json.dumps({k: v for k, v in d.items()
                                     if k != "afirmacoes"}, ensure_ascii=False))
    print("gravei %s" % RESULTADO)
    return 1 if d["falhas"] else 0


if __name__ == "__main__":
    sys.exit(main())
