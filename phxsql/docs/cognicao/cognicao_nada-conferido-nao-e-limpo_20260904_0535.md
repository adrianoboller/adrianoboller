# Nada conferido não é limpo — e quem achou foi a medição que deu zero

**04/09/2026, 05:35.** Descoberto medindo o pedido 175.

## 1. O que aconteceu

A decisão do dono sobre o pedido 175 era **medir antes de implementar**:
quantas tabelas desta base declaram chave estrangeira sem o índice que a
conferência exige? Se a resposta fosse zero, o pedido viraria documentação e
não código.

A primeira corrida apontou o `--example conferir-integridade` para
`bancada/profiler/srv-a`, que é onde eu achava que as tabelas estavam. Ele
respondeu:

```
bancada/profiler/srv-a: 0 tabela(s), 0 chave(s) declarada(s), 0 linha(s) conferida(s)

limpo: nenhuma violacao
```

E saiu **0**. As tabelas moram um nível abaixo, em `<servidor>/base/<banco>/`.
Um caminho **inexistente** dava exatamente a mesma resposta:

```
$ ./target/release/examples/conferir-integridade /tmp/nao-existe-6045
/tmp/nao-existe-6045: 0 tabela(s) [...]
limpo: nenhuma violacao   → saida 0
```

A causa está em `catalogo::tabelas_em`, e ela **não é um defeito lá**:

```rust
pub(crate) fn tabelas_em(diretorio: &Path) -> Result<Vec<String>> {
    if !diretorio.is_dir() {
        return Ok(Vec::new());
    }
```

Lista vazia é a resposta certa para quem abre uma `Instancia` que ainda não tem
pasta. Quem transforma isso em mentira é o `--example`, porque a saída 0 é o
que um **script de manutenção** lê.

## 2. O que eu concluí primeiro, e estava errado

Duas vezes, e a segunda é a que ensina.

**Primeiro:** «não há base nesta máquina» — a partir de um `find` por `*.psch`
que voltou vazio. Só que **`.psch` não é uma extensão que exista**: o esquema
(`PSCH`) é um cabeçalho **dentro do `.reg`**. Eu procurei um arquivo que não
existe e quase concluí a ausência da base a partir disso. As extensões reais
estão listadas em `catalogo.rs` (`EXTENSOES` e `EXTENSOES_TODAS`), e nenhuma
delas é `psch`.

**Depois:** com o caminho certo em mãos, li `0 tabela(s) [...] limpo` e ia
anotar «zero, e o pedido vira documentação». O que me fez olhar de novo foi a
discrepância aritmética: eu tinha contado **63 arquivos `.reg`** e o conferidor
dizia **0 tabelas** no mesmo lugar. Se os dois números tivessem concordado —
se eu tivesse apontado direto para o nível certo — o `limpo` teria passado, e
o defeito continuaria lá.

**Foi a minha primeira corrida ERRADA que achou o defeito.** Se eu tivesse
acertado o caminho de primeira, teria recebido um `limpo` verdadeiro e nunca
saberia que o falso existia.

## 3. O que a medição disse

O corpus desta máquina, contado:

| diretório | tabelas | chaves declaradas | violações |
|---|---:|---:|---:|
| `bancada/phxsql` | 1 | 0 | 0 |
| `bancada/profiler/srv-a/base/loja` | 30 | 0 | 0 |
| `bancada/profiler/srv-b/base/loja` | 30 | 0 | 0 |
| `bancada/profiler/srv-log/base/loja` | 1 | 0 | 0 |
| `bancada/profiler/srv-sonda/base/loja` | 1 | 0 | 0 |
| **total** | **63** | **0** | **0** |

**O número do pedido 175 é zero, e o zero não responde nada.** As 63 tabelas
são da bancada do profiler, que não declara chave estrangeira nenhuma (`grep`
por `declarar_fk`, `chaves_estrangeiras` e `estrangeira` em
`bancada/profiler/`: nenhuma ocorrência). *Zero-porque-tudo-está-indexado* e
*zero-porque-ninguém-declara-chave* são achados diferentes, e este é o segundo.

E a saída, medida nos três caminhos, antes e depois do conserto:

| caminho | antes | depois |
|---|---:|---:|
| nível errado (`srv-a`) | `limpo`, **0** | recado, **2** |
| caminho inexistente | `limpo`, **0** | recado, **2** |
| nível certo (`srv-a/base/loja`) | `limpo`, 0 | `limpo`, **0** |

O segundo achado saiu do mesmo lugar: a conferência de índice é **dos dois
lados**, e só um estava provado. `indice_que_falta_na_filha_e_falha_de_
estrutura` existia desde a sonda do pedido 171; `Falha::SemIndiceNaMae` estava
escrito e **nenhum teste o exercitava**. Prova real nos dois sentidos: com o
`saida.push(...SemIndiceNaMae)` reposto por um descarte, o teste novo
**falha** — e o teste da **filha passa nas duas rodadas**, que é a medida exata
do que ele não cobria.

## 4. A regra

**Nada conferido não é limpo: instrumento que não achou o que medir sai com
erro, nunca com aprovação.** E o corolário do processo: **medição que volta
vazia ainda ensina — sobre o instrumento**, e é ali que se olha antes de
anotar o zero.

Esta não é lei nova; é o **alcance** de uma que já existe. A pétrea diz *«teste
que passa por engano é pior que teste que falta»*, e a guarda dela cobria os
testes. Não cobria o **código de saída de um binário de manutenção**, que é
onde um `0` vale exatamente o mesmo que um teste verde — e mente para um script
em vez de mentir para uma pessoa.

## 5. Como está guardado hoje

* **O conserto ficou no `--example`, e não em `tabelas_em`** — mudar a função
  quebraria os chamadores para quem a lista vazia é a resposta certa. Hoje
  `r.tabelas == 0` imprime o recado (dizendo **onde** as tabelas ficam) e sai
  **2**, o mesmo código de «não deu para varrer», porque é o que é.
* **O lado da mãe tem teste:** `indice_que_falta_na_mae_e_falha_de_estrutura`,
  em `crates/phxsql-store/tests/verificador-de-consistencia.rs`.
* **Onde o buraco ficou:** os três caminhos do `--example` foram provados **à
  mão**, e não por bateria. Não há guarda que reprove quem devolver o `exit 0`
  ao caso vazio — o teste de biblioteca não alcança o `main` de um `example`.
  Está nomeado, e não feito.
* **O pedido 175 continua aberto**, pela mesma decisão que o mandou medir: o
  número tem de sair de uma base real, e a régua para tirá-lo está pronta e
  agora provada dos dois lados. `docs/INTEGRIDADE.md` §7.5.1.
