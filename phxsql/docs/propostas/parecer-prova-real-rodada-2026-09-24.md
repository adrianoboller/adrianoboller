# Parecer do papel F — auditoria estática das 33 guardas novas de hoje (24/09/2026)

Escopo: as guardas acrescentadas em `bancada/guardas/catalogo.py` pelos três
lotes de hoje — Faceis A (`8e5a545`), 522 (`b5fc11c`) e Integridade na
transação (`a494f33`). `docs/TESTES.md` as lista corretamente como **NÃO
JULGADAS** (a bala de "76 das 400 entradas do catálogo" no fecho do dia — as
33 de hoje mais 43 de lotes anteriores que também nunca chegaram a rodar).

**Este documento NÃO é prova real.** Não compilei nada — ordem explícita do
convocador, disco apertado com quatro frentes compilando. É a auditoria que
decide ONDE gastar o compilar escasso no fecho da rodada. A lei continua de
pé: uma guarda só é `✅ provada` quando `provar-guardas.py` a repuser, medir o
vermelho e desfazer para medir o verde.

## O que foi conferido, e como (sem compilar)

1. **Mecânico, nas 33:** o `trecho` aparece exatamente uma vez no arquivo
   declarado (nenhuma `QUEBRADA` por ausência ou duplicidade); todo nome em
   `caem`/`seguem` existe de fato no fonte (nenhuma referência a teste
   fantasma); o saldo de `{`/`(` do `trecho` bate com o do `troca` (nenhuma
   substituição que desbalanceia chaves — reduz, não elimina, risco de erro
   de compilação por variável/tipo).
2. **Semântico, em profundidade, em 14 das 33** (as de formato em disco,
   integridade referencial e concorrência — a lista de risco abaixo): li o
   `troca` contra o `porque`, e o teste `caem` contra o código real ao redor
   do `trecho` (não só o pedaço citado no catálogo), para responder às duas
   perguntas do papel F: o `troca` reproduz o MESMO defeito que o `porque`
   descreve (não um outro, mais largo ou mais estreito)? E o que o teste
   confere é o veredito direto do defeito, ou uma quantidade que pode sair
   certa por acidente (o padrão do caso fundador — «mediu quanto foi lido, não
   se recusou»)?
3. **As 19 restantes** (Faceis A exceto o `por-login`, e o resto do lote 522
   e integridade fora da lista de risco) passaram só pelo item 1. Não achei
   red flag ao ler `porque`/`troca`/`caem` de cada uma, mas não tracei o teste
   contra o código ao redor — é o que falta para virarem confiança de
   verdade, e é trabalho de compilar.

## Achados da leitura profunda (14 guardas)

Nenhuma das 14 mostrou sinal de "passa por engano" — mas duas mereceram
investigação extra antes de eu descartar a suspeita, e registro as duas
porque é exatamente o tipo de raciocínio que `provar-guardas.py` teria de
confirmar, e eu só confirmei lendo:

- **`elo-implicito-sem-trava`** — à primeira leitura pareci ver um problema:
  o `troca` era `for elo in elos.iter().take(0) { ... }`, e temi que isso
  zerasse a cascata inteira (não só a trava), fazendo o `caem`
  (`o_elo_implicito_respeita_a_leitura_repetivel_de_outra_transacao`) cair
  pelo motivo ERRADO — "nada aconteceu" em vez de "aconteceu sem travar". Fui
  ao `servidor.rs` (linha ~17091) e o `trecho` citado é só o PRIMEIRO de TRÊS
  laços sobre `elos`: o que trava (o que o catálogo troca), um de
  contabilidade, e o que efetivamente aplica a escrita (`pre_conferir_uma`,
  linha 17107). Os outros dois continuam rodando. A suspeita morreu lida: o
  `troca` isola exatamente o laço da trava, e o teste mede o sintoma direto —
  `cod(&t3)` relido como 6 em vez de 5 — não um proxy. **Fica como
  aprendizado**: quando o `trecho` do catálogo é só um fragmento de um bloco
  maior, ler só o fragmento engana; é preciso ver os laços vizinhos.
- **`por-login-para-no-primeiro-que-casa`** — o `troca` substitui a função
  inteira, e leva junto a instrumentação de contagem
  (`COMPARACOES_DE_LOGIN`). Temi que o teste
  (`por_login_faz_o_mesmo_numero_de_comparacoes_para_qualquer_login`) medisse
  só a IGUALDADE entre "primeiro" e "último" e "inexistente" — e os três
  dessem 0 (contador nunca incrementado) e passassem por acidente, o padrão
  exato do caso fundador desta lei. Não é o caso: o teste tem um
  `assert_eq!(do_primeiro, 500, …)` ABSOLUTO antes de qualquer comparação
  relativa — com o defeito, `do_primeiro` é 0 (a instrumentação sumiu junto
  da função), e `0 != 500` cai sozinho. A prova é pelo valor absoluto, não só
  pela simetria.

