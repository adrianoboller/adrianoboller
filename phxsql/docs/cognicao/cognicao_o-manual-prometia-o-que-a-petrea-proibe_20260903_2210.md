# O manual prometia o que a pétrea proíbe — e ninguém executava os exemplos dele

## 1. O que aconteceu

Fui atualizar o `MANUAL.txt` e a seção de chave estrangeira tinha **quatro
erros e uma omissão**. O pior deles não era estar desatualizado: era
**documentar uma API que o motor recusa**.

A tabela de ações listava as quatro — `restringir`, `cascata`, `anular`,
`nada` — numa lista só, sem dizer o lado. Mas `ao_excluir` aceita **só**
`restringir`, por pétrea, e recusa na declaração. E o exemplo do `declarar_fk`
mandava `"ao_excluir":"cascata"`. **Quem copiasse o exemplo do manual tomava
erro na cara.**

Os outros três: dizia *«o motor ainda NÃO a IMPÕE»* (falso desde o pedido 171);
dizia *«ausente é restringir»* (verdade no excluir, **falso** no alterar, que
nasce `cascata`); e não mencionava o `"verificar"`, o nasce-conferida nem a
exigência de índice dos dois lados.

## 2. O que eu concluí primeiro, e estava errado

Concluí que era **uma** correção: trocar *«ainda não impõe»* por *«impõe»*, e
pronto — era isso que a rodada de hoje tinha mudado.

Estava errado, e a diferença importa. A frase sobre impor estava vencida **por
mudança de comportamento**, que é o padrão que eu já conhecia. Os outros três
estavam errados **desde sempre**: a tabela plana, o padrão por lado e o exemplo
com `cascata` nunca foram verdade em versão nenhuma. Não envelheceram —
**nasceram errados**, e sobreviveram porque ninguém nunca executou o que o
manual manda executar.

E ao escrever a prova eu errei mais duas vezes, as duas do mesmo naipe:

- **conferi o veredito em vez do motivo.** Duas conferências saíram verdes com
  o servidor respondendo *«acesso negado: faça login»* — o `ok:false` que eu
  lia como «a FK recusou» era o portão de login recusando. Teste que passa por
  engano é pior que teste que falta, e este passava **no meio de nove que
  falhavam**, o que o disfarça ainda melhor;
- **fiz a prova depender da ordem das corridas.** Reusei o database `loja`, e a
  segunda corrida reprovou em três por «chave duplicada».

## 3. O que a medição disse

| | |
|---|---:|
| erros na seção de FK | **4** |
| omissões | **1** (o `verificar`, o nasce-conferida, o índice dos dois lados) |
| deles causados por mudança de comportamento | **1** |
| deles **nascidos errados** | **3** |
| conferências da prova nova | **12** |
| conferências que passavam por engano na 1ª versão | **2 de 11** |

E a prova real no outro sentido: reposto o exemplo antigo do manual, o motor
responde com todas as letras — *«"ao_excluir": "cascata" não existe no PhxSql
— ao excluir é sempre "restringir"»*. A recusa já estava certa e escrita; só o
manual é que não a conhecia.

## 4. A regra

**Exemplo de manual que ninguém executa é exemplo que envelhece calado — e que
pode nascer errado sem ninguém perceber.** Documento com bloco de comando
executável ganha uma prova que o roda contra um servidor de verdade, e a prova
entra na bateria única.

E o corolário sobre a natureza do erro: **nem toda frase errada num documento
envelheceu.** Ao varrer um documento, procure as duas espécies — a que era
verdade e deixou de ser, e a que nunca foi. A segunda não se acha comparando
com o histórico; acha-se **executando**.

## 5. Como está guardado hoje

`bancada/manual/provar-manual.py`, autossuficiente: sobe um `phxsqld` só dela
na porta 6410, num diretório temporário, e o derruba **pelo PID** — nunca por
`pkill`, que mataria o servidor de um vizinho. Registrada na bateria única
(`provar.py`), com prazo de 300 s.

**O buraco que fica nomeado:** ela cobre **só** a seção de chave estrangeira.
O `MANUAL.txt` tem 2.935 linhas e dezenas de outros blocos JSON executáveis —
`criar_tabela`, `inserir_lote`, `begin`/`commit`, `restaurar_backup`,
`profiler_ligar`, os quatro modos de replicação. Nenhum deles é executado por
ninguém hoje. O caminho que eu recomendo, e não fiz: **extrair os blocos do
próprio manual** em vez de recopiá-los na prova — bloco copiado é a segunda
cópia que diverge, que é a lição que esta casa já pagou no rodapé do dossiê.
