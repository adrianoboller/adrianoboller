def lum(h):
    h = h.lstrip('#')
    r, g, b = [int(h[i:i+2], 16) / 255 for i in (0, 2, 4)]
    f = lambda v: v / 12.92 if v <= 0.03928 else ((v + 0.055) / 1.055) ** 2.4
    return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b)


def cr(a, b):
    l1, l2 = lum(a), lum(b)
    return round((max(l1, l2) + 0.05) / (min(l1, l2) + 0.05), 2)


claro_bg = {'fundo': '#f7f5f2', 'painel': '#ffffff',
            'painel-2': '#f2efeb', 'realce': '#e9e4de'}
escuro_bg = {'fundo': '#010418', 'painel': '#0a1122',
             'painel-2': '#0f182c', 'realce': '#152238'}

print("=== TEMA CLARO - texto-3 #7a6d66 (atual) ===")
for k, v in claro_bg.items():
    print("  sobre %-9s %s: %s" % (k, v, cr('#7a6d66', v)))

print("  candidatos:")
for c in ['#6f625b', '#6b5e57', '#685c55', '#665a53', '#63574f', '#605448']:
    print("    %s: painel-2=%s realce=%s fundo=%s painel=%s"
          % (c, cr(c, '#f2efeb'), cr(c, '#e9e4de'),
             cr(c, '#f7f5f2'), cr(c, '#ffffff')))

print()
print("=== TEMA ESCURO - texto-3 #7c8598 (atual) ===")
for k, v in escuro_bg.items():
    print("  sobre %-9s %s: %s" % (k, v, cr('#7c8598', v)))

print()
print("=== texto-2 ===")
print("  claro  #4a3f3a painel-2=%s realce=%s"
      % (cr('#4a3f3a', '#f2efeb'), cr('#4a3f3a', '#e9e4de')))
print("  escuro #a8b0c0 painel-2=%s realce=%s"
      % (cr('#a8b0c0', '#0f182c'), cr('#a8b0c0', '#152238')))

print()
print("=== cores de acao no tema claro sobre painel-2 #f2efeb ===")
for nome, c in [('incluir', '#2f7a3e'), ('alterar', '#8a6a1f'),
                ('excluir', '#b71414'), ('marcar', '#b5257f'),
                ('consultar', '#1f5c93'), ('laranja', '#c63c0a')]:
    print("  %-10s %s: painel-2=%s fundo=%s painel=%s"
          % (nome, c, cr(c, '#f2efeb'), cr(c, '#f7f5f2'), cr(c, '#ffffff')))

print()
print("=== cores de acao no tema escuro sobre painel-2 #0f182c ===")
for nome, c in [('incluir', '#6cc98c'), ('alterar', '#ffc43d'),
                ('excluir', '#ff5f5f'), ('marcar', '#ff8fc7'),
                ('consultar', '#5fa6e8'), ('laranja', '#ff8a1c')]:
    print("  %-10s %s: painel-2=%s fundo=%s painel=%s"
          % (nome, c, cr(c, '#0f182c'), cr(c, '#010418'), cr(c, '#0a1122')))
