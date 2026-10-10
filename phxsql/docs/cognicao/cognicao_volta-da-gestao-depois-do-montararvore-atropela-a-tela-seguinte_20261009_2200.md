# A volta a «Tabelas de» depois do `montarArvore` atropela a tela que a pessoa abriu no meio-tempo

## O que aconteceu

Pedido 782. O caso `testes-web/casos/49-senha-de-execucao.mjs` caía uma vez por
corrida inteira da bateria (dois temas), no passo 3, com o título
«Tabelas de batExecC» na tela; sozinho, passava. A causa estava na TELA, não no
teste: `excluirTabelaDe` termina em `await montarArvore(false); return
gerirTabelas(db)` sem conferir a posse do painel. O passo 2 do caso exclui
`alvo`; o teste segue quando o SERVIDOR já não tem a tabela, mas a tela ainda
está no `montarArvore`. Quando a volta caía entre o `folha("Gestão de alvo2")`
do passo 3 e a pintura do corpo, o `gerirTabela` perdia a posse e desistia
(guarda do 636 funcionando), e a pessoa ficava na lista: `#btVoltarTabs` nunca
aparecia.

Duas irmãs com a mesma forma: o «Criar tabela» do cadastro completo e o
«Colar aqui». O pedido 636 tinha consertado só o «Criar» do CARTÃO
(`telaDiagramaER(db, vez)`).

## O que eu concluí primeiro, e estava errado

A frente anterior chamou de «corrida de pintura quando o servidor acumulou
muitas bases» — certo no sintoma, vago na causa: parecia lentidão do servidor
(prazo de 20 s estourando). Não é: o prazo estoura porque o `gerirTabela`
ABANDONOU a pintura ao perder a posse, não porque esperou demais. E a outra
hipótese, o `#aviso` «trancada» lido cedo ou sobrescrito, morreu no traço: o
passo 3 passava pela espera do aviso nas corridas medidas.

## O que a medição disse

Traço de `folha()` e `montarArvore()` embrulhados dentro do caso 49, numa
corrida inteira (base `898ce19f`): o `montarArvore` do DROP do passo 2 levou
**417 ms com 41 bases** (tema escuro) e **918 ms com 81 bases** (claro), em
série, um `tabelas` por base (~11 ms/base). Ele terminou **depois** de o passo 3
ter pintado a gestão de `alvo2` (claro: gestão em 2951-2967 ms, volta em
3691 ms) e repintou «Tabelas de batExecC». A janela que derruba é de ~16 ms (a
fase «carregando» do `gerirTabela`) mais o tempo até o clique — por isso uma
vez por corrida, e por isso a falha anda de passo.

Prova real, cena 4 do `testes-web/casos/38-pintura-tardia.mjs` (segura `bancos`,
a primeira op do `montarArvore`, abre a telemetria e solta):

- binário da base: VERMELHO em «Excluir tabela» («achei "Tabelas de batptdC"»);
- só o excluir consertado: VERMELHO em «Criar tabela (cadastro completo)»;
- excluir e criar consertados, colar com `if (false && …)`: VERMELHO em «Colar aqui»;
- os três consertados: verde nos dois temas.

Consertado o 49, a segunda das três corridas de prova caiu em OUTRO caso, o
`42-botoes-de-lgpd-e-bloqueios` («a whitelist que o servidor guardou: esperava
[…], achei []»). Ali o defeito era do TESTE: a espera depois do «Salvar
whitelist» casava o valor do `#wlEditavel` — que o próprio caso tinha acabado de
digitar —, então passava na hora e o `bloqueios` lia o servidor antes de o
`whitelist_salvar` voltar. Prova: com o `whitelist_salvar` segurado 1,5 s no
fio, a espera velha liberou **9 ms** depois do clique e o caso caiu com a mesma
frase da bateria; a espera nova (o campo REPINTADO, sem a marca posta antes do
clique) liberou 22 ms depois de o fio soltar, e passou.

## A regra

Ação que termina voltando a uma tela depois de um `await` guarda
`vezDoPainel()` ANTES do primeiro `await` e confere `aindaNoPainel(vez)` antes
da volta: a ação vale, a tela da pessoa fica. E no teste: espera que casa o
que o próprio caso digitou não espera nada — marque o elemento antes do clique
e espere o repintado.

## Como está guardado hoje

Cena 4 do caso 38 (as três ações da gestão). O buraco que fica: a guarda é
convenção por função, e a varredura do 636 contou só `folha()` depois de
`await` — `return gerirTabelas(db)` não chama `folha` na linha e escapou dela.
O `est.atual = null` do excluir também escreve tarde (zera a tabela corrente de
quem já abriu outra); não derruba nada medido e ficou como está.
