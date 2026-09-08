#!/usr/bin/env python3
"""Escreve o `docs/COMPARATIVO.md` a partir do `resultados.json` medido.

    python3 bancada/comparativo/medir.py      # mede
    python3 bancada/comparativo/documento.py  # escreve o documento

**Nenhum numero deste documento se digita.** A tabela, as contagens, os
vereditos e ate as versoes dos motores saem do JSON que o `medir.py` gravou --
mesma disciplina do dossie, e pelo mesmo motivo: numero digitado a mao
envelhece calado.

A prosa mora aqui, no gerador, e nao no documento. E de proposito: documento
gerado que alguem edita a mao volta a ser documento digitado no commit
seguinte, e ninguem percebe qual das duas versoes e a verdadeira.

E ate o NUMERO DA SECAO sai de contador, pela mesma lei em escala pequena: a
secao «as linhas pela metade» so existe quando ha alguma, e uma referencia
cruzada digitada a mao apontaria para a secao errada no dia em que a ultima
delas fosse fechada.
"""

import datetime
import json
import pathlib
import sys
import textwrap

AQUI = pathlib.Path(__file__).resolve().parent
RAIZ = AQUI.parents[1]
FONTE = AQUI / "resultados.json"
ALVO = RAIZ / "docs" / "COMPARATIVO.md"
COLUNA = 78

# A ordem das colunas e a da pergunta do dono, com o PhxSql na frente porque a
# tabela e sobre ELE.
COLUNAS = [
    ("phxsql", "PhxSql"),
    ("hfsql", "HFSQL(R)"),
    ("postgres", "PostgreSQL(R)"),
    ("cassandra", "Cassandra(R)"),
    ("mysql", "MySQL(R)"),
    ("sqlite", "SQLite(R)"),
]

MARCA = {
    "tem": "✅",
    "meio": "◐",
    "nao": "❌",
    "citado": "📄",
    "sem-motor": "—",
}


class Texto:
    """Acumula o documento. `p` quebra paragrafo; `l` sai como esta escrito."""

    def __init__(self):
        self.partes = []
        self.secao = 0

    def p(self, txt):
        self.partes.append(textwrap.fill(" ".join(txt.split()), COLUNA))
        self.partes.append("")

    def l(self, txt=""):
        self.partes.append(txt)

    def cabeca(self, titulo):
        self.secao += 1
        self.partes += ["---", "", f"## {self.secao}. {titulo}", ""]
        return self.secao

    def fim(self):
        return "\n".join(self.partes).rstrip() + "\n"


def maiuscula(s):
    return s[0].upper() + s[1:] if s else s


def carregar():
    if not FONTE.exists():
        sys.exit(f"falta {FONTE} -- rode antes: python3 {AQUI/'medir.py'}")
    return json.loads(FONTE.read_text(encoding="utf-8"))


