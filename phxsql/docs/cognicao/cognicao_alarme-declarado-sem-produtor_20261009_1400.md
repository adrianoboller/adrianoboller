# Dez dos 24 alarmes do aquário estão declarados, traduzidos e sem produtor

**Estado:** PENDENTE

- **Quando:** 2026-10-09, 14:00
- **Onde:** `crates/phxsql-server/src/aquario/mod.rs` (`Alarme::TODOS`),
  `crates/phxsql-server/ui/aquario.js` (`MOTIVOS`),
  `crates/phxsql-server/src/idiomas.rs` (`tela.aq_m_*`)
- **Pedido:** 707, fatia A14 (documentação)

## O que aconteceu

Ao escrever o §11.5 do `docs/TELEMETRIA.md` («os alarmes e o sedimento») e ao
conferir a tabela de chaves do `docs/MENSAGENS.md`, o inventário do enum
(`Alarme::TODOS`, 24 variantes) bateu com a tela: 31 entradas em `MOTIVOS` (7 de
estado + 24 de alarme) e 29 chaves `tela.aq_m_*`. Tudo fecha. Mas fechar a tabela
de tradução não diz que o alarme **acende**: bastou procurar, no fonte, quem
chama `Alarme::X` fora de `aquario/` e dos testes.

## O que eu concluí primeiro, e estava errado

Que os 24 alarmes existiam de ponta a ponta: o enum tem `chave()`, `bit()`,
`gravidade()` e `grupo()` para cada um, o conferidor de idiomas fecha o laço da
tela, e a mensagem de commit da A3 fala em «marcas na origem». A cobertura da
**declaração** e da **tradução** foi lida como cobertura do **produtor**. O
cabeçalho do pedido 707 (item 7) lista LOCK, DISCO, DADO EM RISCO, RÉPLICA,
SEGURANÇA e PRAZO como vermelhos, e eu li a lista como estado.

## O que a medição disse

Busca por `Alarme::<variante>` em `crates/`, excluindo `src/aquario/mod.rs`,
`classe.rs`, `alarme.rs` (só a definição e os testes), `log.rs` e testes, em
09/10/2026, base `3cfc27e5`:

- **Com produtor (14):** `TravaReentrante`, `TravaEnvenenada`, `ErroDeDisco`,
  `DadoCorrompido` (deduzido em `conferir_o_1001`), `ForcaBruta`,
  `SenhaEmClaro`, `PrazoEstourado`, `ForaDoHabitual`, `IntegridadeRecusada`,
  `EsgotamentoPrevisto`, `EsgotamentoIminente`, `ReplicaAtrasada`,
  `InjecaoSuspeita`, `PlanoLargo`.
- **Sem nenhum produtor (10):** `TransacaoAcimaDoTeto`, `FechoRecusado` (só um
  teste o aciona), `FsyncRecusadoAntes`, `MarcaNaoResolvida`, `IndiceAtrasado`,
  `ContinuidadeRompida`, `OrigemInalcancavel`, `FirewallBloqueou`, `DiscoLento`
  e `ForaDoHabitualReincidente`.
- Dos 11 alarmes de escopo `Servidor` (os que viram sedimento), **3** acendem.
  O «Fundo do aquário» fica vazio para os outros 8 mesmo quando o fato
  acontece.
- Também sem escritor: os eventos `nasceu`, `morta`, `retrato` e `sedimento` do
  `aquario.log` (`Evento` em `log.rs`). Os dois primeiros são do item 5 do
  pedido 707.

Isto não é regressão: as fatias A3/A5 entregaram o produtor **único** e as
marcas de tarefa; as origens de servidor eram fatias futuras. O defeito seria
documentar o aquário como se cobrisse o item 7 inteiro.

## A regra

Antes de escrever «o aquário mostra X», procure quem **produz** X no fonte. Tabela
de tradução completa, enum completo e conferidor de idiomas verde provam só que o
alarme pode ser **dito**, não que pode acontecer.

## Como está guardado hoje

Documentado como está, com a lista dos dez, em `docs/TELEMETRIA.md` §11.5 e
§11.7. **O buraco continua aberto:** nenhum teste reprova variante de `Alarme`
sem produtor, e a lista dos dez é retrato desta data (a busca acima a refaz).
Falta decidir, por alarme, entre ligar a origem e retirar a variante; isso é
trabalho de engenharia sobre o pedido 707, não desta fatia.

## Depois: pedido 769 (09/10/2026, papel B)

- **Os dez ganharam produtor**, nenhum saiu como chave morta: todo fato
  existia no código (tabela em `docs/TELEMETRIA.md` §11.5). Um teste por
  produtor em `servidor/testes_dos_produtores_769.rs`, mais
  `tests/alarme-da-transacao-acima-do-teto.rs` (os dois irmãos do teto) e
  `a_ruptura_da_continuidade_vira_pedra_no_sedimento` em
  `tests/continuidade-da-replica.rs`. Doze guardas novas no catálogo, 12/12
  PROVADA pelo `provar-guardas.py`.
- **RED medido:** com as 14 linhas do `sinal` tiradas de uma vez (13 sinais e
  a troca do `reincide`), 9 dos 10 testes de servidor caem (o décimo é o do
  comportamento velho, a whitelist, que não alarma nos dois casos), o teste
  do teto cai na metade do COMMIT, e o da continuidade cai na pedra. Tirada
  só a linha da carga, o teste do teto cai na metade da carga. O bidirecional
  (três marcas que ficam no disco) **não tem teste** que o alcance.
- **A busca desta cognição contou errado três alarmes:** `ForcaBruta`,
  `SenhaEmClaro` e `IntegridadeRecusada` aparecem só no `mod testes` do
  `ocorrencias.rs`, e foram contados como produzidos. A regra de cima vale
  para a própria busca: «fora dos testes» tem de excluir o **módulo** de
  teste, não só os arquivos `testes_*`. Os três continuam sem produtor.
- **A16:** `aquario::testes::todo_alarme_tem_motivo_na_tabela_da_tela` liga o
  enum à tabela `MOTIVOS` do `ui/aquario.js`; tirar a linha de
  `origem_inalcancavel` derrubou o teste nomeando a chave.

Estado continua **PENDENTE**: a evidência acima está escrita, e a promoção
não é de quem a produziu.
