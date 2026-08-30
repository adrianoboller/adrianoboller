# Corrigir os tres pontos desatualizados
# 29/08 06:47

import io

# --- REPLICACAO.md: a tabela da capa e o item da secao 10 ---
p='docs/REPLICACAO.md'
s=io.open(p,encoding='utf-8').read()
velho='''| Master, com a imagem no diário | 28.914 linhas/s |
| Aplicação, por réplica (as três em paralelo) | 4.357 eventos/s |
| Atraso de uma escrita até as três | 1,3 s a 2,1 s |
| Réplica derrubada: voltar a atender e alcançar 4.000 eventos | 343 ms + 1,0 s |'''
novo='''| Master, com a imagem no diário | 34.048 linhas/s |
| Aplicação, por réplica (as três em paralelo) | 17.450 eventos/s |
| Atraso de uma escrita até as três | 140 ms a 2,0 s |
| Réplica derrubada: voltar a atender e alcançar 4.000 eventos | 323 ms + 0,3 s |'''
assert s.count(velho)==1
s=s.replace(velho,novo)

velho='''- **A réplica aplica mais devagar do que o master escreve** — 4.357 eventos/s
  contra 28.914 linhas/s, com as três réplicas competindo pela mesma máquina.
  Sob carga sustentada elas ficam para trás. A razão está no caminho: aplicar
  decodifica a imagem para `Value` e **reencoda** o payload, em vez de gravar
  os bytes que vieram. Gravar o payload direto, remendando só os ponteiros dos
  anexos, é o próximo ganho grande — e é o que a seção 3 descreve.'''
novo='''- ~~A réplica aplica mais devagar do que o master escreve~~ — **este limite
  caiu, e a causa que estava escrita aqui estava errada.** Medido
  (`DESEMPENHO.md` §4.5): reencodar o payload custa 0,35 µs de 229; o que
  custava era o **source** varrendo o diário desde o começo a cada lote. Com a
  marca de posição, cada réplica aplica **17.450 eventos/s** e as três juntas
  ~52.000 — mais do que os 34.048 que o master escreve. O que continua
  verdadeiro: o atraso normal é o `reconectar_em`, e réplica não é backup.'''
assert s.count(velho)==1
s=s.replace(velho,novo)
io.open(p,'w',encoding='utf-8').write(s)
print('REPLICACAO ok')

# --- CLUSTER.md ---
p='docs/CLUSTER.md'
s=io.open(p,encoding='utf-8').read()
velho='''E há um limite medido que precisa ser dito: **a réplica aplica mais devagar do
que o master escreve** — 4.357 eventos/s contra 28.914 linhas/s. Sob carga
sustentada de escrita elas ficam para trás, e a leitura nelas fica velha. A
razão está em `docs/DESEMPENHO.md`.'''
novo='''Um limite que estava escrito aqui **caiu depois de medido**: a réplica
aplicava 4.357 eventos/s contra 28.914 do master, e hoje aplica **17.450**
contra 34.048 — as três juntas passam o master (`docs/DESEMPENHO.md` §4.5; a
causa era o source varrendo o diário a cada lote, não a réplica). Sob carga
sustentada elas acompanham; o atraso que resta é o `reconectar_em` do laço.'''
assert s.count(velho)==1
s=s.replace(velho,novo)
io.open(p,'w',encoding='utf-8').write(s)
print('CLUSTER ok')

# --- ndx.rs: o comentario da era write-through ---
p='crates/phxsql-store/src/ndx.rs'
s=io.open(p,encoding='utf-8').read()
velho='''/// # Por que a gravacao continua atravessando
///
/// Segurar pagina suja em RAM daria mais, e trocaria uma garantia por
/// desempenho **sem avisar**: hoje uma queda do PROCESSO nao atrasa o `.ndx`
/// em relacao ao `.reg`, porque o `write` ja entregou a pagina ao nucleo. So
/// uma queda da MAQUINA faz isso. A diferenca entre os dois casos e grande
/// demais para ser trocada de lado num commit de desempenho.'''
novo='''/// # A gravacao NAO atravessa mais (0.18.0)
///
/// Ate a 0.17.0 toda gravacao ia ao arquivo na hora, e este comentario
/// explicava por que: segurar pagina suja trocaria uma garantia por desempenho
/// **sem avisar**. O write-back entrou justamente quando passou a AVISAR: a
/// marca de sujo no byte 52 do cabecalho vai ao disco antes da primeira pagina
/// suja e so sai depois de todas, entao uma queda e detectada na abertura e o
/// indice recusa responder ate ser reconstruido -- barato desde a construcao
/// em lote. A troca que era inaceitavel em silencio ficou aceitavel declarada.
/// Historia completa em `docs/FORMATO.md` (a marca) e `docs/CONCORRENTES.md`.'''
assert s.count(velho)==1
s=s.replace(velho,novo)
io.open(p,'w',encoding='utf-8').write(s)
print('ndx ok')
