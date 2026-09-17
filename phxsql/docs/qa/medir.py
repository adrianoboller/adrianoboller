#!/usr/bin/env python3
"""Gera a tabela das catracas do QA-PDCA. Nenhum numero se digita.

    python3 docs/qa/medir.py            imprime a tabela
    python3 docs/qa/medir.py --gravar   escreve dentro do docs/QA-PDCA.md

Por que ele existe
------------------
A tabela nasceu com `TETO` em 1.577. Noventa minutos depois, na MESMA rodada,
outra frente traduziu 28 textos e baixou a catraca para 1.549 -- e nenhuma das
duas podia ver a outra. Numero digitado nao envelhece «entre versoes» quando ha
trabalho paralelo: ele ja nasce errado.

Como ele acha as catracas, sem lista digitada
---------------------------------------------
Ele NAO tem lista. Varre `crates/*/examples/*.rs` E `bancada/**/*.py` atras de
quem imprime `catraca:`, e pergunta a cada um com `--numeros`. Cada conferidor
SE DESCREVE: o nome da constante, onde ela mora, o valor dela e o numero medido
hoje.

O `bancada/**/*.py` entrou em 16/09/2026, e o buraco que ele fecha estava
MEDIDO e escrito no `docs/CATRACAS.md`: a varredura so olhava Rust, e as
catracas que moram em Python -- as tres do `mapa-da-trava.py`, as duas do
`mapa-das-threads.py`, a do `pkill-sem-pid.py` e as sete do `trecho-vivo.py`
-- nao apareciam nesta tabela. **A tabela que existe para dizer quantas
catracas ha contava menos do que existe**, e o proprio paragrafo do
`CATRACAS.md` que denunciava isso dizia «sete» quando eram OITO: ele esquecia
a `TETO_PKILL_SEM_PID`, que e da mesma familia e mora na mesma pasta. Lei que
lista menos casos do que existem protege igual hoje e menos no dia em que
alguem usar a lista como inventario -- inclusive quando a lista e a dos
buracos.

Teto e PISO, e por que o tipo viaja na linha
--------------------------------------------
Nem toda catraca desce. O `PISO_DAS_ENTRADAS` do `trecho-vivo.py` conta as
entradas VIVAS do catalogo de guardas mais as aposentadas escritas, e existe
porque apagar uma entrada media o mesmo zero que conserta-la. Para ele,
`medido < valor` e que REPROVA.

Por isso a linha de auto-descricao carrega `tipo=teto` ou `tipo=piso`, e
**quem nao o diz continua sendo teto**: os nove conferidores em Rust nao
mudaram uma linha. Guarda nova entra pedida, nao imposta -- um campo novo que
obrigasse todo emissor antigo a mudar seria o mesmo estrago, em miniatura, que
a janela de conflito faria recusando gravacao sem versao.

E daqui saem duas coisas que uma lista digitada nao daria:

* **catraca frouxa** aparece sozinha -- valor acima do medido e folga onde uma
  regressao se esconde;
* **catraca que ninguem mede** aparece como buraco: uma constante `TETO*` no
  codigo sem conferidor que a reporte nao e catraca, e uma promessa.

E por que ele NAO le o relatorio dos conferidores
-------------------------------------------------
Porque `grep` na prosa e resolver numero por comparacao de FRASE -- a mesma
armadilha que esta casa ja proibiu para texto de tela. No dia em que alguem
melhorar a redacao do relatorio, o gerador quebraria calado e publicaria o
numero de ontem. A chave e estavel; o rotulo e livre.
"""
import re
import subprocess
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
MARCA_INICIO = "<!-- catracas:inicio -->"
MARCA_FIM = "<!-- catracas:fim -->"
MARCA_BAT_INICIO = "<!-- bateria:inicio -->"
MARCA_BAT_FIM = "<!-- bateria:fim -->"


def exemplos_que_se_descrevem():
    """Os exemplos que imprimem `catraca:` -- achados, nao listados."""
    achados = []
    for arq in sorted(RAIZ.glob("crates/*/examples/*.rs")):
        if "catraca:nome=" in arq.read_text(encoding="utf-8", errors="replace"):
            achados.append((arq.parents[1].name, arq.stem))
    return achados


def conferidores_em_python():
    """Os scripts de `bancada/` que imprimem `catraca:` -- mesmo crivo.

    Mesmo crivo de proposito: se o criterio daqui fosse outro (um nome de
    pasta, uma lista), ele divergiria do de cima na primeira pasta nova. O
    criterio e um so em toda esta funcao e na de cima -- **o conferidor se
    descreve** --, e por isso uma catraca nova em Python entra nesta tabela
    sem ninguem editar este arquivo."""
    achados = []
    for arq in sorted(RAIZ.glob("bancada/**/*.py")):
        if "__pycache__" in arq.parts:
            continue
        if "catraca:nome=" in arq.read_text(encoding="utf-8", errors="replace"):
            achados.append(arq)
    return achados


