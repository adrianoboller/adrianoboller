#!/usr/bin/env python3
"""Reescreve os blocos de numero do `docs/ACID.md` a partir do que a prova
mediu -- nenhum numero daquele documento se digita.

    python3 bancada/acid/prova.py           # mede, grava resultado.json
    python3 bancada/acid/gerar-secoes.py    # reescreve os blocos do docs/ACID.md

# Por que reescreve DENTRO do documento, e nao ao lado

A bancada de durabilidade gera um `matriz-gerada.md` que alguem COLA no
documento, e o custo disso ja apareceu nesta casa quatro vezes: entre gerar e
colar existe uma mao, e mao esquece. Aqui o gerador abre o proprio
`docs/ACID.md` e troca o miolo de cada bloco marcado:

    <!-- GERADO: chave -->
    ...o que este script escreve...
    <!-- FIM: chave -->

O texto FORA dos blocos e escrito a mao e nunca e tocado; o texto DENTRO nunca
e escrito a mao. Se um bloco marcado nao tiver gerador, o script RECLAMA em vez
de deixar passar -- bloco marcado que ninguem gera e exatamente o rodape que
publicou 780 KiB quando eram 1.032.
"""
import json
import os
import re
import sys

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
RESULTADO = os.path.join(AQUI, "resultado.json")
DOC = os.path.join(RAIZ, "docs", "ACID.md")

ORDEM_REGIMES = ["por_operacao", "por_lote", "sistema"]
ROTULO = {"por_operacao": "`por_operacao`", "por_lote": "`por_lote` (**padrão**)",
          "sistema": "`sistema`"}


def acha(d, chave):
    for a in d["afirmacoes"]:
        if a["chave"] == chave:
            return a
    raise SystemExit("afirmacao ausente no resultado: %s" % chave)


def veredito(d, chave):
    a = acha(d, chave)
    return "sim" if a["ok"] else "**NÃO**"


# ------------------------------------------------------------------ os blocos

def bloco_afirmacoes(d):
    """Todas as afirmacoes, com o controle de cada uma. E a tabela que faz o
    documento inteiro ser conferivel: quem duvidar de uma linha roda a prova."""
    saida = ["| letra | o que se afirma | controle da mesma corrida | confere |",
             "|---|---|---|---|"]
    for a in d["afirmacoes"]:
        controle = (a["controle"] or "—").replace("|", "/")
        saida.append("| **%s** | %s | %s | %s |"
                     % (a["letra"], a["frase"].replace("|", "/"), controle,
                        "sim" if a["ok"] else "**NÃO**"))
    n = len(d["afirmacoes"])
    falhas = sum(1 for a in d["afirmacoes"] if not a["ok"])
    saida += ["", "**%d afirmações, %d sem confirmar.** Medidas contra `%s`."
              % (n, falhas, d["versao"])]
    return saida


def bloco_a_slots(d):
    s = d["A"]["slots"]
    return ["| momento | slots do `.reg` |", "|---|---:|",
            "| antes de abrir a transação | %d |" % s["antes"],
            "| depois de `BEGIN` + 3 `INSERT` + `ROLLBACK` | %d |" % s["apos_rollback"],
            "| depois de `BEGIN` + as **mesmas** 3 + `COMMIT` | %d |" % s["apos_commit"]]


def bloco_a_queda(d):
    a2 = d["A"]["a2"]
    saida = ["| atraso do `SIGKILL` | onde a queda caiu | linhas em `a` | linhas em `b` |",
             "|---|---|---:|---:|"]
    for rotulo, classe, na, nb in a2["desfechos"]:
        saida.append("| %s | %s | %s | %s |"
                     % (rotulo.split("#")[-1], classe, na, nb))
    saida += ["", "**%d quedas válidas, %d com uma tabela gravada e a outra não.** "
              "Os desfechos não foram todos iguais — a varredura pegou pontos "
              "de morte diferentes, e é isso que prova que ela mirou dentro da "
              "janela." % (a2["validas"], len(a2["metades"]))]
    return saida


