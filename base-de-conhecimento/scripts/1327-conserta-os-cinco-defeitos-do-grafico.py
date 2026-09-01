# Conserta os cinco defeitos do grafico
# 01/09 18:32

from pathlib import Path
p = Path("bancada/comparacao/grafico.py")
s = p.read_text(encoding="utf-8")

# --- 1. As notas dizem o que a fase FAZ, e quantas vezes. Sem isso a pagina
#        mostra «164 ms» sob o titulo «1.000.000 linhas», e quem le entende
#        que achar UMA linha custou 164 ms.
s = s.replace(
    '''FASES = [
    ("inserir", "INSERT", "gravar o milhao de linhas"),
    ("buscar", "SELECT", "achar uma linha pela chave"),
    ("atualizar", "UPDATE", "trocar o valor de uma coluna"),
    ("excluir", "DELETE", "apagar de vez"),
]''',
    '''# A nota de cada fase e um MOLDE: o numero de operacoes sai da medicao, e
# nao de um texto digitado. A do UPDATE dizia «trocar o valor de uma coluna»
# e estava errada -- as tres regravam a linha inteira, porque o `carga.rs`
# regrava, e trocar so uma coluna seria menos trabalho de um lado.
FASES = [
    ("inserir", "INSERT", "gravar {n} linhas, uma a uma"),
    ("buscar", "SELECT", "achar {ops} linhas pela chave, uma instrucao cada"),
    ("atualizar", "UPDATE", "regravar a linha inteira de {ops} delas"),
    ("excluir", "DELETE", "apagar {ops} de vez"),
]''',
)

# --- 2. A escala do painel. Ancorar no MAXIMO deixava uma rodada fora da
#        curva esmagar as tres barras: no UPDATE o eixo ia a 22,97 s por causa
#        de uma rodada, e 277 ms virava uma lasca de 3 px. Hoje o eixo e a
#        maior MEDIANA, e o bigode que passa do fim e cortado com um sinal --
#        a excursao continua dita, no rotulo, em vez de mandar no desenho.
s = s.replace(
    '''    medidos = [x for x in linhas if x[2] is not None]
    teto = max((x[4] or x[2]) for x in medidos) if medidos else 1.0
    teto = teto * 1.18 or 1.0''',
    '''    medidos = [x for x in linhas if x[2] is not None]
    teto = (max(x[2] for x in medidos) * 1.35) if medidos else 1.0''',
)

# --- 3. Vencedor so quando as faixas NAO se cruzam. No SELECT o PhxSql fez
#        164 ms e o SQLite(R) 166, com as faixas inteiramente sobrepostas
#        (151-215 contra 158-232): contornar um dos dois ali afirma uma
#        vitoria que a medida nao tem.
s = s.replace(
    '''    # Quem ganhou -- e so entre os que foram medidos. Comparar contra fase que
    # nao rodou daria vencedor por ausencia do outro.
    melhor = min(medidos, key=lambda x: x[2])[0] if medidos else None''',
    '''    # Quem ganhou -- e so entre os que foram medidos. Comparar contra fase que
    # nao rodou daria vencedor por ausencia do outro.
    #
    # E so ha vencedor quando a faixa dele NAO cruza a do segundo colocado.
    # Contornar o mais rapido de 164 ms contra 166 ms, com as duas faixas
    # sobrepostas, e publicar como resultado o que e ruido da maquina.
    melhor = None
    if len(medidos) >= 2:
        ordem = sorted(medidos, key=lambda x: x[2])
        p1, p2 = ordem[0], ordem[1]
        teto_do_1o = p1[4] if p1[4] is not None else p1[2]
        piso_do_2o = p2[3] if p2[3] is not None else p2[2]
        if teto_do_1o < piso_do_2o:
            melhor = p1[0]
    elif len(medidos) == 1:
        melhor = medidos[0][0]''',
)

