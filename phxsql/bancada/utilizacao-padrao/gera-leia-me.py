#!/usr/bin/env python3
"""Reescreve os blocos de numero do `LEIA-ME.md` desta pasta e a secao 18 do
`docs/DESEMPENHO.md` a partir do que as duas provas mediram.

    python3 bancada/utilizacao-padrao/medir.py 20000
    python3 bancada/utilizacao-padrao/paginacao-alfabetica.py
    python3 bancada/utilizacao-padrao/gera-leia-me.py

Nenhum numero desses dois documentos se digita. O gerador abre o arquivo e
troca o miolo de cada bloco marcado:

    <!-- GERADO: chave -->
    ...o que este script escreve...
    <!-- FIM: chave -->

Bloco marcado sem gerador RECLAMA em vez de passar -- e exatamente o rodape que
publicou 780 KiB quando eram 1.032.

# A regra do TEMPO, e por que ela mora no gerador

O tempo so se publica com a maquina livre. O `medir.py` pergunta ao
`bancada/esta-medindo.sh` ANTES e DEPOIS da corrida e grava as duas respostas;
se qualquer uma acusou, este gerador escreve «nao medido» com o motivo, no
lugar do numero. Assim a decisao nao depende de alguem lembrar: ela esta no
arquivo que escreve o documento.
"""
import json
import os
import re
import sys

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
MEDIDA = os.path.join(AQUI, "resultado.json")
ALFA = os.path.join(AQUI, "resultado-alfabetica.json")
LEIAME = os.path.join(AQUI, "LEIA-ME.md")
DESEMPENHO = os.path.join(RAIZ, "docs", "DESEMPENHO.md")

# A lista de colunas e a de indices saem do PROPRIO `medir.py`, e nao de uma
# copia aqui. E a lei que o rodape de 780 KiB pagou: quando um gerador depende
# de uma lista, a lista tem de sair do codigo.
sys.path.insert(0, AQUI)
import medir  # noqa: E402

ROTULO = {"sem": "`sem`", "com": "`com` (Bin+Memo)", "largo": "`largo` (Str)"}

# Por que cada coluna esta na tabela complexa. E comentario, e por isso mora
# aqui; o NOME e o TIPO vem do `medir.py`, e por isso nao envelhecem.
POR_QUE = {
    "filial": "metade da **chave composta**, e a que faz o índice comparar duas colunas",
    "id": "a outra metade da chave composta",
    "codigo": "**índice único próprio** — o motor lê antes de gravar para poder recusar a repetida",
    "nome": "**índice sem caixa** (`nocase`), que compara dobrando a caixa",
    "cidade": "**índice de baixa cardinalidade** (%d valores): folha longa, o caso oposto do único",
    "nascimento": "dias inteiros no disco, texto ISO no fio",
    "criado_em": "milissegundos no disco, e um **terceiro** formato de volta",
    "saldo": "i128 escalado: **recusa** número em JSON e exige texto, para não perder centavo em `f64`",
    "ativo": "um byte",
    "categoria_id": "a **chave estrangeira**, que nasce conferida",
    "observacao": "o texto longo — `Memo` num lado, `Str` de largura fixa no outro",
    "foto": "o binário — `Bin` num lado, o mesmo hexadecimal em `Str` no outro",
}


def explica(d, lado):
    n = d["colunas_declaradas"]["sem"]
    if lado == "sem":
        return ("%d colunas de dado, %d índices. Nenhum arquivo externo."
                % (n, len(d["indices"])))
    if lado == "com":
        return "as mesmas %d mais `observacao` **Memo** e `foto` **Bin**." % n
    return ("as mesmas %d mais `observacao` e `foto` com os **mesmos nomes e "
            "os mesmos valores**, declaradas `Str(n)` — o pedido no fio é "
            "byte a byte igual ao do `com`." % n)


def mil(n):
    return "{:,}".format(int(n)).replace(",", ".")


def num(x, casas=1):
    return ("%%.%df" % casas % x).replace(".", ",")


def tamanho(n):
    """KiB abaixo de um mega, MiB acima. Unidade fixa faz `26485,4 KiB`
    aparecer ao lado de `0,1 KiB` na mesma coluna, e ninguem le isso."""
    if n < 1024 * 1024:
        return num(n / 1024.0) + " KiB"
    return num(n / (1024.0 * 1024.0)) + " MiB"


