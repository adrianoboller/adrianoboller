# Credencial recusada não é falha transitória — e insistir nela gasta a tolerância do outro lado

Descoberta: 09/09/2026, 06:32 UTC, na primeira corrida de
`bancada/replicacao/credencial-recusada.py` contra o binário de antes do
conserto. Pedido 203.

## 1. O que aconteceu

Uma réplica com o `senha_hash` errado e `reconectar_em: 1` fez **5 tentativas
de login em 4,0 s** (75 por minuto); o master, com a política padrão
(`tentativas_ate_bloquear: 5`, `janela_minutos: 10`, `bloqueio_minutos: 60`),
bloqueou o `127.0.0.1` na quinta, por 60 minutos, com motivo «credencial
invalida (login)». O operador do mesmo endereço, com a senha certa, recebeu
«bloqueado desde 06:32:57 até 07:32:57». Depois do bloqueio a réplica
continuou batendo na porta: 8 conexões barradas em 8 s. Com o `reconectar_em`
padrão de 10 s a mesma conta fecha em ~40 s.

O laço da réplica (`servidor.rs`, `laco_da_replica`) tratava todo `Err` igual:
`eprintln!` e `sleep(reconectar_em)`. «A origem me recusou» e «a origem caiu»
chegavam pela mesma variável e recebiam a mesma resposta.

## 2. O que eu concluí primeiro, e estava errado

Três vezes, e as três medidas.

**a) «O conserto pode ser do lado do master, distinguindo a mesma credencial
repetida de N credenciais diferentes.»** O pedido 203 sugeria isso. Antes de
tocar no bloqueio, a pergunta certa era: qual é a assinatura de quem adivinha
senha? É *o mesmo login com provas diferentes, N vezes* — exatamente o que a
réplica errada faz. Um master leniente com repetição do mesmo login seria um
master leniente com força bruta. O bloqueio ficou como estava, e o controle
da bancada (cinco logins errados pelo soquete) prova a cada corrida que ele
continua bloqueando na quinta.

**b) «Corrijo a credencial no master por `usuario_alterar` e o
`replicacao_ligar` põe a réplica a puxar.»** Falhou duas vezes. Primeiro
porque `usuario_alterar` recusa `senha_hash` pelo protocolo (hash pronto
escolheria o próprio custo de derivação) — passei a mandar `senha`. Depois
porque, com a senha certa gravada no master, a réplica **continuou recusada**:
o `senha_hash` do `config.json` da réplica tem de ser o *mesmo texto* do
cadastro de lá, porque é do sal dele que `derivado_do_hash` tira a chave do
desafio-resposta; `usuario_alterar` sorteia outro sal. O conserto do operador
é no `config.json` da réplica, e o reinício é o outro caminho de volta — e é
isso que o caso 8 da bancada faz.

**c) «O diálogo da tela fez 4 tentativas em 0,2 s ao abrir.»** Escrevi isso no
documento, no comentário da tela e no do servidor, lendo o `acessos.log` da
corrida `--tela`. Estava errado: as quatro em 0,3 s eram o **controle** do
caso 5 (os logins errados da própria bancada), que aparece no mesmo log. A
sonda do diálogo era a série seguinte — **uma tentativa a cada 3 s, cinco em
12 s** (33,7 → 45,7 s) — e foi ela que bloqueou o master. O número certo só
saiu quando imprimi cada linha do log com op, usuário e carimbo, em vez de
contar as recusadas de uma vez. Consertar o diálogo fez a série de 3 s sumir
da corrida seguinte, o que é a prova de que a atribuição nova está certa.

## 3. O que a medição disse

Depois do conserto, a mesma corrida (`bancada/replicacao/credencial-recusada.py`,
8 casos, dois deles controles):

| | antes | depois |
|---|---|---|
| tentativas da réplica errada em 12 s | 5 (bloqueio em 4,0 s) | **1** |
| `blacklist.json` do master | `127.0.0.1` por 60 min | vazio |
| operador do mesmo IP entra? | não | **sim** |
| `replicacao_estado` | só `ultimo_erro` | `parada: "credencial_recusada"` |
| origem fora do ar, 20 s | 21 conexões, 1 s fixo | 5 conexões: **1, 2, 4, 8 s** |
| 5 erradas pela própria bancada | bloqueia | **bloqueia** (controle) |
| `replicacao_ligar` com a credencial errada | operação não existia | 1 tentativa, estaciona de novo |

E o que só a tela achou, exercitando num navegador de verdade: o diálogo
«Acompanhar réplica…» sondava a origem com `replicacao_testar` a cada 3 s —
o **mesmo `ligar`** do laço, pela mesma credencial recusada — e bloqueou o
master com o laço já parado direito. O botão de religar nunca aparecia,
porque a sonda falhada apagava as fichas. Guarda: `replicacao_testar` recusa
uma origem estacionada, nomeando `replicacao_ligar`; o diálogo lê o estado
local antes de sondar e não sonda o que está parado.

## 4. A regra

**Classifique a falha antes de decidir o que fazer com ela — e a recusa
determinística estaciona, a queda de rede recua, o resto espera como antes.**
E o corolário: quando o laço parar de insistir, procure quem mais chama a
mesma função de ligar (a sonda da tela, o laço do cluster) — o irmão é quem
chama as mesmas funções na mesma ordem, e ele bloqueia igual.

## 5. Como está guardado hoje

- `crates/phxsql-server/src/replica.rs`: `Falha::{ao_ligar, na_rodada}`,
  `Ritmo`, `Decisao`, seis testes de unidade em `testes_do_ritmo`.
- `servidor.rs`: `apos_a_falha`, `dormir_vigiando`, `esperar_religar`,
  `tomar_religar`, `rodada_classificada`; o laço agendado e o do cluster
  estacionam também; `op_replicacao_ligar`; `recusar_se_estacionada` no
  `replicacao_testar`; teste `replicacao_ligar_deixa_o_pedido_para_o_laco_consumir`.
- Guarda `replica-insiste-na-credencial-recusada` em
  `bancada/guardas/catalogo.py`, PROVADA em 09/09/2026: repor `Dormir` no
  lugar de `Estacionar` derruba `credencial_recusada_estaciona_na_primeira`.
- Bancada pelo soquete `bancada/replicacao/credencial-recusada.py`, declarada
  na `pagina-dos-testes.py`; `--tela` clica o Religar por
  `testes-web/religar-na-tela.mjs`, e o botão está em `DISPENSADOS` do
  conferidor de botões apontando para lá.
- **Onde o buraco ficou:** `replicacao_testar` com `host`/`usuario`/`senha`
  soltos (o assistente antes de gravar a origem) continua ligando na origem a
  cada chamada, sem estado nenhum para estacionar — um cliente que o chamar
  em laço com a senha errada bloqueia o próprio IP como antes. A tela só o
  chama por clique; o portão cobre só as origens configuradas.