# --- 4. O bigode cortado no fim do painel, e o rotulo DEPOIS dele.
#        O rotulo saia em `ESQ + w + 8`, que e o fim da BARRA -- quando o
#        bigode passava dali, a linha atravessava o texto.
s = s.replace(
    '''        if mn is not None and mx is not None and mx > mn:
            x1 = ESQ + util * mn / teto
            x2 = ESQ + util * mx / teto
            ym = y + alt_barra / 2
            p.append(
                f'<line x1="{x1:.1f}" y1="{ym}" x2="{x2:.1f}" y2="{ym}" class="bigode"/>'
                f'<line x1="{x1:.1f}" y1="{ym - 6}" x2="{x1:.1f}" y2="{ym + 6}" class="bigode"/>'
                f'<line x1="{x2:.1f}" y1="{ym - 6}" x2="{x2:.1f}" y2="{ym + 6}" class="bigode"/>'
            )
        p.append(f'<text x="{ESQ + w + 8:.1f}" y="{y + 22}" class="valor">{fmt(med)}</text>')''',
    '''        fim_do_rotulo = ESQ + w
        if mn is not None and mx is not None and mx > mn:
            x1 = min(ESQ + util * mn / teto, ESQ + util)
            estourou = mx > teto
            x2 = ESQ + util if estourou else ESQ + util * mx / teto
            ym = y + alt_barra / 2
            p.append(
                f'<line x1="{x1:.1f}" y1="{ym}" x2="{x2:.1f}" y2="{ym}" class="bigode"/>'
                f'<line x1="{x1:.1f}" y1="{ym - 6}" x2="{x1:.1f}" y2="{ym + 6}" class="bigode"/>'
            )
            if estourou:
                # Seta em vez de traco: o bigode nao termina ali, ele foi
                # CORTADO. Traco no fim leria como «o maximo e este».
                p.append(
                    f'<path d="M{x2 - 7:.1f},{ym - 6} L{x2:.1f},{ym} '
                    f'L{x2 - 7:.1f},{ym + 6}" class="bigode" fill="none"/>'
                )
            else:
                p.append(
                    f'<line x1="{x2:.1f}" y1="{ym - 6}" x2="{x2:.1f}" y2="{ym + 6}"'
                    f' class="bigode"/>'
                )
            fim_do_rotulo = max(fim_do_rotulo, x2)
        # O rotulo vem DEPOIS do bigode, nunca por cima dele.
        rotulo_valor = fmt(med)
        if mx is not None and mx > teto:
            rotulo_valor += f" (pico {fmt(mx)})"
        p.append(
            f'<text x="{fim_do_rotulo + 8:.1f}" y="{y + 22}" class="valor">'
            f'{rotulo_valor}</text>'
        )''',
)

# --- 5. A nota vem montada de fora, com os numeros da medicao dentro.
s = s.replace(
    "    paineis = \"\\n\".join(painel(c, t, nt, fases.get(c)) for c, t, nt in FASES)",
    '''    ops = d.get("operacoes_por_fase_pontual", 0)
    paineis = "\\n".join(
        painel(c, t, nt.format(n=mil(n), ops=mil(ops)), fases.get(c))
        for c, t, nt in FASES
    )''',
)
s = s.replace(
    '''def numero(v, casas):''',
    '''def mil(x):
    return f"{x:,}".replace(",", ".")


def numero(v, casas):''',
)

# --- 6. O subtitulo mentia duas vezes: dizia «1.000.000 linhas» como se
#        valesse para as quatro fases, e prometia contorno no mais rapido
#        mesmo quando as faixas se cruzam.
s = s.replace(
    '''<p class="sub">{n:,} linhas &middot; menor &eacute; melhor &middot; o bigode vai do
m&iacute;nimo ao m&aacute;ximo das rodadas &middot; contorno marca o mais r&aacute;pido da fase</p>''',
    '''<p class="sub">tabela de {n:,} linhas &middot; menor &eacute; melhor &middot; cada painel
tem escala pr&oacute;pria &middot; o bigode vai do m&iacute;nimo ao m&aacute;ximo das rodadas, e a
seta diz que ele foi cortado no fim do painel &middot; o contorno s&oacute; marca vencedor
quando as faixas <em>n&atilde;o</em> se cruzam</p>''',
)
p.write_text(s, encoding="utf-8")
print("grafico.py: cinco consertos")