Os outros 12 (fsync/byte 52, atestado por processo, `auto-referência`,
ciclo de COMMITs, cascata em voo) verificados sem achado — detalhe de cada um
na seção seguinte.

## Ponto 3: o que depende do SO, e como cada um prova

| categoria | guardas do lote de hoje | como prova |
|---|---|---|
| **fsync recusado** | `atestado-de-antes-da-recusa-vale-depois` | **Emulação declarada.** `phxsql_store::sincronia::falha_de_teste` forja o `EIO`/`ENOSPC` porque montar disco que recusa de verdade exige `CAP_SYS_ADMIN`. O comentário no topo do arquivo de teste é explícito: *"as recusas aqui são forjadas, e a prova contra o SO mora ao lado"* (`bancada/catastrofes/`). Não é uma prova que finge ser real — é uma prova de CONDUTA (o código reage certo à recusa), com a prova de OCORRÊNCIA (o SO recusa de verdade) delegada e já rodada hoje: o commit `b5fc11c` registra "3/3" contra ext4 em loop com provisionamento fino. |
| **pânico com `Drop`** | `cascata-em-voo-ignorada-no-drop`, `cascata-em-voo-so-no-aplicar` | **Real.** `std::panic::catch_unwind(AssertUnwindSafe(...))` em cima de um `panic!()` de verdade, disparado por um gancho de teste (`panico_de_teste::armar(Ponto::...)`). O unwind roda os `Drop` de verdade — não há flag que finja o efeito do pânico. |
| **segundo processo** | `fechar-do-embutido-nao-sincroniza` | **Real.** `std::process::Command::new(std::env::current_exe().unwrap())` reexecuta o PRÓPRIO binário de teste com `--exact …sonda_522_abre_no_processo_seguinte --ignored`, passando o diretório por variável de ambiente. É um processo do SO de verdade, não um reset de estado in-process. |
| **"processo novo" (variante mais fraca)** | `atestado-pelo-caminho-e-nao-pelo-arquivo`, `atestado-fica-no-caminho-velho`, `renomear-esquece-o-atestado`, `restauracao-nao-reconstroi-o-marcado`, `arranque-nao-reconstroi-o-marcado` | **Emulação, não processo real.** Todas usam `esquecer_atestados_para_teste()`, que limpa o registro em memória do processo ATUAL, simulando "o que um processo novo veria" (o registro nasceria vazio). É uma emulação aceitável **porque o próprio registro é só em RAM por processo** — um processo de verdade também nasceria com ele vazio, então não há diferença observável que a emulação possa esconder. Mas **é emulação**, e só uma das cinco categorias acima (`fechar-do-embutido-nao-sincroniza`) paga o preço de um processo de verdade. Se algum dia o atestado passar a persistir em disco ou em algo compartilhado entre processos, estas cinco páram de provar o que dizem provar e ninguém vai perceber lendo o catálogo — só relendo o código do registro. |
| **soquete** | nenhuma do lote de hoje | — |

## As suspeitas (achados que pedem confirmação por compilar, não defeitos confirmados)

1. **`elo-implicito-sem-trava` e `por-login-para-no-primeiro-que-casa`** —
   descritas acima: a leitura resolveu a suspeita, mas como não compilei,
   registro como PRIORIDADE de confirmação, não como fato.
2. **Emulação de "processo novo" em 5 guardas do lote 522** (tabela acima) —
   não é falha, é dívida: documentar no próprio catálogo (campo `porque`) que
   o "processo novo" ali é `esquecer_atestados_para_teste`, e não um
   `std::process::Command`, para quem ler o catálogo não presumir mais do
   que a guarda prova.
3. **19 guardas só com checagem mecânica** (item 3 da seção anterior) — sem
   suspeita concreta, mas sem leitura profunda: risco desconhecido, não risco
   zero.

Nenhuma das 33 mostrou `trecho` ausente/duplicado, teste referenciado
inexistente, ou desbalanço de chaves — ou seja, nenhuma é candidata óbvia a
`QUEBRADA` pela leitura estática. Isso NÃO substitui rodar
`provar-guardas.py`: o `NAO PEGOU` (teste que continua passando com o defeito
reposto) só aparece rodando de verdade, e é justamente o veredito que a
leitura de código não alcança com segurança — é possível o `troca` compilar,
o `caem` cair por um motivo raso e ainda assim não provar o que o `porque`
afirma. As duas suspeitas da seção anterior mostram como esse raciocínio
engana fácil mesmo para quem está lendo com cuidado.

## As 10 de maior risco para o provador, no fecho da rodada