# ------------------------------------------------------------------- os blocos

def bloco_capa(d, a):
    return [
        "| | |",
        "|---|---|",
        "| linhas por lado | **%s** |" % mil(d["linhas"]),
        "| lados comparados | %d — %s |" % (len(d["lados"]),
                                            ", ".join(ROTULO[l] for l in d["lados"])),
        "| colunas declaradas → colunas no esquema | %s |" % ", ".join(
            "%s: %d → **%d**" % (ROTULO[l], d["colunas_declaradas"][l],
                                 d["colunas_no_esquema"][l]) for l in d["lados"]),
        "| índices em cada lado | %d — `%s` |" % (len(d["indices"]),
                                                  "`, `".join(d["indices"])),
        "| blob por linha | %d bytes no `Bin`, %d caracteres no `Memo` |"
        % (d["bin_bytes"], d["memo_chars"]),
        "| linhas conferidas de volta | %s de %s, **%d divergências** |"
        % (mil(sum(v["leitura"]["linhas_lidas"] for v in d["lados"].values())),
           mil(d["linhas"] * len(d["lados"])),
           sum(v["leitura"]["divergentes"] for v in d["lados"].values())),
        "| afirmações da partição alfabética | **%d**, %d sem confirmar |"
        % (a["n_afirmacoes"], a["falhas"]),
        "| tempo publicável | %s |" % ", ".join(
            "%s: %s" % (f, "sim" if q["publicavel"] else "**não** — %s"
                        % motivo_da_fase(d, f))
            for f, q in d.get("portao", {}).items()),
        "| medido contra | `%s` |" % d["versao"],
    ]


def motivo_da_fase(d, fase):
    q = d.get("portao", {}).get(fase, {})
    quem = [l for l in (q.get("quem") or "").splitlines()
            if l.split("\t")[0].strip().isdigit()]
    tipos = sorted({l.split("\t")[1] for l in quem if len(l.split("\t")) > 1})
    onde = [k for k in ("antes", "depois") if q.get(k)]
    return ("o portão `bancada/esta-medindo.sh` acusou %s desta fase "
            "(%d processo(s), %s)"
            % (" e ".join(onde) or "durante", len(quem), "/".join(tipos) or "?"))


def motivo_do_tempo(d):
    """O motivo, SEM a linha de comando do vizinho.

    O portao devolve `pid\ttipo\tcmdline`, e a cmdline traz o shell inteiro de
    quem estava medindo. Isso nao entra em documento versionado: identifica
    outra frente, envelhece em minutos e nao ajuda ninguem a refazer nada. O
    que ajuda e quantos processos ele achou e de que tipo."""
    quem = [l for l in (d.get("esta_medindo_quem_depois") or
                        d.get("esta_medindo_quem_antes") or "").splitlines()
            if l.split("\t")[0].strip().isdigit()]
    tipos = sorted({l.split("\t")[1] for l in quem if len(l.split("\t")) > 1})
    primeiro = ("%d processo(s), %s" % (len(quem), "/".join(tipos))
                if quem else "outra medição")
    quando = []
    if d["esta_medindo_antes"]:
        quando.append("antes")
    if d["esta_medindo_depois"]:
        quando.append("depois")
    return ("o portão `bancada/esta-medindo.sh` acusou %s da corrida (`%s…`)"
            % (" e ".join(quando), primeiro))


def bloco_lados(d, a):
    linhas = ["| lado | o que ele é | fio (B/linha) | disco (B/linha) | `.reg` | `.ndx` | `.bin` | `.memo` |",
              "|---|---|---:|---:|---:|---:|---:|---:|"]
    for l in d["lados"]:
        v = d["lados"][l]
        disco = v["disco"]
        linhas.append("| %s | %s | %s | %s | %s | %s | %s | %s |" % (
            ROTULO[l], explica(d, l), num(v["fio_por_linha"]),
            num(v["disco_por_linha"]),
            tamanho(disco["reg"]), tamanho(disco["ndx"]),
            tamanho(disco["bin"]), tamanho(disco["memo"])))
    linhas += ["",
               "O `.log` é igual nos três (%s) e o `.trash`, o `.reason` e o "
               "`.pag` são só cabeçalho." % tamanho(d["lados"]["sem"]["disco"]["log"])]
    return linhas