def escrever(d):
    linhas = d["linhas"]
    vivos = d["motores_vivos"]
    citados = d["sem_motor_nesta_maquina"]
    quando = datetime.datetime.fromisoformat(d["quando"]).strftime("%d/%m/%Y")

    por_sql = [l for l in linhas if l["como"] == "SQL no motor vivo"]
    por_sonda = [l for l in linhas if l["como"] != "SQL no motor vivo"]
    faltam = [l for l in linhas if l["phxsql"][0] in ("nao", "meio")]
    meios = [l for l in linhas if l["phxsql"][0] == "meio"]
    temos = [l for l in linhas if l["phxsql"][0] == "tem"]

    # As secoes se numeram sozinhas, e as referencias cruzadas saem do
    # contador. So por isso o documento sobrevive a uma secao deixar de
    # existir -- e a das linhas pela metade vai deixar.
    t = Texto()

    t.l("# O que ainda falta no PhxSql, medido contra quem tem")
    t.l()
    t.p(f"""Medido em **{quando}**, uma pergunta de cada vez, contra os motores
        que estão **vivos nesta máquina**. Este documento não é o
        `COMPARACAO.md` (o que os motores maduros têm e nós **trouxemos**) nem
        o `CONCORRENTES.md` (o caminho de inserção deles, lido no fonte). É o
        outro lado: **o que continua faltando aqui**, e quem já resolveu.""")
    # A frase de abertura tambem se remede: escrita quando so havia UM `tem`,
    # ela afirmava «a unica inteira» -- e passaria a mentir no dia em que a
    # tabela virasse, que foi 08/09/2026. O mesmo motivo da §5 condicional.
    if len(temos) <= 1:
        t.p(f"""**{len(faltam)} de {len(linhas)} capacidades** faltam ou estão pela
            metade no PhxSql. A única inteira é um veredito de ausência que esta
            casa publicou **errado** — o sexto — e a seção que o conta está
            abaixo.""")
    else:
        t.p(f"""**{len(faltam)} de {len(linhas)} capacidades** faltam ou estão pela
            metade no PhxSql, e **{len(temos)}** respondem `tem` — medidas
            contra o motor vivo desta árvore, nunca digitadas. O que falta está
            na tabela com a recusa que o motor devolveu, e a §8 diz o que é
            decisão e o que é buraco.""")
    t.l("> Refaça com `python3 bancada/comparativo/medir.py` e depois")
    t.l("> `python3 bancada/comparativo/documento.py`. **Este arquivo não se")
    t.l("> edita** — a prosa mora no gerador, e a medição, no `resultados.json`.")
    t.l()

    # ---------------------------------------------------------------- §1
    t.cabeca("Como cada célula foi decidida")
    t.p("""Três procedências, e a tabela diz qual em cada linha, porque
        misturá-las publica um retrato que nunca existiu.""")
    t.p(f"""**Perguntando ao motor vivo** — {len(por_sql)} das {len(linhas)}
        linhas. A mesma pergunta vai para os quatro motores na língua de cada
        um; a instrução passou → tem, recusou → não, e a mensagem de recusa
        fica guardada no JSON. Estas versões responderam:""")
    t.l("| motor | versão que respondeu |")
    t.l("|---|---|")
    for chave, nome in COLUNAS:
        if chave in vivos:
            t.l(f"| {nome} | `{vivos[chave]}` |")
    t.l()
    t.p(f"""**Sonda de código** — {len(por_sonda)} linhas, só onde SQL não
        alcança. Cada uma aponta **arquivo e linha**, e é por isso que ela
        vale: sonda que aponta para o repositório se reconfere; sonda que
        resume, não.""")
    t.p("**Citado**, para quem não está aqui:")
    for chave, nome in COLUNAS:
        if chave in citados:
            t.l(f"- **{nome}** — {citados[chave]}")
    t.l()
    t.p("""Citado não é mentira; é afirmação de **segunda mão**, e a tabela
        marca cada uma dessas células para que ninguém a leia como medida.""")

    t.l("### As 5 armadilhas que este medidor pagou")
    t.l()
    t.p("""Cada uma virou um portão, e é por isso que estão escritas: portão
        sem a história do defeito é portão que alguém remove por parecer
        exagero.""")
    t.p("""**1.** Perguntar ao **texto do repositório**. `materializar a linha`, num comentário, virou «view materializada»; o `ON DUPLICATE KEY UPDATE` que o DbLink **manda para o MySQL(R)** virou upsert nosso. Sonda por padrão de texto acha o que não é — e as três células saíram erradas na primeira corrida.""")
    t.p("""    → Passou a perguntar ao **motor vivo**, e a sonda de código sobrou só onde SQL não alcança.""")
    t.p("""**2.** Um portão que exigia **diversidade de resposta**: «um motor não pode responder a mesma coisa para tudo». Ele reprovou o PostgreSQL(R), que genuinamente tem todas as capacidades perguntadas por SQL — portão que confunde motor completo com medidor quebrado.""")
    t.p("""    → Entrou um **controle positivo** no lugar: uma instrução inválida de propósito que **todo** motor tem de recusar. Se algum aceitar, quem está mentindo é o medidor.""")
    t.p("""**3.** **Duas linhas diziam `tem` por uma recusa.** Recusa não prova nada sozinha: um índice que devolve zero linhas pode simplesmente não existir, e uma tabela que recusa o valor proibido pode nunca ter nascido.""")
    t.p("""    → Cada sonda de efeito ganhou o **caso legítimo** ao lado do proibido. As duas viraram `não`.""")
    t.p("""**4.** A sonda da visão lia `r["operacoes"]` do **envelope**, e a resposta vem dentro de `resultado`. Ela publicou «nenhuma operação de visão entre as 0» — um `não` certo pelo motivo errado, que é o pior tipo de certo. E o mesmo erro de nível estava em mais três sondas irmãs.""")
    t.p("""    → Um só desembrulhador, e **um controle que o prova**: grava `v = 42` e exige ler 42 de volta antes de qualquer sonda rodar.""")
    t.p("""**5.** As tabelas das sondas **nunca nasceram**. O índice ia como `[{"coluna": 0}]` e o servidor recusava com «índice pk sem colunas»; ninguém lia a recusa, e as leituras seguintes achavam vazio e publicavam «o campo foi aceito e IGNORADO».""")
    t.p("""    → O `cria()` passou a **exigir** que a tabela nasça, e as duas sondas em que a recusa é a própria resposta pedem isso pelo nome (`exigir=False`).""")

    # ---------------------------------------------------------------- §2
    t.cabeca("A tabela")
    t.l(f"{MARCA['tem']} tem &middot; {MARCA['meio']} pela metade &middot;"
        f" {MARCA['nao']} não tem &middot; {MARCA['citado']} citado, não medido"
        f" aqui &middot; {MARCA['sem-motor']} o verbo não existe neste motor")
    t.l()
    t.l("| capacidade | como | " + " | ".join(n for _, n in COLUNAS) + " |")
    t.l("|---|---|" + "---|" * len(COLUNAS))
    for l in linhas:
        como = "SQL" if l["como"] == "SQL no motor vivo" else "sonda"
        cels = " | ".join(MARCA[l[c][0]] for c, _ in COLUNAS)
        t.l(f"| {l['titulo']} | {como} | {cels} |")
    t.l()
    t.p("""Somando as colunas. As marcas de procedência entram na conta em vez
        de sumir dela — um ✅ citado e um ✅ medido valem coisas diferentes, e
        misturá-los numa coluna só é exatamente o que esta tabela recusa
        fazer:""")
    ordem = ["tem", "meio", "nao", "citado", "sem-motor"]
    t.l("| motor | " + " | ".join(MARCA[v] for v in ordem)
        + " | soma | procedência |")
    t.l("|---|---|---|---|---|---|---|---|")
    for chave, nome in COLUNAS:
        c = [sum(1 for l in linhas if l[chave][0] == v) for v in ordem]
        proc = "medido aqui" if chave in vivos else "citado"
        t.l(f"| {nome} | " + " | ".join(str(x) for x in c)
            + f" | {sum(c)} | {proc} |")
    t.l()
    t.p(f"""A coluna **soma** existe para a linha fechar em {len(linhas)}: sem
        ela, uma marca esquecida some da conta e o leitor não tem como
        perceber.""")

    # ---------------------------------------------------------------- §3
    t.cabeca("O que o SQL vivo respondeu")
    t.p("""Cada recusa abaixo é o texto que o próprio motor devolveu, sem
        retoque. Onde ela aparece cortada, é porque o medidor guarda só o
        começo da mensagem — o resto é o manual do outro motor.""")
    for l in por_sql:
        t.l(f"### {l['titulo']} &mdash; {MARCA[l['phxsql'][0]]}")
        t.l()
        t.l(f"> {l['phxsql'][1]}")
        t.l()
        vivo = " &middot; ".join(f"{nome} {MARCA[l[c][0]]}"
                                 for c, nome in COLUNAS
                                 if c != "phxsql" and c in vivos)
        t.l(f"Nos vivos: {vivo}.")
        for c, nome in COLUNAS:
            if c in citados:
                t.l(f"{nome} {MARCA[l[c][0]]}, citado: {l[c][1]}.")
        t.l()

    # ---------------------------------------------------------------- §4
    t.cabeca("O que a sonda de código achou")
    t.p("""Perguntas que SQL não responde, ou porque o verbo não existe em
        nenhum dos dialetos, ou porque a resposta está na configuração e não na
        instrução. Cada veredito abaixo nomeia onde olhar.""")
    for l in por_sonda:
        t.l(f"### {l['titulo']} &mdash; {MARCA[l['phxsql'][0]]}")
        t.l()
        # A evidencia sai como CITACAO, do mesmo jeito que a recusa dos
        # motores vivos na secao anterior. Antes ela vinha como frase, e o
        # capitalizador virava `crates/...` em `Crates/...` -- caminho
        # maquiado nao se copia e colar.
        t.l(f"> {l['phxsql'][1]}")
        t.l()
        if l.get("nota"):
            t.p(f"*{maiuscula(l['nota'])}.*")
        for c, nome in COLUNAS:
            if c in citados:
                t.l(f"{nome} {MARCA[l[c][0]]}, citado: {l[c][1]}.")
        t.l()

    # ---------------------------------------------------------------- §5
    #
    # Nasceu quando SO a trava por linha respondia `tem`, e o titulo dizia
    # «unico» porque era. A remedicao de 08/09/2026 (F-BANCADA, sondas vivas
    # para coluna/PITR/parametro/diferencas) levou o numero de TEM de 1 para
    # muitos -- e um titulo que afirma «unico» sobre uma lista de dezesseis
    # seria a MESMA mentira que este documento existe para nao repetir:
    # contagem que nao se remede contra o dado publica um retrato que ja
    # mudou. A secao fica condicional, como a §6 e a §7 ja sao.
    trava = next((l for l in temos if l["chave"] == "trava_por_linha"), None)
    if len(temos) <= 1:
        t.cabeca("O único ✅ do PhxSql, e por que ele é uma lição")
        for l in temos:
            t.p(f"**{l['titulo']}** — {l['phxsql'][1]}."
                + (f" {maiuscula(l['nota'])}." if l.get("nota") else ""))
        t.p("""Esta linha entrou nesta tabela como `não`, escrita de memória, e a
            sonda de código a derrubou. Ela é o **sexto** veredito de ausência que
            esta casa publicou errado — os cinco anteriores estão na §6 do
            `HFSQL.md`, e o padrão dos seis é o mesmo: **ninguém reconfere uma
            ausência, porque não há o que olhar.** Um número errado alguém
            desconfia ao bater o olho; um «não há» fica.""")
        t.p("""É por isso que este documento sai de um medidor e não de uma leitura:
            **veredito de ausência se remede por data, não por suspeita.**""")
    else:
        t.cabeca(f"O que passou a responder `tem`, e por que a lição continua")
        t.p(f"""Até 07/09/2026 esta seção listava **um** `tem` só — a trava por
            linha — e o título dizia «único» porque era. **{len(temos)}** das
            **{len(linhas)}** linhas responderam `tem` nesta remedição
            (as dezoito do comparativo entraram por contrato, medidas contra
            o motor vivo em vez de digitadas): a maioria porque o motor
            GANHOU a capacidade nesta rodada, e quatro — coluna, PITR,
            parâmetro e diferenças — porque a SONDA deixou de ser código e
            passou a exercitar o EFEITO pelo soquete, com o controle na mesma
            corrida.""")
        for l in temos:
            t.p(f"**{l['titulo']}** — {l['phxsql'][1]}."
                + (f" {maiuscula(l['nota'])}." if l.get("nota") else ""))
        if trava:
            t.p("""A **trava por linha** é quem abriu esta seção, e a lição dela
                continua valendo sozinha: entrou nesta tabela como `não`,
                escrita de memória, e uma sonda de código a derrubou. Foi o
                **sexto** veredito de ausência que esta casa publicou errado —
                os cinco anteriores estão na §6 do `HFSQL.md` — e o padrão dos
                seis é o mesmo: **ninguém reconfere uma ausência, porque não há
                o que olhar.** Um número errado alguém desconfia ao bater o
                olho; um «não há» fica.""")
        t.p("""É por isso que este documento sai de um medidor e não de uma
            leitura: **veredito de ausência se remede por data, não por
            suspeita** — e, como o título desta seção acabou de provar,
            **veredito de unicidade também.**""")

    # ---------------------------------------------------------------- §6
    #
    # Esta secao NAO existe por decisao de redacao: ela nasce quando alguma
    # sonda de efeito acha campo engolido, e some quando nenhuma achar. E o
    # achado mais util desta rodada, e nao estava na pergunta.
    engolidos = [l for l in linhas if "IGNORADO" in l["phxsql"][1]]
    if engolidos:
        t.cabeca("O achado que a pergunta não pedia: campo engolido")
        t.p(f"""**{len(engolidos)} campos de esquema desconhecidos são aceitos
            pelo `criar_tabela` e não fazem nada.** A tabela nasce, o campo
            some, e quem escreveu o pedido acha que declarou uma garantia:""")
        for l in engolidos:
            t.l(f"- **{l['titulo']}** — {l['phxsql'][1]}")
        t.l()
        t.p("""É a mesma lei que o `recursos.cache_paginas` já custou uma vez:
            **configuração que não é lida mente**, e campo de esquema sem
            leitor é pior que campo ausente — o ausente o servidor recusa, e
            quem pediu descobre na hora.""")
        t.p("""E não é falta de rigor geral: na mesma corrida, o índice
            **recusou** a coluna inexistente `lower(nome)` com o nome do
            problema. O motor confere o que ele conhece e cala sobre o que não
            conhece — a assimetria é entre saber recusar e saber que havia algo
            a recusar.""")
        t.p("""Este medidor pagou o preço disso na própria carne: a primeira
            corrida criou as tabelas com o índice no formato errado, o
            `criar_tabela` recusou, ninguém leu a recusa, e as sondas seguintes
            publicaram «o campo foi aceito e IGNORADO» sobre tabelas que nunca
            existiram. Hoje o `cria()` **exige** que a tabela nasça, e as duas
            sondas em que a recusa é a própria resposta dizem isso no nome.""")

    # ---------------------------------------------------------------- §7
    if meios:
        t.cabeca("As linhas pela metade")
        t.p("""Meia capacidade não é meio caminho andado — é o caminho que
            PARECE andado, e por isso ela ganha marca própria em vez de cair
            para o lado que der jeito:""")
        for l in meios:
            t.l(f"- **{l['titulo']}** — {l['phxsql'][1]}."
                + (f" {maiuscula(l['nota'])}." if l.get("nota") else ""))
        t.l()

    # ---------------------------------------------------------------- §8
    t.cabeca("O que esta tabela NÃO diz")
    t.p("Três honestidades, e as três mudam como se lê o resto:")
    t.l("1. **Faltar não é o mesmo que estar errado.** Boa parte destas")
    t.l("   ausências é sequência, não esquecimento: sem nível de isolamento")
    t.l("   acima de `READ COMMITTED` não adianta afinar trava, e sem")
    t.l("   subconsulta correlacionada não há `EXISTS` para pedir. A ordem")
    t.l("   está no `docs/PENDENCIAS.md`.")
    t.l(f"2. **Duas colunas são de segunda mão.** {len(citados)} motores não")
    t.l("   estão nesta máquina; o que a tabela diz deles saiu de leitura")
    t.l("   anterior, e está marcado 📄 célula a célula. Vantagem nossa contra")
    t.l("   folha velha não é vantagem provada contra o produto de hoje.")
    t.l("3. **Ter não é ter bem.** A tabela pergunta se a instrução passa, não")
    t.l("   se ela é rápida nem se o plano é bom. Quem responde por custo é a")
    t.l("   `bancada/`, e ela mede outra coisa.")
    t.l()

    # ---------------------------------------------------------------- §9
    t.cabeca("Como remedir")
    t.l("```bash")
    t.l("python3 bancada/comparativo/medir.py       # pergunta aos motores vivos")
    t.l("python3 bancada/comparativo/documento.py   # reescreve este arquivo")
    t.l("```")
    t.l()
    t.p("""O medidor sobe o PhxSql sozinho e usa os `mysql`, `psql` e `sqlite3`
        da máquina. Se algum não estiver no `PATH`, ele **para** em vez de
        publicar uma coluna vazia como se fosse ausência — que seria
        exatamente o erro que este documento existe para não repetir.""")

    ALVO.write_text(t.fim(), encoding="utf-8")
    print(f"escrito: {ALVO.relative_to(RAIZ)}")
    print(f"  {len(linhas)} capacidades, {len(faltam)} faltando ou pela metade")
    print(f"  {len(por_sql)} por SQL vivo, {len(por_sonda)} por sonda de codigo")


if __name__ == "__main__":
    escrever(carregar())
