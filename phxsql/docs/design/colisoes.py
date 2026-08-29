"""Procura COLISAO DE NOME DE CLASSE na folha do Centro de Controle.

O padrao que interessa: a mesma classe usada como sujeito principal de dois
blocos diferentes -- dois componentes com o mesmo nome. O segundo ganha, e o
primeiro passa a ter propriedades que ninguem escreveu para ele. Foi assim com
`.ferramentas` (barra da moldura x fila de filtros) e com `.modo` (bloco de
texto da entrada x cartao de escolha do dialogo).

Nao conta como colisao a variacao do MESMO componente (`.botao` e
`.botao.mini`), nem descendente (`.criar label`): so o seletor cujo ultimo
elemento composto e exatamente `.classe`, possivelmente com pseudo-classe.
"""
import re
import sys

CAMINHO = sys.argv[1] if len(sys.argv) > 1 else "index.html"
s = open(CAMINHO, encoding="utf-8").read()
css = s[s.index("<style>") + 7: s.index("</style>")]

# Fora os comentarios, que trazem chaves e pontos e confundiriam o parser.
css = re.sub(r"/\*.*?\*/", "", css, flags=re.S)

blocos = []          # (seletor, corpo, linha)
profundidade = 0
atual = ""
inicio = 0
i = 0
linha_de = lambda pos: css[:pos].count("\n") + 1

# Um varredor simples de chaves, que pula @media sem se perder.
pilha = []
buf = ""
for i, ch in enumerate(css):
    if ch == "{":
        pilha.append((buf.strip(), i))
        buf = ""
    elif ch == "}":
        if pilha:
            sel, pos = pilha.pop()
            if not sel.startswith("@"):
                fim = i
                corpo = css[pos + 1:fim]
                # So blocos de declaracao (sem bloco aninhado dentro).
                if "{" not in corpo:
                    # Dentro de @media? Regra de viewport nao e colisao de
                    # componente: e o MESMO componente noutra largura, e
                    # sobrescrever ali e o proposito da media query.
                    em_media = any(s.startswith("@") for s, _ in pilha)
                    blocos.append((sel, corpo.strip(), linha_de(pos), em_media))
        buf = ""
    else:
        buf += ch

# Para cada seletor simples, qual a classe "sujeito" (ultimo elemento composto)?
ALVO = re.compile(r"^\.([A-Za-z0-9_-]+)(:[A-Za-z-]+(\([^)]*\))?)*$")

por_classe = {}
for sel, corpo, ln, em_media in blocos:
    if em_media:
        continue
    for parte in sel.split(","):
        parte = parte.strip()
        if not parte:
            continue
        ultimo = re.split(r"\s+|>|\+|~", parte)[-1]
        m = ALVO.match(ultimo)
        if not m:
            continue
        # Um seletor com ancestral (`.form-dbl .cmp`) e escopado de proposito:
        # esse e justamente o conserto, nao a doenca.
        temAncestral = len(re.split(r"\s+|>|\+|~", parte)) > 1
        classe = m.group(1)
        por_classe.setdefault(classe, []).append(
            (parte, corpo, ln, temAncestral))

PROP = re.compile(r"^\s*([a-z-]+)\s*:", re.M)


def props(corpo):
    return set(PROP.findall(corpo))


print("COLISOES DE NOME DE CLASSE\n" + "=" * 60)
achou = 0
for classe, usos in sorted(por_classe.items()):
    # So os blocos NAO escopados: dois componentes reivindicando o nome cru.
    crus = [u for u in usos if not u[3]]
    if len(crus) < 2:
        continue
    # Sao mesmo componentes diferentes? Se os conjuntos de propriedades se
    # sobrepoem numa propriedade de LAYOUT, um esta desmanchando o outro.
    layout = {"display", "position", "grid-area", "flex-direction", "gap",
              "flex-wrap", "grid-template-columns", "padding", "margin",
              "margin-bottom", "width", "overflow", "overflow-x", "cursor",
              "text-transform", "align-items"}
    conjuntos = [props(c[1]) for c in crus]
    briga = set()
    for a in range(len(conjuntos)):
        for b in range(a + 1, len(conjuntos)):
            briga |= (conjuntos[a] & conjuntos[b] & layout)
    # Reporta TODO nome com dois blocos crus: a heuristica de propriedade
    # deixou passar o `.modo`, e nome repetido ja merece olhada humana.
    achou += 1
    print("\n.%s  -- %d blocos crus%s"
          % (classe, len(crus),
             ("  BRIGAM EM: " + ", ".join(sorted(briga))) if briga else ""))
    for sel, corpo, ln, _ in crus:
        resumo = " ".join(corpo.split())[:90]
        print("   linha %-5d %-26s %s" % (ln, sel, resumo))

print("\n%d colisao(oes)." % achou)
