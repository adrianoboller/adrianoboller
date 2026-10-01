# Família que delega custa um salto na régua por nome — e aprofundar tudo traz a porta comum de volta

## O que aconteceu

Pedido 633. A catraca `alcancam-fsync-2` do `bancada/concorrencia/mapa-da-trava.py`
desceu de 23 para 22 com o `4400d7fa` (pedido 632) sem melhora nenhuma: a
`op_excluir_fk` continua segurando a trava global sobre `fsync`, e a régua
deixou de vê-la. Conferido contra a árvore de antes (`git archive 4400d7fa^`):
23, e a única diferença era ela.

## O que eu concluí primeiro, e estava errado

A nota do 632 dizia «dois saltos além do corte», porque o `sync_all` direto
da reescrita foi parar em `regravar_esquema_caro -> regravar_esquema_fase_a ->
reescrever_volume`. **Era um salto, e por outro caminho.** O que a régua
alcança de novo é o caminho BARATO, `regravar_esquema -> sincronizar`, que já
estava no quinto elo: a família `sincronizar` tem 11 definições e 10 delas só
delegam (9 para outro `sincronizar`, 1 para `sincronizar_volume`). No último
salto a fração sai 1/11, e a seção nunca passa do `CERTO`. Antes do 632 a
`op_excluir_fk` era contada pelo `reescrever_volume` no mesmo elo, com fração
1/1, e a família nunca precisou ser atravessada.

A segunda conclusão errada era a do pedido: «aumentar a profundidade». Medido,
aprofundar TODAS as classes compra o `fsync` e traz ruído junto.

## O que a medição disse

Contra a base de 5 saltos (22 na catraca, 93 seções):

| Hipótese | `fsync` | seções novas no `fsync` | outras classes mudadas |
|---|---:|---|---:|
| H1 6 saltos para tudo | 24 | `op_excluir_fk`, `op_restaurar_backup` | 20 (leitura → escrita) |
| H1 7 saltos para tudo | 25 | + `reaplicar_diario_ate` | 26; `rede-ou-espera-2` 11 → 20 |
| H2 função que só delega não cobra salto | 22 | nenhuma | 17 |
| H3 nome de definição única não cobra salto | 27 | 5 | 39; `rede-ou-espera-2` 11 → 28 |
| H4 chamada ao próprio homônimo não cobra salto (uma vez) | 22 | nenhuma (`sincronizar` fica 10/11) | 0 |
| H5 6 saltos e porta comum reconhecida em todo elo | 24 | as mesmas 2 | 17 |
| **H6 6 saltos só para a durabilidade** | **24** | **as mesmas 2** | **0** |

As 20 leituras que viram escrita em H1 entram pela marca do separador
(`abrir_travada -> abrir_travada_com -> abrir_database ->
separador::exigir_migrado -> marcar_novo`): a porta comum do `abrir_database`
(28 seções, já herdada) entrando por outra primeira porta, que fica abaixo do
corte. Fato verdadeiro, mas que não distingue seção nenhuma — o ruído que a
`PORTA_COMUM` existe para tirar. As duas seções novas do `fsync` foram
conferidas à mão: o `regravar_esquema` barato sincroniza os volumes, e o
`confirmar` do restaurar troca o database e sincroniza com a trava na mão, de
propósito (pedidos 467/582). A terceira, em 7 saltos, ninguém conferiu.

Prova real: com `SALTOS_DURABILIDADE = 5` (régua velha reposta) a catraca mede
22 e a `op_excluir_fk` some do `--json`; a guarda 9 do `--autoteste` falha.
Com 6, 24 e a guarda passa.

## A regra

Antes de mexer na profundidade de uma régua por nome, meça quantas seções
mudam em CADA classe — e aprofunde só a pergunta que precisa, porque um salto
a mais para todas traz de volta a porta comum por uma porta lateral.

## Como está guardado hoje

`SALTOS_DURABILIDADE = 6` e `alcance_da_secao` no `mapa-da-trava.py`, com a
guarda 9 do `--autoteste` (a folga sai das constantes, e não de um número
digitado). A `alcancam-fsync-2` (teto 22) foi aposentada e nasceu a
`alcancam-fsync-3` no medido do dia, 24. **O buraco que ficou:** H1 mostrou
que a porta comum se mede pela PRIMEIRA função do caminho, e a mesma porta
entrando por outra primeira função escapa do corte. Isso vale também em 5
saltos (H5 tirou 5 seções de «escrita» na base) e não foi consertado aqui.