def _colher(saida, rotulo, onde):
    linhas = []
    for l in saida.splitlines():
        if l.startswith("catraca:"):
            campos = dict(p.split("=", 1) for p in l[len("catraca:"):].split(";") if "=" in p)
            campos = {k.strip(): v.strip() for k, v in campos.items()}
            campos["exemplo"] = rotulo
            campos["crate"] = onde
            linhas.append(campos)
    return linhas


def perguntar(crate, exemplo):
    r = subprocess.run(
        ["cargo", "run", "-q", "--release", "--example", exemplo, "-p", crate,
         "--", "--numeros"],
        cwd=RAIZ, capture_output=True, text=True)
    if r.returncode != 0:
        return [], f"{exemplo}: nao rodou ({r.returncode})"
    return _colher(r.stdout, exemplo, crate), None


def perguntar_python(arq):
    """Pergunta a um conferidor de `bancada/` -- e exige que ele RESPONDA.

    Um script sem `--numeros` sai calado com codigo 0, e o silencio pareceria
    «nao tem catraca». Como o crivo so o escolheu porque ele imprime
    `catraca:nome=`, silencio aqui e defeito, e sai como problema nomeado --
    conferidor que diz «limpo» sem ter conferido e o que esta casa pune."""
    rel = arq.relative_to(RAIZ)
    r = subprocess.run([sys.executable, str(arq), "--numeros"],
                       cwd=RAIZ, capture_output=True, text=True)
    if r.returncode != 0:
        return [], f"{rel}: nao rodou ({r.returncode})"
    achadas = _colher(r.stdout, arq.stem, str(rel))
    if not achadas:
        return [], (f"{rel}: escreve `catraca:nome=` e nao respondeu ao "
                    "`--numeros`")
    return achadas, None


def constantes_teto():
    """Toda `pub const TETO*` do Rust e todo `TETO*`/`PISO*` de `bancada/`.

    Serve para achar quem NAO tem conferidor: uma constante sem quem a
    reporte nao e catraca, e uma promessa.

    O lado Python entrou em 16/09/2026 e traz uma diferenca que o lado Rust
    nao precisava. Em Rust, `pub const TETO*` dentro de `src/` e sinal forte.
    Em `bancada/`, um `TETO_*` no topo de um script e sinal FRACO: das oito
    que existiam no dia, quatro eram limite de funcionamento (o prazo de uma
    sonda, o espelho de um teto de producao) e nao divida de codigo. Uma
    regua que as acusasse encheria a tabela de ruido e a faria parar de
    significar a unica coisa que ela diz.

    Entao a isencao existe, e ela mora NA LINHA DA CONSTANTE, nao numa lista
    aqui: `# nao-e-catraca: <motivo>`. E o molde do `ISENTOS` do conferidor de
    temporarios -- isencao por nome e com motivo --, com a receita saindo do
    codigo em vez de uma segunda lista neste gerador, que envelheceria
    sozinha. Uma constante nova sem marca e sem conferidor aparece como
    buraco, e alguem decide qual das duas ela e."""
    achadas = {}
    for arq in RAIZ.glob("crates/*/src/**/*.rs"):
        for n, linha in enumerate(arq.read_text(encoding="utf-8", errors="replace").splitlines(), 1):
            m = re.match(r"\s*pub const (TETO\w*)\s*:", linha)
            if m:
                achadas[m.group(1)] = f"{arq.relative_to(RAIZ)}:{n}"
    for arq in sorted(RAIZ.glob("bancada/**/*.py")):
        if "__pycache__" in arq.parts:
            continue
        for n, linha in enumerate(arq.read_text(encoding="utf-8", errors="replace").splitlines(), 1):
            m = re.match(r"((?:TETO|PISO)\w*)\s*=", linha)
            if m and "# nao-e-catraca:" not in linha:
                achadas[m.group(1)] = f"{arq.relative_to(RAIZ)}:{n}"
    return achadas


