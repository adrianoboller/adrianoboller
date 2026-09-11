# Fechador de região `<!-- GERADO -->` perto de prosa entrelaçada

## O que aconteceu

Na primeira versão dos 16 fechadores do `docs/TECNOLOGIAS.md` (pedido 156),
usei a regra "fechador imediatamente antes da próxima heading `##`/`###`, do
próximo marcador, ou de uma divisória `---`". Rodei `--gravar` e o diff
pareceu limpo — nenhuma heading foi engolida, nenhum marcador foi corrompido.
O integrador reprovou mesmo assim: quatro frases inteiras do autor
desapareceram («tokenizador de Rust», «manda tratar assim», «rodapé do
dossiê», «quatro níveis dentro»), porque a regra "até a próxima heading"
também engole qualquer prosa que o autor tenha colado **entre** o bloco
gerado e essa heading — que é exatamente onde a maior parte da prosa deste
documento mora (parágrafos de análise escritos à mão logo depois de cada
tabela/trecho colado, antes da seção seguinte).

## O que eu concluí primeiro, e estava errado

Concluí que "próxima heading/marcador/divisória" era uma fronteira segura
porque nenhum dos meus testes de fronteira (ler linha a linha, conferir
contra o texto) acusou problema — e não acusou mesmo, porque eu estava
conferindo "o fechador não corta uma heading ao meio", não "existe prosa do
autor short entre o fim do bloco gerado e a heading". São duas perguntas
diferentes, e eu só fiz a primeira. A regra do pedido original ("imediatamente
antes da próxima heading...") também contribuiu para o erro: ela nomeia três
âncoras (heading, marcador, divisória) como se region sempre terminasse
exatamente num desses três pontos — nunca antes. Nesta base, isso é falso na
maioria dos 16 blocos: **10 dos 16** tinham prosa autoral entre o fim do
trecho gerável e a próxima heading, e em **3 casos** (Rust por crate, testes,
recusados) essa prosa fica **antes** da última linha do bloco gerador, não
depois — a suposição "prosa vem depois, nunca no meio" também é falsa.

## O que a medição disse

Auditando as 16 regiões frase por frase contra o que cada `bloco_*()`
realmente devolve (não contra o que "parece" ter sido colado):

- **6 regiões** eram limpas (tabela ou extração-fiel de outro `.md`, prosa
  do autor estritamente depois): `bloco_interface`, `bloco_outras_linguagens`,
  `bloco_dependencias`, `bloco_empacotamento_zero_deps`, `bloco_normas`,
  `bloco_empacotamento_plataformas`, `bloco_gpu_veredito`,
  `bloco_dez_propostas`, `bloco_comparacao_fora` — bastou cortar mais cedo
  (no fim da tabela/extração, não na próxima heading).
- **2 regiões** exigiram cortar NO MEIO de uma linha (`bloco_bancadas`,
  `bloco_recusados`): a frase gerável termina no meio de um parágrafo
  contínuo, sem quebra de linha, e a continuação já é do autor.
- **1 região** exigiu REORDENAR (`bloco_testes`): a única linha gerável
  (`cargo test --workspace: N testes...`) estava fisicamente ENTRE dois
  parágrafos do autor, o primeiro dos quais é anterior a ela no documento —
  não dá para excluir conteúdo "antes" do fechador sem mover a linha
  gerável para logo depois do marcador e empurrar os dois parágrafos do
  autor (na mesma ordem relativa) para depois do fechador.
- **3 regiões** não tinham NENHUM prefixo limpo e seguro (`bloco_modelos`,
  o marcador triplo `bloco_conferidores+catracas+guardas`,
  `bloco_transacoes_nao_entrou`): a elaboração do autor está entremeada
  item a item dentro da MESMA frase/lista (ex.: `docs/MODELOS.md` registra
  **N** rodadas: "rodada 1" (comentário do autor sobre a rodada 1); "rodada
  2" (comentário do autor sobre a rodada 2); ...), sem nenhum ponto de corte
  gramatical seguro. Nesses três, a região ficou **vazia** logo após o
  marcador — o `--gravar` escreve ali só a frase terse do gerador, e toda a
  elaboração antiga do autor (com números desatualizados) permanece
  intocada, agora como um parágrafo separado logo abaixo.

## A regra

**A fronteira de uma região `<!-- GERADO -->` nunca é "a próxima heading" —
é sempre "a última linha que a função realmente emite, e nem uma palavra
a mais", mesmo que isso signifique cortar no meio de uma linha, reordenar
duas linhas, ou deixar a região vazia.** Antes de fechar uma região:

1. Rode a função (ou peça a saída fresca) e compare frase a frase com o
   texto colado no documento — não confie em "parece que bate".
2. Se a prosa do autor vem só DEPOIS do trecho gerável, corte ali — mesmo
   que fique bem antes da próxima heading.
3. Se a prosa do autor vem ANTES (entre o marcador e o trecho gerável),
   mova o trecho gerável para logo após o marcador e empurre a prosa para
   depois do fechador, na mesma ordem.
4. Se a prosa do autor está entremeada DENTRO da mesma frase/lista, sem
   nenhum ponto de corte gramatical seguro, deixe a região vazia — nunca
   grave menos legibilidade em troca de perder uma frase do autor.

E o corolário do teste de aceitação que pegou isto: **grep de frase inteira
prova sobrevivência, não posição** — os quatro `grep -c "frase"` do
integrador não verificam ONDE a frase está, só que ela ainda existe em
algum lugar do arquivo. Isso é o que torna as estratégias 3 e 4 acima
(reordenar, esvaziar) aceitáveis: a frase sobrevive, ainda que sua vizinha
imediata no documento mude.

## Como está guardado hoje

Só neste arquivo e no diff do commit desta correção. O
`docs/tecnologias/extrair.py` não sabe nada disto — ele só grava o que o
marcador manda, cego a onde o fechador foi posto. Se `docs/MODELOS.md`,
`docs/TRANSACOES.md` §11 ou o marcador triplo (conferidores/catracas/
guardas) ganharem uma seção nova, a região correspondente continua vazia
(o autor precisa decidir, à mão, se quer fundir a elaboração antiga com o
`bloco_*()` correspondente ou deixar as duas coexistindo como hoje) — este
é o buraco que uma tarefa futura poderia fechar, reescrevendo esses três
`bloco_*()` para produzirem a elaboração inteira (não só a lista terse),
absorvendo o texto do autor para dentro do gerador em vez de deixá-lo do
lado de fora.
