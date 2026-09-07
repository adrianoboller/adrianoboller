# A porta nova herda o defeito velho da porta antiga

*Descoberto em 07/09/2026, 17:52, exercitando o `SHOW SERVER SETTINGS` contra o
servidor vivo na porta 7300.*

## 1. O que aconteceu

O `ALTER SERVER SET` entrou desembocando no **mesmo** `config_gravar` — mesmo
portão, mesma validação, mesma gravação. O `SHOW SERVER SETTINGS` entrou
lendo o **mesmo** `configuracao_json()` que a tela de Configurações lê. Reuso
exemplar, nenhum caminho duplicado.

E a porta nova nasceu **com um defeito que a porta velha já tinha consertado**:

```text
SQL> ALTER SERVER SET timeout_s = 45 MOTIVO 'suporte';
     {"gravado":true,"exigem_reinicio":["timeout_s"]}

SQL> SHOW SERVER SETTINGS;
     {"recurso":"timeout_s","valor":30, …}     <- gravou 45, respondeu 30
```

O `configuracao_json()` devolve o que está **valendo**, e para campo que só
aplica no próximo arranque isso é o valor de antes. A tela de Configurações já
tinha pago exatamente isso: *«quem acabou de digitar 90 via 45 de novo e não
tinha como saber que o 90 estava gravado»* — e o conserto de lá foi a chave
`no_arquivo`, montada por `divergencias_do_arquivo` e devolvida **dentro** do
mesmo `configuracao_json()`. O conserto estava lá. A porta nova simplesmente
não olhava para ele: ela lia campo a campo com `valor_em(&vivo, campo)` e o
bloco `no_arquivo` passava ao largo, intocado, na mesma resposta.

## 2. O que eu concluí primeiro, e estava errado

Concluí que **reusar a função certa era suficiente** — que o defeito de
«mostrar valor velho» era um defeito *do leitor de configuração*, e portanto
quem chamasse o leitor certo herdaria o conserto junto.

Está errado, e o erro tem uma forma que vale nomear: o conserto de lá não
estava na função, estava **na leitura da resposta dela**. `configuracao_json()`
sempre devolveu o valor vivo — o que a tela ganhou foi um campo *a mais* na
mesma resposta, e um pedaço de JavaScript que sabe procurá-lo. Quem chama a
mesma função e não sabe do campo extra recebe a resposta com o conserto dentro
e o descarta.

Foi também um caso de «conserto entra no caminho que o motivou, e o caminho
irmão fica» pelo avesso: aqui o caminho irmão **não existia** quando o conserto
foi feito. Ele nasceu depois, e nasceu velho.

## 3. O que a medição disse

Achado **exercitando**, não lendo — e o `SHOW` de antes passava por toda a
bateria unitária, porque nenhum teste perguntava «o que o `SHOW` responde
*depois* de um `ALTER` que exige reinício?».

| | antes | depois |
|---|---|---|
| `ALTER SERVER SET timeout_s = 45` → `SHOW` | `valor: 30` (só) | `valor: 30`, `no_arquivo: 45`, `esperando_reinicio: true` |
| `ALTER SERVER SET max_linhas = 500` → `SHOW` | `valor: 500` | `valor: 500`, **sem** `no_arquivo` |

A segunda linha é o controle positivo, e ela é metade da guarda: se o campo que
aplica a quente também ganhasse o par, «esperando reinício» não significaria
nada. Dos 49 campos graváveis, **17 aplicam a quente e 32 esperam reinício** —
ou seja, dois terços da tela de diretivas estavam sujeitos ao defeito.

## 4. A regra

**Quando uma porta nova reusa a função de uma porta antiga, procure os
consertos que moram na LEITURA da resposta, e não dentro da função** — eles não
viajam com a chamada, e não há teste que acuse: a porta antiga continua verde.

## 5. Como está guardado hoje

- `diretivas_do_servidor()` em `crates/phxsql-server/src/servidor.rs` lê o bloco
  `no_arquivo` da mesma resposta e o distribui campo a campo, com o comentário
  dizendo por que ele existe.
- Teste `o_show_diz_o_que_esta_gravado_e_ainda_nao_vale`, com o controle
  positivo do campo a quente no mesmo corpo. Prova real: reposto o defeito
  (voltar a montar o campo só com `valor_em(&vivo, …)`), ele falha.
- Bancada `bancada/diretivas/sql.py`, parte 2, com as duas afirmações lado a
  lado — e é ela que pega o dia em que a tela mudar o nome do bloco.

**Onde o buraco ficou:** não há guarda genérica que ligue «resposta que a tela
enriquece» a «toda porta que consome a mesma resposta». Se amanhã entrar um
segundo campo de enriquecimento no `configuracao_json()` — um `esperando_algo`
qualquer —, o `SHOW` voltará a descartá-lo em silêncio, e nenhum teste do lado
da tela vai acusar. O que existe hoje é o hábito escrito aqui.
