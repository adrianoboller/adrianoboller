# Rotulo entra na barra quando nao cabe fora
# 01/09 18:34

from pathlib import Path
p = Path("bancada/comparacao/grafico.py")
s = p.read_text(encoding="utf-8")

# O rotulo depois do bigode resolveu a sobreposicao e criou a truncagem: com o
# bigode cortado na borda nao sobra painel para o texto, e «12,3 s (pico 17,1
# s)» saia como «12». Agora ele so vai para fora se COUBER; se nao couber,
# entra na barra, alinhado a direita e em branco.
s = s.replace(
    '''        # O rotulo vem DEPOIS do bigode, nunca por cima dele.
        rotulo_valor = fmt(med)
        if mx is not None and mx > teto:
            rotulo_valor += f" (pico {fmt(mx)})"
        p.append(
            f'<text x="{fim_do_rotulo + 8:.1f}" y="{y + 22}" class="valor">'
            f'{rotulo_valor}</text>'
        )''',
    '''        # O rotulo vem DEPOIS do bigode, nunca por cima dele -- e se nao
        # couber ali, vem DENTRO da barra, alinhado a direita. A primeira
        # versao so empurrava para a direita, e o texto saia cortado pela
        # borda do painel justamente nas linhas mais interessantes.
        rotulo_valor = fmt(med)
        if mx is not None and mx > teto:
            rotulo_valor += f" (pico {fmt(mx)})"
        largura = len(rotulo_valor) * 7.3          # 13px semibold, medido no desenho
        if fim_do_rotulo + 8 + largura <= LARG - 2:
            p.append(
                f'<text x="{fim_do_rotulo + 8:.1f}" y="{y + 22}" class="valor">'
                f'{rotulo_valor}</text>'
            )
        else:
            p.append(
                f'<text x="{ESQ + w - 8:.1f}" y="{y + 22}" class="valor dentro"'
                f' text-anchor="end">{rotulo_valor}</text>'
            )''',
)

# O painel sobrava quase 100 px de altura vazia embaixo das tres barras.
s = s.replace("LARG, ALT = 460, 300", "LARG, ALT = 460, 224")

# O texto dentro da barra precisa contrastar com o cheio da cor da marca.
s = s.replace(
    """.valor{{font:600 13px system-ui;fill:var(--tinta);font-variant-numeric:tabular-nums}}""",
    """.valor{{font:600 13px system-ui;fill:var(--tinta);font-variant-numeric:tabular-nums}}
.valor.dentro{{fill:#fff}}""",
)
p.write_text(s, encoding="utf-8")
print("grafico.py: rotulo que nao cabe entra na barra")
