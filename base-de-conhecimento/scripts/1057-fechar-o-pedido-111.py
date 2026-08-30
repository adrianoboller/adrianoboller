# Fechar o pedido 111
# 29/08 03:56

import io
p='docs/PENDENCIAS.md'
s=io.open(p,encoding='utf-8').read()
m=[l for l in s.split('\n') if l.startswith('| ◐ | 111 |')]
assert len(m)==1
nova = ('| ☑️ | 111 | **A réplica acompanhar a escrita do master** | **acompanha: 4.273 → 17.450 '
        'eventos/s por réplica (4,08×)**, e as três juntas aplicam ~52.000/s contra 34.048 que o '
        'master escreve. O alcance de 100.000 eventos caiu de 18,7 s para **5,7 s**. E a causa '
        'registrada aqui estava **errada**: acusava `aplicar` de reencodar o payload, e isso custa '
        '**0,27 µs** — `aplicar_evento` são 16,15 µs contra 15,88 de uma inserção local. Os 229 µs '
        'por evento estavam **no source**: servir «500 eventos a partir de P» varria os P '
        'anteriores lendo o cabeçalho de cada um, e alcançar 100.000 custava **4,07 s só ali** '
        '(`--example custo-do-desde`); a marca de posição levou a **0,09 s, 45×**. Mais o laço, que '
        'dormia depois de **toda** rodada e não só das vazias, e o `bytes_para_hex`, que fazia um '
        '`format!` por byte |')
s=s.replace(m[0], nova)
s=s.replace('**112 feitos · 8 parciais · 9 planejados**, de 129 pedidos.',
            '**113 feitos · 7 parciais · 9 planejados**, de 129 pedidos.')
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