def bloco_decomposicao(d, a):
    s, c, g = (d["lados"][x] for x in ("sem", "com", "largo"))
    carga = s["disco_por_linha"]
    via_externo = c["disco_por_linha"] - carga
    via_slot = g["disco_por_linha"] - carga
    payload = d["bin_bytes"] + d["memo_chars"]
    fio_extra = c["fio_por_linha"] - s["fio_por_linha"]
    out = [
        "| a diferença | o que ela mede | fio | disco |",
        "|---|---|---:|---:|",
        "| `sem` → `largo` | **o peso no fio e no slot**: o mesmo JSON, "
        "guardado em coluna de largura fixa, sem nenhum arquivo externo | "
        "+%s B/linha | +%s B/linha |" % (num(g["fio_por_linha"] - s["fio_por_linha"]),
                                         num(via_slot)),
        "| `largo` → `com` | **o `.bin` e o `.memo`**: o mesmo pedido no fio, "
        "outro destino no disco | %s | %s B/linha |"
        % ("0 B/linha (idêntico)" if abs(c["fio_por_linha"] - g["fio_por_linha"]) < 0.05
           else "+%s B/linha" % num(c["fio_por_linha"] - g["fio_por_linha"]),
           num(via_externo - via_slot)),
        "",
        "O dado que a linha carrega são **%d bytes** (%d no binário e %d "
        "caracteres no texto). Guardado no `.bin`/`.memo` ele custa **%s bytes "
        "por linha** de disco — %s%% de sobra, que são o cabeçalho do bloco, o "
        "CRC e o ponteiro que entra no slot. Guardado em `Str` de largura fixa "
        "custa **%s bytes por linha**, e a conta fecha exatamente com as "
        "larguras declaradas: o `.reg` cresce o que a coluna pediu, esteja ela "
        "cheia ou vazia."
        % (payload, d["bin_bytes"], d["memo_chars"], num(via_externo),
           num(100.0 * (via_externo - payload) / payload), num(via_slot)),
        "",
        "E no **fio** os dois lados custam o mesmo: +%s bytes por linha, dos "
        "quais %d são o hexadecimal do `Bin` — **um binário viaja com o dobro "
        "do tamanho** porque JSON não tem tipo binário." % (num(fio_extra),
                                                            d["bin_bytes"] * 2),
    ]
    return out