Sem cripto/senha neste lote (as guardas de cifra de hoje são de commits
anteriores já julgados). A lista cobre formato em disco (a raiz do 522 e o
único caminho de segundo-processo real), integridade referencial (a regra
primordial) e concorrência (o desempate do ciclo de COMMITs, com a regressão
de vivacidade medida em 11 s/1.870 rodadas):

1. `fechar-baixa-o-byte-52-sem-fsync` — a raiz do lote 522: byte 52 = 0 sem `fsync`.
2. `atestado-sobrevive-a-escrita` — a companion: escrita que não terminou não perde o atestado.
3. `atestado-de-antes-da-recusa-vale-depois` — fsync recusado no diretório, único item forjado desta lista (com a prova real já feita em `bancada/catastrofes/`).
4. `fechar-do-embutido-nao-sincroniza` — único item com processo do SO de verdade (`std::process::Command`) nas 33.
5. `arranque-nao-reconstroi-o-marcado` — recuperação no arranque; `prazo: 600` já indica que a própria frente a considerou pesada.
6. `auto-referencia-pulada-no-excluir` — a regra primordial («nunca mata o pai que tem filhos») aplicada à auto-referência.
7. `auto-laco-conta-como-filha` — o lado oposto da mesma regra: não travar quem só aponta para si.
8. `ciclo-de-commits-sem-desempate` — o desempate que fechou a regressão de vivacidade (1.870 rodadas em 11 s).
9. `quem-cede-no-ciclo-segura-as-travas` — se quem cede não solta a trava, o desempate de cima não resolve nada.
10. `elo-implicito-sem-trava` — a suspeita que só morreu lendo o código vizinho; é a que mais pede confirmação por compilar.

Comando exato (`--so` casa por SUBSTRING no id — `--so auto-referencia-pulada-no-excluir`
também alcança a irmã `-pelo-servidor`, o que é desejável aqui, mesma dupla de defeito por dois caminhos):

```bash
python3 bancada/guardas/provar-guardas.py \
  --so fechar-baixa-o-byte-52-sem-fsync \
  --so atestado-sobrevive-a-escrita \
  --so atestado-de-antes-da-recusa-vale-depois \
  --so fechar-do-embutido-nao-sincroniza \
  --so arranque-nao-reconstroi-o-marcado \
  --so auto-referencia-pulada-no-excluir \
  --so auto-laco-conta-como-filha \
  --so ciclo-de-commits-sem-desempate \
  --so quem-cede-no-ciclo-segura-as-travas \
  --so elo-implicito-sem-trava \
  --json /tmp/guardas-prioridade-2026-09-24.json
```

Depois, `python3 bancada/guardas/tabela-no-testes.py /tmp/guardas-prioridade-2026-09-24.json`
para levar o veredito real ao `docs/TESTES.md` — e SÓ ENTÃO essas 10 saem de
NÃO JULGADAS.

## Como isto se resolve (nos dois sentidos, quando alguém compilar)

Para cada uma das 33, a prova real fica assim, e é isso que falta:

1. rodar `provar-guardas.py --so <id>` na árvore de hoje: as 33 têm de dar
   `✅ provada` (ou `🟰 redundante`, se algum `espera: "nada muda"` aparecer
   — não vi nenhuma das 33 usando esse modo);
2. qualquer `NAO PEGOU` é o achado mais valioso da rodada, por definição
   desta casa: teste que passa por engano é pior que teste que falta, e vai
   direto para o topo do PENDENCIAS;
3. qualquer `ESTRAGOU` (um `seguem` caiu junto) sobe a mesma prioridade: a
   troca quebrou mais do que o defeito original quebrava.

## Resumo

- **33 guardas novas auditadas** (Faceis A: 6, 522: 12, Integridade: 15).
- **0 suspeitas confirmadas como defeito** — 2 suspeitas levantadas e
  derrubadas pela leitura do código vizinho (`elo-implicito-sem-trava`,
  `por-login-para-no-primeiro-que-casa`), registradas acima porque o
  raciocínio que as descartou é o que a rodada de compilar tem de repetir de
  verdade, não confiar em mim.
- **1 dívida de documentação**: 5 guardas do lote 522 chamam de "processo
  novo" uma emulação em memória, não um processo do SO; só uma
  (`fechar-do-embutido-nao-sincroniza`) paga um `std::process::Command` de
  verdade.
- **10 para o provador**, comando acima, cobrindo formato em disco,
  integridade referencial e concorrência — sem cripto/senha porque não há
  guarda de cripto no lote de hoje.
- **19 guardas** só com checagem mecânica (trecho único, testes existem,
  chaves balanceadas): sem suspeita, sem confirmação profunda — a próxima
  rodada de auditoria estática, se o disco continuar apertado, começa por
  elas.