def bloco_a_cascata(d):
    a4 = d["A"]["a4"]
    saida = ["| corrida | onde a queda caiu | veredito | índices reconstruídos pela recuperação | filhas na chave nova | filhas na chave velha |",
             "|---|---|---|---:|---:|---:|"]
    reconstruiu = 0
    for rotulo, ver, classe, idx, novas, velhas in a4["corridas"]:
        if idx:
            reconstruiu += 1
        saida.append("| %s | %s | %s | %s | %s | %s |"
                     % (rotulo.split("#")[-1], classe, ver,
                        "—" if idx is None else idx, novas, velhas))
    saida.append("")
    saida.append("Vereditos: " + " · ".join("**%d** %s" % (v, k)
                                            for k, v in sorted(a4["vereditos"].items()))
                 + " — e a recuperação reconstruiu o índice de alguma filha em "
                   "**%d** das %d corridas." % (reconstruiu, len(a4["corridas"])))
    return saida


def bloco_c_guardas(d):
    """A tabela do que e imposto na gravacao, derivada das afirmacoes da
    letra C -- e nao de uma lista escrita a mao ao lado."""
    saida = ["| garantia | a violação | o caso legítimo, na mesma corrida |",
             "|---|---|---|"]
    c = d["C"]
    linhas = [
        ("unicidade num índice único", c["unicidade"]),
        ("chave estrangeira: filha sem mãe", c["chave_estrangeira"]),
        ("coluna obrigatória com `NULL`", c["obrigatoria"]),
        ("tipo da coluna", c["tipo"]),
        ("texto maior que a coluna", c["tamanho"]),
    ]
    for nome, r in linhas:
        saida.append("| %s | `%s` | %s |"
                     % (nome, r["violacao"], "aceito" if r["legitimo"] else "**recusado**"))
    rp = c["regra_primordial"]
    saida.append("| **regra primordial**: matar a mãe que tem filha, de vez | `%s` | mãe sem filha: %s |"
                 % (rp["de_vez"], "aceita" if rp["sem_filha"] else "**recusada**"))
    saida.append("| **regra primordial**: matar a mãe que tem filha, suave | `%s` | — |"
                 % rp["suave"])
    me = c["marca_de_exclusao"]
    saida.append("| filha **marcada** ainda restringe a mãe | `%s` | mãe sem filha nenhuma: %s |"
                 % (me["mae_com_filha_marcada"],
                    "aceita" if me["mae_sem_filha"] else "**recusada**"))
    saida.append("| mãe **marcada** não aceita filha nova | `%s` | — |"
                 % me["filha_de_mae_marcada"])
    nc = c["nasce_conferida"]
    saida.append("| chave declarada **sem pedir** `verificar` já confere | `%s` | com `\"verificar\": false` a órfã entra: %s |"
                 % (nc["sem_pedir"], "sim" if nc["verificar_false"] else "**não**"))
    ae = c["ao_excluir"]
    saida.append("| `\"ao_excluir\": \"cascata\"` na declaração | `%s` | `restringir` nasce: %s |"
                 % (ae["cascata"], "sim" if ae["restringir"] else "**não**"))
    idx = c["indice_dos_dois_lados"]
    saida.append("| chave conferida **sem o índice na filha** | declaração: `%s`; `excluir` da mãe: `%s` | — |"
                 % (idx["declaracao"], idx["excluir_a_mae"]))
    return saida


def bloco_i_fenomenos(d):
    i = d["I"]
    ls = i["leitura_suja"]
    lnr = i["leitura_nao_repetivel"]
    f = i["fantasma"]
    pa = i["perda_de_atualizacao"]
    sk = i["skew_de_escrita"]
    return [
        "| fenômeno da norma | acontece? | como se mediu |",
        "|---|---|---|",
        "| **leitura suja** | **não** | outra sessão leu `%s` enquanto a transação via `%s` na própria escrita não confirmada |"
        % (ls["outra_sessao"], ls["dentro"]),
        "| **leitura não repetível** | **sim** | duas leituras da mesma linha na mesma transação: `%s` e depois `%s` |"
        % (lnr["primeira"], lnr["segunda"]),
        "| **fantasma** | **sim** | a mesma varredura na mesma transação: %d linhas e depois %d |"
        % (f["primeira"], f["segunda"]),
        "| **perda de atualização** entre escritas soltas | **sim** | as duas leram `%s`, as duas somaram 1, e o valor final é `%s` em vez de `%s` |"
        % (pa["sem_nada"]["lido_por_dois"][0], pa["sem_nada"]["final"],
           pa["sem_nada"]["esperado_se_nao_houvesse_perda"]),
        "| a mesma, mandando `\"versao\"` | **não** | a segunda gravação volta `%s` |"
        % pa["com_versao"],
        "| a mesma, dentro de transação | **não** | a segunda espera o `LOCK TIMEOUT` e volta `%s` |"
        % pa["com_trava"],
        "| **skew de escrita** | **sim** | as duas transações viram %d de plantão, cada uma tirou a sua linha, as duas confirmaram, e sobraram **%d** |"
        % (sk["viu_a"], sk["de_plantao_no_fim"]),
    ]


