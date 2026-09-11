# Onde mora o portão do tipo de motor — e por que no store, não no despachar

- **Assunto:** infraestrutura dos três tipos de database (padrão/hive/vetorial)
- **Descoberto:** 11/09/2026, ~09:47Z, ao fiar a recusa «motor em construção»
- **Papéis:** C (DBA, formato e marcador), B (engenharia), A (integração)

## 1. O que aconteceu

O dono pediu a infraestrutura para três tipos de database. Padrão tem motor;
hive e vetorial não — os motores são frentes abertas. A infraestrutura ficou:
`TipoDatabase` no núcleo, o marcador `_database.json` por database (ausência =
padrão), e a recusa honesta de operar tabela num database cujo motor ainda não
existe.

A pergunta de projeto que não era óbvia: **onde fica o portão que recusa?**
Um database hive é criado de verdade (marcador gravado), mas se alguém rodar
`criar_tabela` nele hoje, o motor **padrão** rodaria e criaria um `.reg`
relacional dentro de um diretório que se diz colmeia — «fingir que gravou»,
metade pior que nada.

## 2. O que eu concluí primeiro, e estava errado

Concluí primeiro que o portão iria no **servidor**, no `despachar`/`executar`,
espelhando o portão de permissão por tabela — «já existe um portão único ali,
o de tipo entra ao lado». Estava errado, e o motivo é a própria pétrea do
portão de permissão: *«não espalhe o portão por quarenta operações, porque a
que alguém esquecer vira a porta dos fundos»*. Um portão de tipo no despachar
precisaria **classificar** cada operação («esta mexe em tabela? aquela é só
catálogo?»), e essa classificação é exatamente o espalhamento que a pétrea
condena — uma op de dados nova que alguém esquecesse de classificar rodaria o
motor padrão num hive, calada.

O erro era supor que «portão único» quer dizer «no mesmo lugar que o outro
portão único». Não quer: quer dizer **uma função de portão, chamada nas
entradas naturais**, no ponto onde não dá para desviar.

## 3. O que a medição disse

Não é medição de tempo — é de **alcance e regressão**, e conta como número:

- O motor padrão tem **6 entradas de tabela** no `Database`/`Instancia`
  (`criar_tabela`, `abrir_tabela`, `excluir_tabela`, `duplicar_tabela`,
  `copiar_tabela_para`, e o `abrir_para_ler` da `Instancia`). Uma única função
  `exigir_motor_padrao()` é chamada no topo das seis — um portão, seis chamadas,
  nenhuma reimplementação. As entradas de **listagem** (`tabelas`,
  `existe_tabela`) ficam de fora de propósito: um hive existe e aparece na
  lista com o seu tipo; escondê-lo seria outra mentira.
- Colocado no store, o portão **não pode ser desviado** por uma op de servidor
  nova: qualquer op que vá operar tabela abre um `Database` do disco (que lê o
  marcador) e bate na função. No despachar, uma op nova que esquecesse a
  classificação passaria.
- Regressão: **169 testes de store + 1.022 de servidor** seguiram verdes com o
  portão dentro. O motor padrão opera exatamente como antes em database padrão
  — o portão só barra o que **não** é padrão. Prova real nos dois sentidos:
  `hive_recusa_operacao_de_tabela_mas_padrao_funciona` fica VERMELHO se alguém
  tirar o `exigir_motor_padrao` de `criar_tabela` (aí a colmeia criaria o `.reg`
  calada).

E um alcance de documentação: o `FORMATO.md` §11 afirmava «a regra é
estrutural, **sem arquivo de marcação**». A frase quebrou num ponto só — o
**tipo** do database — e a correção honesta foi cirúrgica: schema e tabela
continuam 100% estruturais, só o tipo ganhou marca. Alargar a negação para «tem
marcação» teria mentido sobre schema e tabela.

## 4. A regra

**Portão de invariante de motor mora onde o motor entra, não onde o protocolo
despacha.** Uma invariante do tipo «o motor X só opera dado do tipo X» se impõe
nas entradas do próprio motor (uma função de portão, chamada em cada entrada),
porque ali não há como uma operação nova nascer sem passar por ela. Pôr a mesma
regra no despachar exigiria classificar operação por operação — o espalhamento
que a porta dos fundos adora.

## 5. Como está guardado hoje

- `crates/phxsql-store/src/catalogo.rs`: `Database::exigir_motor_padrao()`, o
  portão, chamado nas seis entradas de tabela; o marcador `_database.json`
  (`escrever_marca`/`ler_marca`, ausência/corrompido → padrão).
- `crates/phxsql-core/src/tipo_database.rs`: o enum `TipoDatabase` e
  `motor_pronto()`.
- `crates/phxsql-server/src/servidor.rs`: `op_criar_database` aceita `"tipo"`,
  devolve `motor_pronto` e a `nota`.
- Provas: `hive_recusa_operacao_de_tabela_mas_padrao_funciona` (store e
  servidor), mais os testes do enum e do marcador.
- Formato: `docs/FORMATO.md` §11.1. Propostas dos motores:
  `docs/propostas/colmeia.md`, `docs/propostas/vetorial.md`.
- **Onde o buraco fica, de propósito:** os motores de hive e vetorial não
  existem — a infraestrutura roteia por eles e diz «em construção». Cada um
  começa por **medir a premissa** (ver as propostas), não por escrever formato.
  E vêm depois do P0 de atomicidade, por ordem do parecer do Sprint 0010.
