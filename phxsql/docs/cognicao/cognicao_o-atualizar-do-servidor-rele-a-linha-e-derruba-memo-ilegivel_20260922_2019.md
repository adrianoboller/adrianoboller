# O `atualizar` do servidor relê a linha, e com ela o `.memo` ilegível

Pedido 375 (o bit `antes_indisponivel` no JSON da op `trilha`). Descoberto em
22/09/2026, 20:19 UTC, escrevendo a prova real no nível do **servidor**.

## 1. O que aconteceu

O conserto do 375 é uma linha em `op_trilha`
(`crates/phxsql-server/src/servidor.rs:18826`): publicar
`("antes_indisponivel", Json::Bool(e.antes_indisponivel()))` ao lado dos dois
bits de redação que já saíam.

A prova real precisa de um evento **de verdade** com o bit ligado, e ele só
nasce de um bloco externo velho que não abre. O caminho é o mesmo do irmão da
loja (`crates/phxsql-store/tests/trilha-lgpd.rs:616`): estragar o `.memo` com a
tabela fechada e alterar a linha. No servidor isso é legítimo — ele abre e
fecha a tabela **dentro** de cada operação (`abrir_travada`), então entre duas
chamadas o arquivo está livre.

Só que o `atualizar` **não passou**:

```
Corrompido("CRC do bloco em /tmp/.../b/p.memo offset 64 nao confere")
```

A gravação inteira foi recusada. O evento nunca chegou à trilha, e o teste
morreu no preparo — exatamente a armadilha da cognição de 18/09
(`cognicao_prova-real-que-morre-no-preparo-nao-prova-o-caminho_20260918_1311.md`),
com a diferença de que aqui o preparo nem chegou a produzir vermelho na linha
certa: produziu um `unwrap` estourado.

## 2. O que eu concluí primeiro, e estava errado

Concluí que a leitura do externo velho subia o erro em vez de virar `None`, ou
seja: que o conserto do pedido 367 (`marcados_externos_antigos`, com o
`.ok()` que transforma a falha em `None`) não valia no caminho do servidor.

Estava errado, e errado de um jeito que teria levado a mexer no motor. O
`Table::atualizar` está certo: ele decodifica o payload antigo com
`decodificar(payload, false)`, que devolve `Null` para `Bin`/`Memo` e não toca
o `.memo`, e a única leitura do bloco velho é a do `.ok()`. Quem leu o `.memo`
com o erro subindo foi **o servidor, antes de chamar o motor**.

## 3. O que a medição disse

O culpado é o bloco de `op_atualizar` que preserva a coluna de sistema
(`servidor.rs:18437-18447`), o que entrou para impedir que um `atualizar` de
rotina **ressuscitasse** linha excluída:

```rust
if let Some(i) = t.esquema().coluna_softdeleted() {
    let veio = /* o pedido trouxe "softdeleted"? */;
    if !veio {
        if let Some(atual) = t.ler(rowid)? {   // <-- aqui
            linha[i] = atual[i].clone();
        }
    }
}
```

`Table::ler` decodifica a linha **inteira**, e inteira inclui o `Memo`. O `?`
sobe o `Corrompido`.

Medido, os dois lados, no mesmo teste e no mesmo `.memo` estragado:

| pedido `atualizar` | resultado |
|---|---|
| sem `"softdeleted"` | `Err(Corrompido("CRC do bloco ... offset 64 nao confere"))` — a gravação é recusada |
| com `"softdeleted": false` | grava, e a trilha recebe o evento com `FLAG_ANTES_INDISPONIVEL` |

Ou seja, **o alcance do conserto do 367 no servidor depende de quem chama**:
quem manda a coluna de sistema — a interface web manda, porque ela envia a
linha inteira — atravessa e ganha o registro de auditoria com a marca; quem a
omite recebe erro e não grava nada.

E há um efeito de segunda ordem que não é da trilha: **uma linha com `.memo`
corrompido não se consegue alterar** por um cliente que omita a coluna de
sistema, nem mesmo para gravar um valor novo que substituiria o bloco ruim. O
`t.ler` é o portão, e ele só existe para copiar **um booleano**.

## 4. A regra

**Antes de dizer que o motor não trata um erro, procure quem leu a mesma coisa
antes dele.** O `?` que derruba a operação pode estar numa leitura que a camada
de cima faz por outro motivo — aqui, ler a linha inteira para copiar um bit.

E o corolário da prova real: **o preparo que atravessa o servidor tem de montar
o pedido como o cliente real o monta.** Um campo que parece enfeite no JSON do
teste pode ser o que decide se o caminho sob prova chega a rodar.

## 5. Como está guardado hoje

- `crates/phxsql-server/src/servidor.rs`, módulo
  `testes_do_bit_indisponivel_na_trilha` —
  `o_json_da_trilha_distingue_antes_ilegivel_de_antes_vazio`. O `"softdeleted"`
  dos dois pedidos leva comentário dizendo **por que** está ali e qual é o erro
  exato que a ausência dele produz, para ninguém o limpar como ruído.
- A prova é nos dois sentidos, e o preparo é conferido **antes** do veredito:
  a asserção do texto `trilha::INDISPONIVEL` roda primeiro, para o vermelho do
  campo sob prova nunca ser confundido com um `.memo` que voltou a ser legível.
- Vermelho medido em 22/09/2026, duas reposições:
  1. linha do `op_trilha` retirada → `left: None / right: Some(true)`, na
     asserção do bit do evento ilegível;
  2. linha trocada por `Json::Bool(true)` cravado → `left: Some(true) /
     right: Some(false)`, na asserção do evento cujo valor velho era **nulo**.
     É esta que mata a implementação que publicaria `true` sempre — e é a que
     o teste da loja (`trilha.rs:1018-1024`) já trazia na sua camada.

**O buraco que ficou:** a recusa do `atualizar` numa linha com `.memo`
corrompido não tem pedido nem teste próprio. Não é defeito da trilha, é do
caminho de gravação, e quem decide se ela é o comportamento certo (recusar) ou
errado (deixar o usuário substituir o bloco ruim) é o DBA — não esta frente.
