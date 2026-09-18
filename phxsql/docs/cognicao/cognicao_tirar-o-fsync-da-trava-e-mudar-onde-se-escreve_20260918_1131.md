# Tirar o `fsync` de dentro da trava não é mexer no `fsync` — é mexer em ONDE se escreve

Pedido 252, decisão do dono de 18/09/2026. Frente B (engenheiro).

## 1. O que aconteceu

A catraca `alcancam-fsync` do `bancada/concorrencia/mapa-da-trava.py` estava
vermelha desde 08/09: **25 seções, teto 22**. A seção que a decisão do dono
mandou atacar era a reaplicação do diário do PITR — `reaplicar_diario_ate`, em
`crates/phxsql-server/src/servidor.rs` —, que sincronizava cada tabela
reaplicada com a trava global de dados na mão.

O pedido dizia, e a ficha dele registra a frase: *«a restauração até um
instante **precisa** segurar a trava enquanto reaplica — soltar deixaria
leitores vendo meio banco»*. Medido, é o contrário: **a janela de "meio banco"
já existia**, e é a mesma coisa que obrigava o `fsync` a ficar lá dentro.

## 2. O que eu concluí primeiro, e estava errado

Duas coisas, e as duas compilavam.

**(a) «Basta soltar a ficha antes do `fsync`.»** O `Table` desta casa não tem
amarra de vida com o guard da trava (`Exclusiva::sem_amarra`: a ficha é
convenção, não tipo). Então dá para abrir a tabela com a trava na mão, soltar a
trava e chamar `sincronizar()` depois — **compila, passa nos doze testes do
PITR, e a catraca fica verde**. E é corrupção: dali em diante a tabela
restaurada já está na raiz de dados, qualquer sessão pode abri-la, e eu estaria
descarregando páginas sujas de um `Table` obsoleto por cima do trabalho dela.
*O conserto que funciona pelo motivo errado sobrevive melhor que o defeito.*

**(b) «A ordem confirmar → reaplicar é detalhe de implementação.»** Não é: é a
**causa**. Enquanto o destino da escrita é um database que já entrou na raiz,
nenhum `fsync` dali pode sair da trava — porque a partir do `rename` há segundo
dono possível. O que libera o `fsync` não é onde o `drop` está; é o destino da
escrita não ser alcançável por ninguém.

## 3. O que a medição disse

| | antes | depois |
|---|---|---|
| `alcancam-fsync` (mapa da trava, 86 seções) | **25** | **24** |
| primeiro tamanho do `.reg` restaurado visto por um terceiro | **570 B** | **17.912 B** (= o final) |
| a mesma prova, com o defeito reposto | pega em **10 de 10** corridas | passa em **10 de 10** |
| `bancada/pitr/provar.py` (22 conferências pelo soquete) | 0 falhas | **0 falhas** |

E a conta do que sobra fecha sem sobra: **24 − 22 = 2**, que são exatamente
`empilhar` e `empilhar_atualizar_com_cascata` — as duas que o próprio 252 já
tinha medido como *não sendo código*: elas entraram quando `abrir_travada`
(34 chamadas, `HERDADO 18/86`) virou `abrir_travada_sem_sobrepor` (2 seções,
abaixo do corte de 17) para o par da transação. Envoltório compartilhado que
vira privado muda o veredito da catraca sem o código mudar.

**A tentação medida e recusada:** juntar as duas portas num nome só devolveria
a catraca a 22 na hora — e seria trocar um nome para trocar um veredito, com
zero mudança no que a trava segura. Não se faz.

## 4. A regra

**Quando mandarem tirar trabalho caro de dentro de uma trava, procure o DESTINO
da escrita antes de procurar o ponto do `drop`.** Soltar a ficha só é seguro
onde o que se escreve não tem segundo dono possível; onde tem, o `drop` não é
melhoria, é corrupção que compila.

E o corolário, que é o que surpreendeu: **mover a escrita para fora do alcance
dos outros costuma deixar a garantia MAIS forte, não mais fraca.** Aqui fechou
de graça a janela em que outra sessão via o banco no instante da cópia, e fez
erro duro na reaplicação parar de deixar um database criado ao lado de uma
resposta de fracasso.

## 5. Como está guardado hoje

Três guardas, e as três provadas nos dois sentidos:

- `o_fsync_da_restauracao_fica_fora_da_trava` — catraca do fonte: o `drop` da
  ficha vem acima de todo `sincronizar` no corpo do `reaplicar_diario_ate`, e
  **tem de haver** um `sincronizar` (restauração que não sincroniza devolve um
  database que a próxima queda de energia leva junto). Com o defeito reposto:
  falha nomeando as linhas; e o mapa volta a medir 25.
- `o_pitr_reaplica_antes_de_o_database_entrar_na_raiz` — a metade que torna a
  de cima segura. Com o defeito reposto: falha.
- `o_restaurado_so_aparece_com_o_diario_ja_reaplicado` — comportamento, com um
  vigia em outra *thread*. É o único que um leitor de fora poderia escrever, e
  ele mede **tamanho**, não veredito.

O buraco que fica, dito: **a catraca continua vermelha**, 24 contra teto 22, e
o que falta são as duas seções do corte da porta comum — que não se consertam
com código e não se consertam subindo teto. Elas são a metade (1) que voltou
para a mesa do dono, agora com o alvo dele já resolvido por mérito.