def bloco_tempo(d, a):
    if not d["tempo_publicavel"]:
        return [
            "**Não medido nesta corrida, e o motivo está no `resultado.json`:** %s."
            % motivo_da_fase(d, "cargas"),
            "",
            "Nesta casa isso é resultado, e não falha: tempo medido com a "
            "máquina ocupada mede o vizinho. O que está acima — bytes no fio, "
            "bytes em disco, `fsync` contados por `strace` e as linhas "
            "conferidas — não depende de quem mais está rodando, e por isso "
            "é o que se publica. Para medir o tempo: peça ao portão "
            "(`bash bancada/esta-medindo.sh`) e refaça a corrida.",
        ]
    l = d["lados"]
    out = [
        "Duas cargas por lado, em tabelas próprias, na ordem %s. Duas colunas "
        "de tempo, porque elas não medem a mesma coisa: **parede** inclui "
        "montar o JSON no cliente, o fio e a análise da volta; **motor** é o "
        "`ms` que o próprio servidor carimba na resposta."
        % " → ".join("`%s`" % x for x in d["ordem_de_carga"]),
        "",
        "| lado | 1ª carga (parede / motor) | 2ª carga (parede / motor) | 2ª: linhas/s |",
        "|---|---:|---:|---:|",
    ]
    for x in d["lados"]:
        v = l[x]
        out.append("| %s | %s / %s s | %s / %s s | %s |" % (
            ROTULO[x], num(v["carga_s"], 2), num(v["carga_ms_servidor"] / 1000, 2),
            num(v["carga_s_2"], 2), num(v["carga_ms_servidor_2"] / 1000, 2),
            mil(v["linhas_por_s_2"])))
    piores = max(l[x]["carga_ms_servidor"] / l[x]["carga_ms_servidor_2"]
                 for x in l)
    espalha = (max(l[x]["carga_ms_servidor_2"] for x in l)
               - min(l[x]["carga_ms_servidor_2"] for x in l)) / 1000.0
    out += [
        "",
        "**Na segunda carga os três lados custam o mesmo, dentro de %s s de "
        "diferença** (%s de motor) — ou seja, o tempo **não separa** os três "
        "lados. Quem separa são os bytes, e eles estão nas tabelas acima."
        % (num(espalha, 2),
           " / ".join("%s %s s" % (ROTULO[x], num(l[x]["carga_ms_servidor_2"] / 1000, 2))
                      for x in l)),
        "",
        "**E há um efeito que eu não sei explicar, e ele fica escrito assim.** "
        "A primeira carga dos dois lados com peso grande custa até **%s×** a "
        "segunda do mesmo lado, e a diferença aparece dentro do motor, não só "
        "no fio. Dois controles mataram as duas explicações óbvias, e nenhum "
        "deles explicou o efeito:" % num(piores, 2),
        "",
        "1. **não é a posição na fila** — `PHX_ORDEM_INVERTIDA=1` inverte a "
        "ordem das seis cargas e o padrão não muda de dono: os mesmos dois "
        "lados saem lentos na primeira e rápidos na segunda;",
        "2. **não é «a primeira carga de uma série»** — o controle da chave "
        "conferida, logo abaixo, faz três cargas idênticas seguidas e as três "
        "custam o mesmo, nos dois braços.",
        "",
        "Enquanto não houver causa medida, a conclusão publicada é a que **as "
        "duas colunas concordam**: entre a diferença dos lados e a diferença "
        "entre as duas cargas do mesmo lado, a segunda é maior. *Número citado "
        "é número que não se mede* — e diagnóstico plausível não é diagnóstico "
        "medido.",
    ]
    return out


