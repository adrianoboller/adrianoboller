# A drenagem que so roda quando ha trabalho nao roda quando o trabalho ja foi feito por outro

Descoberto em 16/09/2026, 10:26, pelo papel B ao ler o fonte para o pedido
254 — antes de medir, e confirmado medindo as 10:31.

## 1. O que aconteceu

O pedido 254 dizia: «`bulkinsert(false)` nao drena `marcas_pendentes`; o
conserto e chamar a mesma drenagem que `descarregar_sujas` faz». A drenagem
existe e e uma so, `descarregar_sujas_com` (`servidor.rs`): drena o conjunto
de sujas, sincroniza cada tabela, e — quando nenhuma falhou — apaga todas as
marcas `.tx` pendentes. Ela comecava assim:

```rust
let lista: Vec<String> = match self.sujas.lock() { ... s.drain().collect() ... };
if lista.is_empty() {
    return;
}
```

Ou seja: **sem tabela suja, ela voltava antes de chegar as marcas.** E quem
chega nela sem tabela suja e justamente quem acabou de sincronizar a unica
tabela suja por conta propria e so precisa da limpeza — dois chamadores, que
chamam as mesmas funcoes na mesma ordem (sincronizar, tirar das sujas,
drenar):

* `op_bulkinsert(false)`: sincronizava a tabela reservada e a tirava das sujas
  — e **nao chamava** a drenagem. E o pedido 254 como escrito.
* `gravar_de_verdade`, o fecho da janela: sincroniza a propria tabela, a tira
  das sujas, e **chama** a drenagem — que voltava na linha acima quando a
  tabela era a unica suja. E o irmao, que o pedido nao via.

Nos dois, a marca de um commit ja duravel ficava no disco. E o relogio de
fundo nao a alcancava: ele tambem volta quando nao ha tabela suja
(`descarregar_sujas`), e durante uma reserva `janela.pendente()` nem sobe,
porque `tabela_reservada(p)` curto-circuita o `hora_de_gravar()`.

## 2. O que eu conclui primeiro, e estava errado

Que o conserto era o que o pedido dizia: acrescentar
`self.descarregar_sujas_com(&trava)` ao `op_bulkinsert(false)`. Aplicado ao
pe da letra, esse conserto **compila, e nao conserta nada**: depois do
`sujas.remove` a lista esta vazia, a drenagem volta na primeira linha, e a
marca fica exatamente como antes. «Chamar a mesma drenagem» era a receita
certa com uma premissa errada — a de que a drenagem drenava quando chamada.

E a segunda conclusao errada, menor: que o relogio de fundo acabaria
limpando a marca «na proxima janela». Nao acabaria — pelo mesmo `return`, e
porque a reserva nao conta operacoes na janela.

O que derrubou as duas foi procurar o irmao antes de consertar: ao perguntar
«quem mais tira tabela das sujas?», o fecho da janela apareceu fazendo a
mesma sequencia e **chamando** a drenagem — e mesmo assim deixando a marca.
Se ele chama e a marca fica, chamar nao basta.

## 3. O que a medicao disse

Instrumento: `bancada/tomada/marca-apos-bulkinsert.py`, que reusa a
`corrida()` e o `julgar_tx_em_bulk` da bancada da tomada (porta 7610, 800
`inserir` numa transacao dentro da reserva).

**Antes** (binario de `c0e45e8`, compilado 10:28:03Z):

* sem queda: marca
  `bancada/tomada/.base-da-prova/base/tomada/transacao_1789554685673.tx`
  gravada as 10:31:26.377Z pelo COMMIT, **ainda la** as 10:31:26.391Z (o
  «ok» do `bulkinsert(false)`) e as 10:31:26.692Z (300 ms depois);
* com a tomada chutada depois do «ok» final: **3/3**
  `APOS_BULKINSERT_FALSE_MARCA_REPORTADA`, arranque com `achadas 1 /
  completadas 1 / ja_aplicadas 800`.

