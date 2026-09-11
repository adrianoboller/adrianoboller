# Cognição: um "status" precisa de ESTADO, não de bool — e a terceira aba estourou a barra

**Descoberta:** 11/09/2026 ~16:30 UTC.

## 1. O que aconteceu

O dono pediu **"Status das solicitações"** para o correio. O protótipo
`crates/phxsql-core/examples/correio-e2e.rs` já tinha os dois canais de pedido
(confiança e moderação), mas nenhum carregava um **estado** que uma tela de
status pudesse mostrar: a confiança era um `bool` (aceita ou não) e a moderação
não tinha estado nenhum. Modelei os estados, provei as transições e pus a aba
**Solicitações** na maquete. `cargo run --example correio-e2e` → **24/24 PROVA
VERDE**; a maquete exercitada no navegador → **40/41** (o 1 vermelho é a fonte
do Google bloqueada pelo proxy do sandbox, não defeito da página).

## 2. O que eu concluí primeiro, e estava errado

O `bool` **bastava para o portão**: `confia(a,b)` só precisa saber se há
confiança aceita, e o `bool` responde isso. Então quase entreguei o status
lendo o `bool` — "aceita / não aceita". Está certo para o *gate* e **mentiroso
para o status**: "não aceita" junta duas coisas que a pessoa distingue —
*recusada* ("ele disse não") e *pendente* ("ele ainda não respondeu"). Um
status que mostra "pendente" para quem foi recusado esconde exatamente o que o
status existe para contar. O `bool` que servia a uma função não servia à
outra, e eu ia usar o mesmo campo para as duas.

## 3. O que a medição disse

- **Motor (24/24):** confiança virou `pendente/aceita/recusada`, moderação
  `aberta/deferida/indeferida`. Prova real nos dois sentidos: recusar como
  no-op deixa o estado `Pendente` e a checagem falha; recusar "aceitando"
  deixa `confia()` verdadeiro e o envio passa — a outra checagem falha. A
  decisão de moderação também exige motivo (a lei do `.reason`), e não se
  re-julga o que já foi julgado.
- **Tela (40/41):** exercitar a **400px** pegou um defeito que ler o código
  não pegaria — a **terceira aba** alargou a barra de modos e estourou **49px**
  na horizontal, porque `.modos` era um flex **sem quebra de linha**. Só
  aparece exercitando; o conserto foi `flex-wrap: wrap` (overflow = 0).

## 4. A regra

**"Status" pede ESTADO explícito, não bool: o campo que serve a um portão
("está aceito?") mente quando vira status, porque junta 'recusado' com 'sem
resposta' — a pessoa distingue os dois. E recusar não é bloquear: são estados
diferentes, e o status mostra a diferença.**

## 5. Como está guardado hoje, e onde o buraco ficou

Guardado no protótipo rodável (modelo em memória), commit `afc0a77`. A tela é
a maquete (Versão 4). Buracos, nomeados para não virar promessa:

- **Formato em disco** — as tabelas PSCH (contas, confianças **com estado**,
  moderações **com estado**, mensagens, anexos-à-parte) não existem; decide-se
  com o dono antes de gravar (papel C).
- **Alcance da pétrea "interface só se prova exercitando":** uma peça nova numa
  barra flex **sem `flex-wrap`** estoura na horizontal em tela estreita, e só a
  prova a 400px pega. Quando entrar um item novo numa linha de botões/abas,
  confira se a linha quebra.
- **Recusar é soft:** quem foi recusado pode pedir de novo (bounded pelo teto
  de pendentes e, no limite, pelo bloqueio). Se o uso real mostrar isso virar
  vetor de insistência, medir antes de endurecer.
