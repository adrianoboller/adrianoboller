# Registrar no DESEMPENHO e reajustar os numeros
# 29/08 03:57

import io
p='docs/DESEMPENHO.md'
s=io.open(p,encoding='utf-8').read()
anc = '## 5. Por que LSM não cabe dentro do motor atual'
assert s.count(anc)==1
novo = '''## 4.5 A réplica: a causa registrada estava errada

Estava escrito em dois documentos que a réplica ficava para trás porque
«aplicar decodifica a imagem para `Value` e **reencoda** o payload, em vez de
gravar os bytes que vieram». Com o lote da B+tree pronto, o item virou o
próximo da fila — e a primeira coisa foi medir a acusação.

`--example onde-doi-na-replica`, 20.000 eventos, sem rede no meio:

| | µs/evento |
|---|---:|
| hexadecimal da imagem, no source | 3,48 |
| montar o JSON do lote | 1,21 |
| analisar o JSON, na réplica | 2,44 |
| hexadecimal da imagem, na réplica | 0,62 |
| `aplicar_evento` (decodifica + insere) | 16,15 |
| **o caminho todo, sem rede** | **23,90** |
| uma inserção local pura, para comparar | 15,80 |

`aplicar_evento` custa **16,15 µs** e uma inserção local custa **15,80**. A
acusação vale **0,35 µs** — e a réplica media **229 µs por evento**. Os outros
205 nunca estiveram nesse caminho.

### Onde eles estavam

**1. O source varria o diário desde o começo a cada lote.** Desde que o evento
deixou de ter largura fixa, chegar ao evento N é caminhar pelos N−1 anteriores
lendo o cabeçalho de cada um. `--example custo-do-desde`, diário de 100.000:

| P | ler 500 a partir de P | por evento |
|---:|---:|---:|
| 0 | 0,56 ms | 1,11 µs |
| 50.000 | 20,36 ms | 40,72 µs |
| 90.000 | 36,32 ms | **72,65 µs** |

Perfeitamente linear em P — e o total, quadrático. Alcançar os 100.000 de 500
em 500 gastava **4,07 s só do lado de quem serve**, ou 40,7 µs por evento
entregue, com três réplicas fazendo isso ao mesmo tempo sob a trava global do
master.

Com uma **marca de posição**, **0,09 s: 45×**. Ela é uma *dica*: uma errada faz
a leitura começar no lugar errado e o CRC do evento recusar, ou cair depois do
`fim` e devolver vazio. Nenhum dos dois entrega evento errado.

Ela mora no **servidor**, e não na tabela, porque a tabela é aberta e fechada a
cada pedido — e são pedidos seguidos que ela serve. E são **várias por tabela**:
um source atende réplicas em posições diferentes, e uma marca só seria empurrada
para frente pela mais adiantada e nunca serviria às outras. Foi essa correção
que trouxe o número de 7.835 para 17.450.

**2. O laço dormia depois de toda rodada.** O `reconectar_em` é o intervalo
entre perguntas **em vão**; dormir depois de uma rodada que aplicou eventos é
dormir enquanto o source escreve. Erro continua dormindo, de propósito.

**3.** E `bytes_para_hex` fazia um `format!` — e uma alocação de `String` — **por
byte** da imagem: 3,48 → 0,24 µs por evento, **14,5×**.

### O resultado, na bancada dos quatro servidores

| | antes | agora |
|---|---:|---:|
| master, com a imagem no diário | 28.914 linhas/s | 34.048 |
| **aplicação, por réplica (as três em paralelo)** | **4.273 ev/s** | **17.450** |
| alcançar 100.000 eventos | 18,7 s | **5,7 s** |
| exclusão física até as três | 1.952 ms | **140 ms** |
| réplica derrubada: alcançar 4.000 eventos | 1,0 s | 0,3 s |

**4,08×.** As três juntas aplicam ~52.000 eventos/s contra os 34.048 que o
master escreve — que era o pedido.

> A lição repete a do Profiler, e por isso vale escrevê-la de novo: o
> diagnóstico plausível sobrevive porque ninguém o mede. Aqui ele apontava para
> o lado errado do fio.

---

'''
io.open(p,'w',encoding='utf-8').write(s.replace(anc, novo+anc))
print('ok')