O irmao, medido por teste unitario (`por_lote`, `lote_operacoes = 4`, uma
tabela: BEGIN, 1 `inserir`, COMMIT, 3 `inserir` soltos): a janela fecha, o
conjunto de sujas esvazia, e a marca **fica** —
`a_janela_que_fecha_numa_tabela_so_leva_a_marca_junto` caiu em
`servidor.rs:35115` com o defeito presente.

**Depois** (o `return` da lista vazia removido; `op_bulkinsert(false)` chama a
drenagem sob a mesma trava, depois do `remove`):

* sem queda: marca pendente depois do COMMIT (como tem de ser: a janela nao
  fecha na reserva) e **nenhuma** as 10:35:02.933Z (o «ok») nem as
  10:35:03.233Z;
* com queda depois do «ok»: **3/3** `APOS_BULKINSERT_FALSE_SEM_MARCA`,
  arranque calado;
* os dois testes passam.

**Defeito reposto** com o `troca` do catalogo (o mesmo que a guarda usa): a
marca `transacao_1789555020401.tx` de volta as 10:37:01.121Z e 300 ms depois.
A guarda repoe o defeito que a bancada ve, e nao outro.

## 4. A regra

**Condicione a limpeza ao estado de DEPOIS — «nada ficou sujo» —, nunca ao
trabalho que a chamada recebeu — «a lista veio vazia».** Quem chega com a
lista vazia e quem ja fez o trabalho e so precisa da limpeza; um `return` na
entrada o manda embora sem ela, e nenhum teste com duas tabelas sujas acusa.

## 5. Como esta guardado hoje

* `servidor::testes_transacoes::a_marca_do_commit_na_reserva_sai_no_bulkinsert_false`
  e `a_janela_que_fecha_numa_tabela_so_leva_a_marca_junto` — os dois caem
  com o defeito presente (medido antes do conserto) e passam com ele.
* Catalogo: `bulkinsert-false-nao-drena-a-marca` (derruba o primeiro; o
  segundo **segue**, porque e outro ponto) e `fecho-sem-suja-nao-drena-a-marca`
  (derruba os dois, porque o `bulkinsert(false)` consertado passa por esse
  fecho).
* A bancada da tomada deixou de tratar o ponto `tx_em_bulk` como linha
  informativa: marca depois do «ok» final e corrida invalida, e a conferencia
  «a marca fica pendente ate o bulkinsert(false) e sai nele» reprova.
* `bancada/tomada/marca-apos-bulkinsert.py` mede so este ponto, nos dois
  sentidos, em menos de um minuto.

E um efeito colateral medido ao provar as guardas: a primeira corrida do
`provar-guardas.py` reprovou a **arvore limpa** (1084/1086), com a suite do
workspace rodando ao lado — `a_marca_espera_o_fsync_e_so_entao_e_apagada`
caiu na primeira asercao porque a janela de 200 ms de fabrica fechou no
proprio commit (o relogio dela conta desde o nascimento do servidor, e criar
o banco e duas tabelas passou dos 200 ms sob carga: 2 quedas em 15 corridas
avulsas). Nao era o conserto — o caminho e o mesmo antes e depois —; era o
teste dependendo do relogio de parede. Ele passou a fixar a janela por
configuracao (1 h), como os testes novos ja faziam, e a segunda corrida das
guardas saiu 2/2 com a arvore limpa verde em 28,8 s.

**Onde o buraco ficou:** o invariante «marca pendente implica tabela suja» e
o que torna seguro o `return` do `descarregar_sujas` (o relogio de fundo)
quando nao ha suja. Depois do conserto nenhum caminho o quebra, mas nada o
afirma em tempo de execucao — quem o segura sao os dois testes acima. Um
caminho novo que tire tabela das sujas sem passar pela drenagem reabre o
defeito, e so aparece se tiver um teste que olhe o disco.