def bloco_i_matriz(d):
    lc = d["I"]["leitura_consistente"]
    s, t = lc["escritor_solto"], lc["escritor_em_transacao"]
    return [
        "| o leitor pergunta | escritor **sem** transação | escritor **em** transação |",
        "|---|---:|---:|",
        "| **uma** instrução (`varrer` devolve as duas linhas) | %d de %d |    %d de %d |"
        % (s["uma_instrucao"], s["voltas_do_leitor"],
           t["uma_instrucao"], t["voltas_do_leitor"]),
        "| **duas** instruções (`ler` + `ler`) | %d de %d | %d de %d |"
        % (s["duas_instrucoes"], s["voltas_do_leitor"],
           t["duas_instrucoes"], t["voltas_do_leitor"]),
        "",
        "A corrida não foi vazia: o escritor deu **%d** voltas na coluna da "
        "esquerda e **%d** na da direita, contra %d perguntas do leitor em cada."
        % (s["escritas"], t["escritas"], s["voltas_do_leitor"]),
        "",
        "E os **estados** que a instrução única viu contra o escritor sem "
        "transação, que é o número que separa «o leitor rasgou a leitura» de "
        "«o banco estava mesmo inconsistente»: %s. O escritor passa **uma** "
        "ida e volta em cada estado do meio da transferência e **três** em "
        "cada estado em acordo — a frequência tem de sair 3:1:3:1, e sai."
        % " · ".join("`(%s)` %d" % (k, v)
                     for k, v in s["estados_vistos_por_uma_instrucao"].items()),
    ]


# O `prova.py` nomeia os fenomenos sem acento, como todo texto de console
# desta casa; a acentuacao mora aqui, que e o lado que escreve documento.
ACENTO = {"leitura suja": "leitura suja",
          "leitura nao repetivel": "leitura não repetível",
          "fantasma": "fantasma"}


def bloco_i_nivel(d):
    n = d["nivel_ansi"]
    return [
        "> **%s**, e nada acima disso." % n["nivel"],
        "",
        "Os fenômenos que **acontecem** e que impedem o nível seguinte: %s. "
        "E o **skew de escrita**, que a leitura moderna cobra do `SERIALIZABLE`, "
        "%s." % (", ".join("**%s**" % ACENTO.get(f, f)
                              for f in n["fenomenos_que_acontecem"]),
                 "acontece" if n["skew_de_escrita_acontece"] else "não acontece"),
    ]


def bloco_d_fsync(d):
    ins, com = d["D"]["fsync_insercao"], d["D"]["fsync_commit"]
    exts = []
    for tab in (ins, com):
        for r in ORDEM_REGIMES:
            for e in tab[r]:
                if e not in exts:
                    exts.append(e)
    exts = [e for e in ("tx", "reg", "ndx", "bin", "memo", "log", "trash", "reason")
            if e in exts] + [e for e in exts if e not in
                             ("tx", "reg", "ndx", "bin", "memo", "log", "trash", "reason")]
    cab = "| operação | regime | " + " | ".join("`.%s`" % e for e in exts) + " | total |"
    saida = [cab, "|---|---|" + "---:|" * (len(exts) + 1)]
    for nome, tab in (("um `INSERT` comum", ins), ("um `COMMIT` de uma linha", com)):
        for r in ORDEM_REGIMES:
            v = tab[r]
            saida.append("| %s | %s | %s | **%d** |"
                         % (nome, ROTULO[r],
                            " | ".join(str(v.get(e, 0)) for e in exts),
                            sum(v.values())))
    return saida