def tabela():
    linhas = [MARCA_INICIO,
              "",
              "| Catraca | Onde mora | Valor | Medido hoje | Estado |",
              "|---|---|---:|---:|---|"]
    vistos, problemas = set(), []
    colhidas = []
    for crate, exemplo in exemplos_que_se_descrevem():
        cs, erro = perguntar(crate, exemplo)
        (problemas.append(erro) if erro else colhidas.extend(cs))
    for arq in conferidores_em_python():
        cs, erro = perguntar_python(arq)
        (problemas.append(erro) if erro else colhidas.extend(cs))
    # O separador de milhar se troca SO no numero. A primeira versao fazia
    # `.replace(",", ".")` na linha inteira, e comia a virgula da frase: «em
    # cima, sem folga» saiu «em cima. sem folga». Formatacao aplicada onde nao
    # devia e a mesma familia de «rotulo se estiliza, dado nunca» -- so que
    # aqui o estrago foi na prosa.
    def fmt(n):
        return f"{n:,}".replace(",", ".")

    for c in colhidas:
        vistos.add(c["nome"])
        valor, medido = int(c["valor"]), int(c["medido"])
        # `tipo` ausente e TETO: os conferidores em Rust nao dizem o tipo e nao
        # precisam dizer -- guarda nova entra pedida, nao imposta.
        piso = c.get("tipo") == "piso"
        if medido == valor:
            estado = "em cima, sem folga"
        elif piso:
            estado = (f"**REPROVANDO** — {valor - medido} abaixo do piso"
                      if medido < valor else
                      f"**FROUXO** — {medido - valor} acima, suba-o")
        elif medido > valor:
            estado = f"**REPROVANDO** — {medido - valor} acima"
        else:
            estado = f"**FROUXA** — {valor - medido} de folga, baixe-a"
        linhas.append(
            f"| `{c['nome']}` ({c['mede']}) | `{c['onde']}` | "
            f"{'piso ' if piso else ''}{fmt(valor)} | **{fmt(medido)}** | "
            f"{estado} |")

    orfas = [(n, o) for n, o in sorted(constantes_teto().items()) if n not in vistos]
    linhas += ["",
               f"*{len(vistos)} catraca(s) medida(s) por conferidor. "
               f"Refaz com `python3 docs/qa/medir.py`.*"]
    if orfas:
        linhas += ["",
                   "**Constantes `TETO*`/`PISO*` que NENHUM conferidor reporta.** Elas não",
                   "são catracas: são limites, ou promessas. A diferença importa — catraca",
                   "sem medidor não segura nada e ainda parece que segura. Em `bancada/`,",
                   "uma constante que seja limite de funcionamento sai daqui escrevendo",
                   "`# nao-e-catraca: <motivo>` na própria linha dela:",
                   ""]
        linhas += [f"- `{n}` — `{o}`" for n, o in orfas]
    if problemas:
        linhas += ["", "**Não consegui medir:** " + "; ".join(problemas)]
    linhas.append(MARCA_FIM)
    return "\n".join(linhas)


def bateria():
    """Quantas partes a bateria tem -- PERGUNTADAS a ela, nao contadas na prosa.

    O texto de abertura deste documento dizia «25 partes» e a bateria ja tinha
    26 quando alguem foi olhar. Numero digitado a mao envelhece calado, e este
    envelheceu no mesmo lugar que ensina a nao digitar numero.

    Perguntar ao `provar.py --listar` e o mesmo principio das catracas: quem
    sabe o numero e quem o tem. Um `grep` por `parte(` na fonte contaria
    tambem as chamadas comentadas e as de dentro de um exemplo do docstring.
    """
    r = subprocess.run([sys.executable, "provar.py", "--listar"],
                       cwd=RAIZ, capture_output=True, text=True)
    m = re.search(r"^(\d+) partes:", r.stdout, re.M)
    if not m:
        return (MARCA_BAT_INICIO
                + "\n\n*nao consegui perguntar ao `provar.py --listar` "
                  "quantas partes a bateria tem.*\n\n"
                + MARCA_BAT_FIM)
    return (MARCA_BAT_INICIO
            + f"\n- `provar.py` orquestra a bateria única, hoje **{m.group(1)} "
              "partes** — o número sai de `python3 provar.py --listar`, nunca "
              "digitado aqui.\n"
            + MARCA_BAT_FIM)


def substituir(t, inicio, fim, novo, doc):
    if inicio not in t:
        print(f"nao achei {inicio} no {doc} -- ponha as duas marcas em volta "
              "do trecho e rode de novo")
        return None
    i, f = t.index(inicio), t.index(fim) + len(fim)
    return t[:i] + novo + t[f:]


def principal():
    saida, contagem = tabela(), bateria()
    if "--gravar" in sys.argv:
        doc = RAIZ / "docs/QA-PDCA.md"
        t = doc.read_text(encoding="utf-8")
        t = substituir(t, MARCA_INICIO, MARCA_FIM, saida, doc.name)
        if t is None:
            return 2
        t = substituir(t, MARCA_BAT_INICIO, MARCA_BAT_FIM, contagem, doc.name)
        if t is None:
            return 2
        doc.write_text(t, encoding="utf-8")
        print(f"tabela e contagem gravadas em {doc.relative_to(RAIZ)}")
    else:
        print(contagem)
        print()
        print(saida)
    return 0


if __name__ == "__main__":
    sys.exit(principal())
