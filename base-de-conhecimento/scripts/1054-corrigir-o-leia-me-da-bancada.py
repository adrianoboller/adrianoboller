# Corrigir o LEIA-ME da bancada
# 29/08 03:55

import io
p='bancada/replicacao/LEIA-ME.md'
s=io.open(p,encoding='utf-8').read()
velho = """| | |
|---|---|
| Master, com a imagem no diário | 18.773 linhas/s |
| Aplicação, por réplica (as três em paralelo) | 4.273 eventos/s |
| Atraso de uma escrita até as três | 1,3 s a 2,1 s |
| Réplica derrubada: voltar a atender | 343 ms |
| Réplica derrubada: alcançar 4.000 eventos | 1,0 s |
| Retrato das quatro, no fim | idênticos |

O atraso é dominado pelo `reconectar_em` (2 s aqui), e não pelo trabalho: é
quanto tempo a réplica dorme entre uma pergunta e outra. Baixar o intervalo
baixa o atraso e sobe o tráfego de perguntas em vão.

**A réplica aplica mais devagar do que o master escreve** — 4.273/s contra
18.773/s. Sob carga sustentada as réplicas ficam para trás. A razão está no
caminho: aplicar decodifica a imagem para `Value` e **reencoda** o payload, em
vez de gravar os bytes que vieram. Está anotado em `docs/PENDENCIAS.md`."""
novo = """| | |
|---|---|
| Master, com a imagem no diário | 34.048 linhas/s |
| Aplicação, por réplica (as três em paralelo) | **17.450 eventos/s** |
| Alcançar 100.000 eventos, as três | 5,7 s |
| Atraso de uma escrita até as três | 140 ms a 2,0 s |
| Réplica derrubada: voltar a atender | 323 ms |
| Réplica derrubada: alcançar 4.000 eventos | 0,3 s |
| Retrato das quatro, no fim | idênticos |

O atraso ainda é dominado pelo `reconectar_em` (2 s aqui) quando a escrita cai
logo depois de a réplica adormecer — é por isso que a mesma coluna traz 140 ms
e 2,0 s. Baixar o intervalo baixa o atraso e sobe o tráfego de perguntas em vão.

### O que estava escrito aqui, e estava errado

Esta seção dizia: «a réplica aplica mais devagar do que o master escreve — a
razão está no caminho: aplicar decodifica a imagem para `Value` e **reencoda**
o payload». Medido, a acusação não se sustenta: `aplicar_evento` custa
**16,15 µs** e uma inserção local pura custa **15,88 µs**
(`--example onde-doi-na-replica`). Decodificar e reencodar custam **0,27 µs**.

Os 4.273 eventos/s eram **229 µs por evento**, e o caminho de CPU inteiro dos
dois lados custa 20,5 µs. Os outros 208 estavam em dois lugares, nenhum deles
na réplica:

1. **O source varria o diário desde o começo a cada lote.** Servir «500 eventos
   a partir de P» caminhava pelos P anteriores lendo o cabeçalho de cada um —
   alcançar 100.000 em lotes de 500 custava **4,07 s só do lado de quem serve**
   (`--example custo-do-desde`). Com a marca de posição, **0,09 s: 45×**.
2. **O laço dormia depois de toda rodada, inclusive das produtivas.** O
   `reconectar_em` é o intervalo entre perguntas **em vão**; uma rodada que
   aplicou eventos volta na hora.

E um terceiro, menor: `bytes_para_hex` fazia um `format!` — e uma alocação de
`String` — **por byte** da imagem. Tabela de dígitos no lugar: 3,48 → 0,24 µs
por evento, **14,5×**.

**4.273 → 17.450 eventos/s por réplica: 4,08×**, e o alcance de 100.000 eventos
caiu de 18,7 s para 5,7 s. Em conjunto as três aplicam ~52.000 eventos/s, mais
do que o master escreve — o que era o pedido 111."""
assert s.count(velho)==1
io.open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print('ok')
