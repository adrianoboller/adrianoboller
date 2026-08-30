# Acrescentar o pedido 129 e reordenar o 128
# 29/08 03:09

import io
p='docs/PENDENCIAS.md'
linhas=io.open(p,encoding='utf-8').read().split('\n')

# A ordem e a dos pedidos: 128 veio depois de 127, e estava antes.
i128=[i for i,l in enumerate(linhas) if l.startswith('| ☑️ | 128 |')][0]
i127=[i for i,l in enumerate(linhas) if l.startswith('| ☐ | 127 |')][0]
assert i128 < i127
l128=linhas.pop(i128)
i127-=1
linhas.insert(i127+1, l128)

nova = ('| ☑️ | 129 | **O motor SQL tem de conhecer o `BULKINSERT`; e o prazo, no '
        '`config.json` e na tela** | o prazo já era `recursos.carga_prazo_min` '
        '(padrão 30 min) desde o 128; entrou a **tela de configuração explicando '
        'cada ajuste** — com a seção «Cargas em andamento» listando quem reservou '
        'o quê — e o **`docs/SQL.md`**, que diz o que a camada SQL precisa saber '
        'antes de existir. `BULKINSERT` não é açúcar sintático: é palavra '
        'reservada, vale para a **sessão** (um driver que multiplexa conexões '
        'quebra a exclusividade sem avisar) e o `EM_CARGA` tem de virar '
        '*serialization failure* no SQLSTATE, não *access denied*. E a frase que '
        'o documento repete alto: **não é transação** — ele reserva a tabela, não '
        'desfaz nada |')
linhas.insert(i127+2, nova)

texto='\n'.join(linhas)
texto=texto.replace('**110 feitos · 8 parciais · 10 planejados**, de 128 pedidos.',
                    '**111 feitos · 8 parciais · 10 planejados**, de 129 pedidos.')
io.open(p,'w',encoding='utf-8').write(texto)
print('ok')
