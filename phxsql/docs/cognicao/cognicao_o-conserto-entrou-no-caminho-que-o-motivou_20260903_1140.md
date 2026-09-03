# O conserto entrou no caminho que o motivou, e o caminho irmão ficou

**Descoberto em 03/09/2026, 11:40**, integrando as duas frentes da rodada.

## 1. O que aconteceu

`Table::atualizar` confere a árvore da cascata inteira antes de gravar
qualquer coisa — é o pedido 169, e ele existe porque a avó ia para o disco e a
mãe ficava para trás quando a neta tinha `restringir`.

`Table::recascatear`, o caminho que a **recuperação** usa, não conferia:

```rust
let passos = self.planejar_ao_alterar(antes, depois)?;
if passos.is_empty() { return Ok(()); }
self.aplicar_ao_alterar(passos)      // aplica sem conferir
```

Os dois são do mesmo dia e do mesmo assunto. `recascatear` é do pedido 168 e
nasceu **antes** de `conferir_a_arvore` existir; quando o 169 escreveu a
conferência, ela entrou no caminho que a motivara — o `atualizar` — e o irmão
ficou. Nenhum teste apontava para lá, e a suíte inteira estava verde.

## 2. O que eu concluí primeiro, e estava errado

Li o código e **dispensei o achado**: «na recuperação a mãe já está no disco,
então não há o que proteger recusando cedo — a recusa tardia é só mais
barulhenta». A frase é confortável e é falsa.

O que a conferência antecipada protege **não é a mãe**. São as **outras
filhas**. Com duas filhas no nível 1, aplicar sem conferir grava a primeira
inteira e só então desce na segunda e descobre que a neta dela restringe: meia
cascata, gravada, numa recuperação que ninguém assiste. Recusar antes manda o
caso inteiro para `operacoes IMPOSSIVEIS`, que é o comportamento que a §5.5.3
já descreve como o certo.

Eu quase fechei a integração com «é benigno» escrito, e o que me fez voltar foi
não conseguir escrever *por que* era benigno sem citar a mãe — e a mãe não era
o assunto.

## 3. O que a medição disse

Escrevi o teste antes do conserto. Com o defeito de pé:

```
assertion `left == right` failed: a recusa chegou DEPOIS de gravar a
  primeira filha: cascata pela metade
  left: Int(7)      <- gravada
 right: Int(1)
```

Com o conserto, 12 de 12 no arquivo, e a suíte em **1.542** (era 1.541).

Dois números que decidiram a forma do teste:

* **com UMA filha o defeito não aparece.** Cada `filha.atualizar` confere a
  própria sub-árvore, então numa cadeia mãe→filha→neta→bisneta a recusa chega
  antes de qualquer escrita. É preciso um **leque** no nível 1: uma filha que
  passa e outra que recusa;
* **a ordem não é sorteio.** `catalogo::tabelas_em` faz `nomes.sort()`, então
  `aaa_pedidos` é sempre planejada antes de `zzz_pedidos`. Sem isso o teste
  passaria em metade das corridas, e teste que passa por engano é pior que
  teste que falta — eu já ia escrevê-lo com duas filhas de nome qualquer.

## 4. A regra

**Conserto que nasce num caminho procura o caminho irmão no mesmo commit** — e
irmão aqui é quem chama as mesmas duas funções na mesma ordem, não quem tem
nome parecido. Quando o irmão é o caminho da recuperação, a busca é obrigatória:
ele roda quando ninguém está olhando.

## 5. Como está guardado hoje

Teste `a_recascata_recusa_antes_de_gravar_a_primeira_filha` em
`crates/phxsql-store/tests/cascata-ao-alterar.rs`, e a guarda
`recascata-sem-conferir-a-arvore` no catálogo — **provada**: reposto o defeito,
cai 1 de 1 e as duas de controle seguem passando.

O que **não** está guardado: nada procura, hoje, por «duas funções que deviam
chamar a mesma conferência e só uma chama». Este achado saiu de leitura na
integração, não de máquina. Fica escrito aqui como buraco, e não como resolvido.
