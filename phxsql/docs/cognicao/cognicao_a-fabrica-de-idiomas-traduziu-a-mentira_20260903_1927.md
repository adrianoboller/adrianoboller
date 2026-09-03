# A fábrica de idiomas garantiu que a frase existisse em seis línguas — e a frase era falsa

## 1. O que aconteceu

Fechando a rodada, fui atualizar o dossiê e encontrei nele três passagens
dizendo que a chave estrangeira do PhxSql é «declarada e não aplicada» — «a
chave estrangeira é catálogo», «`Restringir` e `Cascata` são intenção
guardada». Era verdade até esta mesma rodada, e o **pedido 171** a desfez: hoje
toda porta local de escrita confere, e chave declarada **nasce** conferida.

Varri a base atrás da mesma afirmação, e ela não estava só no dossiê. Estava
**na tela**, que é onde dói: quem abrisse o cartão de uma tabela hoje lia, num
aviso vermelho, que as chaves ali «não são aplicadas». E a tela da Claude
mostrava, ao lado do modelo proposto, *«a chave estrangeira do PhxSql é
DECLARADA, e não imposta — o motor não confere a referência na hora de
gravar»*, com o remate *«quem promete integridade que não existe entrega um
estrago com nome bonito»*.

Ao consertar, esbarrei numa segunda frase do mesmo naipe, de outro pedido: a
tela de transações afirmava que *«a transação não vê as próprias escritas»*, e
o **pedido 162** tinha desfeito isso, com o teste
`a_transacao_enxerga_o_que_ela_mesma_escreveu` a poucos metros dali.

## 2. O que eu concluí primeiro, e estava errado

Concluí que era um problema do **dossiê** — documento de vitrine, escrito à
mão, que envelhece porque ninguém o regenera. É a explicação que a casa já
tinha pronta, e por isso mesmo foi a primeira: a lei diz «todo número visível
sai de um gerador», e um parágrafo não é número.

Estava errado no alcance. As frases não estavam só na vitrine escrita à mão —
estavam na **fábrica de idiomas**, que é justamente a máquina que esta casa
construiu para *cuidar* do texto de tela. Elas passaram pelo procedimento
inteiro do `docs/MENSAGENS.md`, ganharam chave, foram traduzidas para os seis
idiomas e passaram na catraca. A fábrica fez exatamente o que promete: garantiu
que a frase existisse em português, francês, inglês, italiano, alemão e
espanhol. **Nenhum degrau dessa máquina pergunta se a frase é verdade.**

## 3. O que a medição disse

A mesma afirmação vencida, contada por lugar:

| onde | quantos textos | como estava guardado |
|---|---:|---|
| tela — nota do cartão da tabela | 1 | cravado no `index.html` |
| tela — brinde do editor ER | 1 | cravado, fora da fábrica |
| tela — aviso da Claude | 2 | **na fábrica, nos 6 idiomas** |
| tela — nota de isolamento | 1 | **na fábrica, nos 6 idiomas** |
| dossiê | 3 | parágrafo à mão |
| `PENDENCIAS.md` | 2 | tabela e lição |
| `HFSQL.md` | 1 | tabela comparativa |
| **total** | **11** | |

Quatro dos cinco textos de tela estavam **dentro** da fábrica — ou seja, a
guarda que existe para o texto de tela cobria todos eles e nenhum foi
apanhado. Traduzida, a frase falsa custou **24 células** de tradução (4 textos
× 6 idiomas).

E o número que fecha o argumento: das duas grades que mostram chave
estrangeira, **uma já mostrava a coluna `verificar` e a outra não** — a de
Estrutura tinha até o comentário certo ao lado («sem ela, duas tabelas com a
mesma chave apareceriam idênticas na tela e se comportariam diferente»), e a
do cartão da tabela não a tinha. O conserto entrou num caminho, e o irmão
ficou; o irmão é que carregava o aviso falso.

## 4. A regra

**Mudança de comportamento vence toda frase que descreve aquele
comportamento — e frase não quebra teste.** Ao fechar um pedido que muda o que
o motor faz, procure a afirmação contrária pelo texto dela, na tela primeiro:
`grep` da negação («não é aplicada», «não confere», «não vê»), não só do
assunto.

E o corolário sobre a fábrica: **estar na fábrica de idiomas prova que o texto
foi traduzido, nunca que ele é verdadeiro.** A catraca conta cobertura, não
veracidade — e uma frase falsa coberta pela fábrica é uma frase falsa em seis
línguas.

## 5. Como está guardado hoje

As onze foram corrigidas nesta rodada, e a catraca desceu de **1.720 para
1.715** porque um dos textos cravados entrou pela fábrica junto com o conserto
(prova real feita nos dois sentidos: passa em 1.715, reprova em 1.714).

**O buraco continua aberto, e é este:** não há guarda que ligue uma frase de
tela ao comportamento que ela descreve. Não sei escrever essa guarda de forma
honesta — casar frase com código por texto seria exatamente o «resolver por
comparação da frase» que a pétrea dos idiomas proíbe, e no dia em que alguém
melhorasse a redação a guarda quebraria calada.

O que dá para guardar, e fica registrado como proposta e não como feito: uma
lista de **afirmações de limitação** — cada uma com a chave do texto, o pedido
que a tornaria falsa, e o teste que prova o comportamento. Aí a guarda pergunta
«o teste que provaria esta frase falsa está verde?», que é uma pergunta sobre
código e não sobre redação.