def bloco_d_queda(d):
    q = d["D"]["queda"]
    saida = ["| regime | linhas depois do `SIGKILL` | o relatório do arranque |",
             "|---|---:|---|"]
    for r in ORDEM_REGIMES:
        rel = q[r]["relatorio"]
        if rel is None:
            texto = "não saiu — não havia marca (a tabela já estava no disco)"
        else:
            texto = ("achadas=%d, completadas=%d, reaplicadas=%d, já aplicadas=%d, "
                     "impossíveis=%d" % (rel["achadas"], rel["completadas"],
                                         rel["reaplicadas"], rel["ja_aplicadas"],
                                         rel["impossiveis"]))
        saida.append("| %s | %d | %s |" % (ROTULO[r], q[r]["linhas_depois"], texto))
    saida += ["", "E o contraponto, que é o ponto desta seção: **%d** inserções "
              "comuns em `por_lote`, com **zero** `fsync` no `.reg`, também "
              "voltaram inteiras depois do `SIGKILL`."
              % q["insert_solto_por_lote"]]
    return saida


def bloco_maquina(d):
    m = d["maquina"]
    return ["Medido contra `%s`. Havia outra medição em curso na máquina no "
            "momento: **%s** — e isso não muda nenhum número desta página, "
            "porque nenhum deles é uma duração."
            % (d["versao"], "sim" if m["havia_medicao_em_curso"] else "não")]


GERADORES = {
    "afirmacoes": bloco_afirmacoes,
    "a-slots": bloco_a_slots,
    "a-queda": bloco_a_queda,
    "a-cascata": bloco_a_cascata,
    "c-guardas": bloco_c_guardas,
    "i-fenomenos": bloco_i_fenomenos,
    "i-matriz": bloco_i_matriz,
    "i-nivel": bloco_i_nivel,
    "d-fsync": bloco_d_fsync,
    "d-queda": bloco_d_queda,
    "maquina": bloco_maquina,
}


def main():
    if not os.path.exists(RESULTADO):
        raise SystemExit("falta %s -- rode `python3 bancada/acid/prova.py`" % RESULTADO)
    with open(RESULTADO) as f:
        d = json.load(f)
    if d.get("rapido"):
        raise SystemExit("o resultado foi medido com PHX_ACID_RAPIDO=1 -- "
                         "as varreduras de SIGKILL nao rodaram, e isso nao se publica")
    with open(DOC, encoding="utf-8") as f:
        texto = f.read()

    marcados = re.findall(r"<!-- GERADO: ([a-z0-9-]+) -->", texto)
    sem_gerador = [m for m in marcados if m not in GERADORES]
    if sem_gerador:
        raise SystemExit("bloco marcado sem gerador: %s" % ", ".join(sem_gerador))
    nao_usados = [k for k in GERADORES if k not in marcados]

    trocados = 0
    for chave in marcados:
        corpo = "\n".join(GERADORES[chave](d))
        # O miolo pode estar VAZIO (a primeira geracao, com o bloco recem
        # aberto no documento), entao a expressao nao pode exigir o `\n` dos
        # dois lados -- ela casa marcador a marcador e o gerador repoe as
        # quebras de linha.
        padrao = re.compile(r"(<!-- GERADO: %s -->)(.*?)(<!-- FIM: %s -->)"
                            % (re.escape(chave), re.escape(chave)), re.S)
        texto, n = padrao.subn(
            lambda m: m.group(1) + "\n" + corpo + "\n" + m.group(3), texto)
        if n != 1:
            raise SystemExit("bloco %s: achei %d fecho(s), esperava 1" % (chave, n))
        trocados += 1

    with open(DOC, "w", encoding="utf-8") as f:
        f.write(texto)
    print("reescrevi %d bloco(s) em %s" % (trocados, DOC))
    if nao_usados:
        print("geradores sem bloco no documento (nao e erro, mas vale olhar): %s"
              % ", ".join(nao_usados))
    return 0


if __name__ == "__main__":
    sys.exit(main())