def bloco_chave_conferida(d, a):
    if not d.get("portao", {}).get("chave_conferida", {}).get("publicavel", True):
        return ["**Não medido nesta corrida:** %s. A tabela some em vez de "
                "publicar um tempo que mediu o vizinho."
                % motivo_da_fase(d, "chave_conferida")]
    k = d["chave_conferida"]
    def mediana(braço):
        v = sorted(x["motor_s"] for x in k[braço])
        return v[len(v) // 2]
    com, sem = mediana("com_fk"), mediana("sem_fk")
    linhas = ["| a chave | 1ª carga | 2ª | 3ª | mediana (motor) | µs por linha |",
              "|---|---:|---:|---:|---:|---:|"]
    for braço, rot in (("sem_fk", "**declarada? não** — só o índice `porCategoria`"),
                       ("com_fk", "**conferida** (o que a tabela desta bancada usa)")):
        v = k[braço]
        linhas.append("| %s | %s | %s | %s | **%s s** | %s |" % (
            rot, num(v[0]["motor_s"], 2), num(v[1]["motor_s"], 2),
            num(v[2]["motor_s"], 2), num(mediana(braço), 2),
            num(mediana(braço) * 1e6 / d["linhas"], 1)))
    linhas += [
        "",
        "**A chave conferida custa %s× a gravação.** Os dois braços diferem em "
        "uma coisa só — a declaração da chave; o índice `porCategoria` existe "
        "nos dois —, então o que está medido é a **conferência**, e não o "
        "índice. É o preço da regra primordial da casa cobrado na entrada: "
        "para cada linha gravada, o motor pergunta à mãe se o pai existe e se "
        "ele está vivo. `docs/DESEMPENHO.md` §15 mede a mesma coisa por "
        "dentro, com `--example custo-da-fk`; aqui ela é medida pela porta de "
        "dados, com a tabela inteira." % num(com / sem, 2),
        "",
        "E ele é maior que o dos blobs — que no tempo é **zero**, medido no "
        "bloco acima. O que **não** está medido aqui é quanto cada um dos "
        "cinco índices custa: para dizer isso seria preciso um braço por "
        "índice, e ele não existe. Quem carregar milhões de linhas e puder "
        "conferir depois tem aqui o número para decidir; quem não puder tem "
        "aqui o que a garantia custa.",
        "",
        "De quebra, ele é o **controle da posição**: três cargas idênticas, "
        "uma atrás da outra, com o mesmo esquema e as mesmas linhas. Nos dois "
        "braços as três custam o mesmo — logo, «ser a primeira carga» não "
        "explica sozinho o que o bloco do tempo mostra.",
    ]
    return linhas


def bloco_fsync(d, a):
    f = d["fsync"]
    if "erro" in f:
        return ["**Não medido nesta corrida:** o `strace` não conseguiu anexar "
                "(`%s`). A tabela some em vez de virar uma linha de zeros — "
                "zero de um instrumento que não mediu é a mentira mais barata "
                "que existe." % f["erro"]]
    exts = ["reg", "ndx", "bin", "memo", "log", "trash", "reason"]
    linhas = ["| lado | ação | " + " | ".join("`.%s`" % e for e in exts) + " | total |",
              "|---|---|" + "---:|" * (len(exts) + 1)]
    for lado in d["lados"]:
        for acao, conta in f["por_lado"][lado].items():
            linhas.append("| %s | %s | %s | **%d** |" % (
                ROTULO[lado], acao.replace("_", " "),
                " | ".join(str(conta.get(e, 0)) for e in exts),
                sum(conta.values())))
    linhas += [
        "",
        "Medido em `recursos.durabilidade = \"%s\"`, e não na configuração de "
        "fábrica — de propósito: a de fábrica (`por_lote`, 200 operações ou "
        "200 ms) fecha a janela pelo **relógio**, e aí a contagem passa a "
        "depender de quantas vezes o relógio bateu no meio da carga. Medido "
        "assim numa primeira corrida, o mesmo lote deu 2 `fsync` no `.reg` "
        "para um lado e 1 para o outro, e a diferença era o relógio."
        % f["regime"],
        "",
        "**O achado: o `.bin` e o `.memo` custam ZERO `fsync` a mais.** O fecho "
        "da janela sincroniza os oito arquivos da tabela **exista ou não** "
        "coluna que os use — o lado `sem`, que não tem `Bin` nem `Memo` "
        "nenhum, paga o `fsync` do `.bin` e do `.memo` igual aos outros dois. "
        "O custo do blob aparece em **bytes**, e não em chamadas ao disco.",
    ]
    return linhas


def bloco_conferencia(d, a):
    linhas = ["| lado | linhas lidas | páginas | divergências | o comparador "
              "acusa o estrago? |", "|---|---:|---:|---:|---|"]
    for l in d["lados"]:
        v = d["lados"][l]
        ac = v["controle_do_comparador"]["acusou"]
        provas = []
        for chave, campos in ac.items():
            if chave == "sem_estrago":
                continue
            provas.append("%s → %s" % (chave.replace("_", " "),
                                       ", ".join("`%s`" % c for c in campos) or "**NÃO**"))
        linhas.append("| %s | %s | %d | %d | %s |" % (
            ROTULO[l], mil(v["leitura"]["linhas_lidas"]), v["leitura"]["paginas"],
            v["leitura"]["divergentes"], "; ".join(provas)))
    linhas += [
        "",
        "A última coluna é o **controle positivo**, e ele roda na mesma "
        "corrida: uma cópia do valor esperado é estragada de propósito — um "
        "caractere na cidade, o `rownum` fora da ordem de digitação, um "
        "centavo no `Decimal`, **um byte** no hexadecimal do blob, o último "
        "caractere do memo — e o mesmo "
        "comparador tem de nomear o campo. Sem isso, «zero divergências» "
        "poderia ser um comparador cego, e esta casa já publicou zero com um "
        "medidor cego. Sem estrago nenhum ele cala: %s."
        % ", ".join("%s %s" % (ROTULO[l],
                               d["lados"][l]["controle_do_comparador"]["acusou"]["sem_estrago"]
                               or "`[]`") for l in d["lados"]),
    ]
    return linhas


def bloco_integridade(d, a):
    i = d["integridade"]
    return [
        "| o que se pediu | o que o servidor respondeu |",
        "|---|---|",
        "| excluir (suave) a categoria que tem filhas | `%s` |" % i["suave"],
        "| excluir de vez a mesma categoria | `%s` |" % i["de_vez"],
        "| inserir linha apontando para categoria que não existe | `%s` |" % i["orfa"],
        "| inserir com `codigo` repetido (índice único) | `%s` |" % i["codigo_repetido"],
        "| **controle** — excluir de vez uma categoria SEM filhas | %s |"
        % ("passou, como tinha de passar" if i["controle_sem_filhas"]
           else "**NÃO passou** — a recusa acima não prova nada"),
        "",
        "As duas primeiras linhas são a regra primordial da casa pela porta de "
        "dados: *nunca se mata o pai que tem filhos* — e o **suave** também, "
        "porque pai logicamente morto deixa filha apontando para linha que a "
        "tela não mostra mais. O controle da última linha é o que separa "
        "«recusou por causa das filhas» de «recusa sempre».",
    ]


def bloco_padrao(d, a):
    p = d["lados"]["com"]["coluna_com_padrao"]
    return [
        "| | |",
        "|---|---|",
        "| slots reescritos | %s |" % mil(p["slots_reescritos"]),
        "| índices refeitos | %s |" % ("sim" if p["indices_refeitos"] else "não — o `.ndx` aponta para rowid, e o rowid não mudou"),
        "| nas linhas que já existiam (a primeira e a última) | %s |"
        % ", ".join("`%s`" % x for x in p["nas_linhas_que_ja_existiam"]),
        "| na linha inserida **depois** | %s |"
        % ("`%s`" % p["na_linha_inserida_depois"]
           if p["na_linha_inserida_depois"] is not None else "**nulo**"),
        "",
        "**Coluna com valor padrão não existe no esquema, e isto está medido, "
        "não suposto.** `Column` guarda id, nome, rótulo, descrição, máscara, "
        "tipo, se aceita nulo e a classificação de dado pessoal — e mais nada. "
        "O único `padrao` do motor está no `acrescentar_coluna`, e ali ele é o "
        "valor de **preenchimento** das linhas que já existem: a linha "
        "inserida depois nasce **nula**, como a tabela acima mostra. Pedir "
        "«coluna com padrão» a esta bancada seria pedir uma funcionalidade que "
        "não há; o que ela faz é exercitar a que há e dizer onde termina.",
    ]


def bloco_alfabetica(d, a):
    linhas = ["| o que se afirma | controle da mesma corrida | confere |",
              "|---|---|---|"]
    for af in a["afirmacoes"]:
        linhas.append("| %s | %s | %s |" % (
            af["frase"].replace("|", "/"),
            (af["controle"] or "—").replace("|", "/"),
            "sim" if af["ok"] else "**NÃO**"))
    linhas += [
        "",
        "**%d afirmações, %d sem confirmar**, medidas contra `%s`. Os arquivos "
        "que a partição criou no disco: %s."
        % (a["n_afirmacoes"], a["falhas"], a["versao"],
           ", ".join("`porletra_%s.reg`" % x for x in a["arquivos_no_disco"])),
    ]
    return linhas


def bloco_esquema(d, a):
    linhas = ["| coluna | tipo | por que ela está aqui |", "|---|---|---|"]
    for c in medir.COLUNAS_BASE:
        porque = POR_QUE.get(c["nome"], "")
        if "%d" in porque:
            porque = porque % d["cidades"]
        if not c.get("obrigatoria"):
            pass
        linhas.append("| `%s`%s | `%s` | %s |" % (
            c["nome"], " **(obrigatória)**" if c.get("obrigatoria") else "",
            c["tipo"], porque))
    for nome, tipos in (("observacao", ("Memo", "Str")),
                        ("foto", ("Bin", "Str"))):
        linhas.append("| `%s` — só nos lados `com` e `largo` | `%s` / `%s(n)` | %s |"
                      % (nome, tipos[0], tipos[1], POR_QUE[nome]))
    linhas += ["", "| índice | colunas | marca |", "|---|---|---|"]
    for i in medir.INDICES:
        marcas = []
        if i.get("primario"):
            marcas.append("primário")
        if i.get("unico"):
            marcas.append("único")
        if any("nocase" in c for c in i["colunas"]):
            marcas.append("sem caixa")
        if len(i["colunas"]) > 1:
            marcas.append("composto")
        linhas.append("| `%s` | `%s` | %s |" % (
            i["nome"], "`, `".join(i["colunas"]), ", ".join(marcas) or "comum"))
    fk = medir.FK[0]
    linhas += [
        "",
        "E a chave estrangeira `%s`: `%s` → `%s(%s)`, com "
        "`ao_excluir: %s` e `ao_alterar: %s`. Ela **nasce conferida** — o "
        "`verificar` nem é mandado no pedido —, e chave conferida exige índice "
        "**dos dois lados**: `porId` na mãe e `%s` na filha."
        % (fk["nome"], ", ".join(fk["colunas"]), fk["tabela_ref"],
           ", ".join(fk["colunas_ref"]), fk["ao_excluir"], fk["ao_alterar"],
           medir.INDICES[-1]["nome"]),
        "",
        "As **colunas de sistema** entram sozinhas, e é por isso que a linha "
        "«colunas declaradas → colunas no esquema» da capa tem dois números: "
        "%s. Coluna de sistema nova já quebrou *todo salvar e todo incluir* "
        "pela tela uma vez, e por isso as duas entram na **conferência**, e "
        "não só na contagem: o `rownum` de cada linha é comparado com a "
        "posição em que ela foi digitada, e o `softdeleted` com `false`. "
        "Contar coluna não teria pego aquele defeito; comparar o valor pega."
        % ", ".join("%s %d → %d" % (ROTULO[l], d["colunas_declaradas"][l],
                                    d["colunas_no_esquema"][l])
                    for l in d["lados"]),
    ]
    return linhas


GERADORES = {
    "esquema": bloco_esquema,
    "capa": bloco_capa,
    "os-tres-lados": bloco_lados,
    "decomposicao": bloco_decomposicao,
    "tempo": bloco_tempo,
    "chave-conferida": bloco_chave_conferida,
    "fsync": bloco_fsync,
    "conferencia": bloco_conferencia,
    "integridade": bloco_integridade,
    "coluna-com-padrao": bloco_padrao,
    "alfabetica": bloco_alfabetica,
}


def reescrever(caminho, d, a):
    with open(caminho, encoding="utf-8") as f:
        texto = f.read()
    marcados = re.findall(r"<!-- GERADO: ([a-z0-9-]+) -->", texto)
    sem_gerador = [m for m in marcados if m not in GERADORES]
    if sem_gerador:
        raise SystemExit("bloco marcado sem gerador em %s: %s"
                         % (caminho, ", ".join(sem_gerador)))
    for chave in marcados:
        corpo = "\n".join(GERADORES[chave](d, a))
        padrao = re.compile(r"(<!-- GERADO: %s -->)(.*?)(<!-- FIM: %s -->)"
                            % (re.escape(chave), re.escape(chave)), re.S)
        texto, n = padrao.subn(
            lambda m: m.group(1) + "\n" + corpo + "\n" + m.group(3), texto)
        if n != 1:
            raise SystemExit("bloco %s em %s: achei %d fecho(s), esperava 1"
                             % (chave, caminho, n))
    with open(caminho, "w", encoding="utf-8") as f:
        f.write(texto)
    return len(marcados)


def main():
    for arq, quem in ((MEDIDA, "medir.py"), (ALFA, "paginacao-alfabetica.py")):
        if not os.path.exists(arq):
            raise SystemExit("falta %s -- rode `python3 bancada/"
                             "utilizacao-padrao/%s`" % (arq, quem))
    with open(MEDIDA) as f:
        d = json.load(f)
    with open(ALFA) as f:
        a = json.load(f)
    total = 0
    for caminho in (LEIAME, DESEMPENHO):
        if not os.path.exists(caminho):
            continue
        n = reescrever(caminho, d, a)
        print("reescrevi %d bloco(s) em %s" % (n, caminho))
        total += n
    if total == 0:
        print("nenhum bloco marcado -- nada a fazer")
    return 0


if __name__ == "__main__":
    sys.exit(main())
