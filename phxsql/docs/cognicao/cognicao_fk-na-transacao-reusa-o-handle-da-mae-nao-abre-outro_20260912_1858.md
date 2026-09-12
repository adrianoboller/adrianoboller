# Cognição: a conferência de FK dentro da transação reusa o handle da mãe, não abre um segundo

- **Assunto:** P0 — o pai empilhado invisível à conferência de FK da filha
- **Descoberto:** 2026-09-12, ~18:58 (papéis B+C+F, fechando o P0)
- **Arquivos:** `crates/phxsql-store/src/table.rs` (`conferir_fks`),
  `crates/phxsql-server/src/servidor.rs` (`aplicar_conjunto`),
  `crates/phxsql-server/src/transacao.rs` (`completar`/`aplicar_uma`)

## 1. O que aconteceu

`BEGIN; INSERT pai; INSERT filha(-> pai); COMMIT` recusava a filha, com o pai
inserido na MESMA transação. A recusa vinha do `COMMIT`, não do empilhar:
`aplicar_conjunto` aplica o pai no handle do `.reg`/`.ndx` da mãe (na ordem, o
pai primeiro), e a filha, ao conferir a FK, abria a mãe num **segundo
descritor** (`Table::abrir` dentro de `conferir_fks`). A guarda de visibilidade
do `.ndx` — corretamente — recusa ler o índice de uma tabela que outro handle
está escrevendo. Erro medido, palavra por palavra:

```
fk_cliente: nao deu para conferir contra clientes agora -- a guarda de
visibilidade de .../clientes.ndx recusou responder, e o arquivo esta SAO
```

## 2. O que eu concluí primeiro, e estava errado

**Errei duas vezes, e as duas medidas desfizeram.**

- Primeiro achei que o buraco era no **empilhar** (o `empilhar` recusaria a
  filha). Medido: o `empilhar` **não confere FK nenhuma** — só unicidade,
  CHECK e gatilho BEFORE. A recusa nasce no `COMMIT` (`aplicar_conjunto`) e,
  numa queda, de novo na **recuperação** (`completar`/`aplicar_uma`), que têm o
  mesmo desenho de «um handle por tabela num mapa».

- Depois achei que bastava montar a **sobreposição** (o conjunto de escrita da
  transação) e aplicá-la a um handle NOVO da mãe: `buscar` já funde os
  `nascidos()`. Errado, e medido: o handle novo bate na guarda do `.ndx` sujo
  **antes** de fundir a sobreposição — `self.ndx.buscar(...)?` falha na primeira
  linha. Sobreposição num segundo descritor não resolve, porque o problema
  nunca foi dado que falta: é haver **dois handles** para a mesma tabela.

## 3. O que a medição disse

- O InnoDB nunca teve esse buraco porque nunca há dois objetos para a mesma
  tabela na transação — o pai empilhado **é** a página. A passada de commit e a
  recuperação já mantêm UM handle por tabela num `HashMap`; **reusar esse
  handle** é o mesmo desenho, e some a guarda de visibilidade (o próprio
  escritor lê o seu índice).
- A ordem preserva a pétrea sozinha, sem código extra: a passada aplica na
  ordem empilhada, então o pai só está visível se foi aplicado ANTES da filha.
  `p0_filha_antes_do_pai_no_commit_ainda_recusa` mede: filha antes do pai
  continua recusada (`INTEGRIDADE`) — o `DEFERRABLE` do PostgreSQL não entra.
- O portão é o `Option<&mut dyn MaesEmProgresso>`: fora de transação ninguém
  passa resolvedor, a mãe vem do disco como sempre, custo zero.
  `p0_fora_de_transacao_a_fk_le_o_disco` mede que nada muda.
- Prova vermelha nos dois caminhos: revertido o `inserir_com_maes` para
  `inserir` puro, `p0_pai_empilhado...` (commit) e
  `p0_recuperacao_completa_pai_e_filha...` (recuperação) **falham**; com o
  conserto, passam.

## 4. A regra

**Dentro da transação, a conferência de FK reusa o handle que a passada já abriu
para a mãe — nunca abre um segundo descritor. O portão é o resolvedor opcional:
sem transação, a mãe vem do disco.**

## 5. Como está guardado hoje

- `MaesEmProgresso` (trait em `table.rs`) + `inserir_com_maes`/`atualizar_com_maes`;
  `conferir_fks` virou `conferir_fks_com(valores, Option<resolvedor>)`, com
  `conferir_uma_fk` extraída para servir os dois caminhos (disco e emprestado).
- O resolvedor `MaesAbertas` (em `servidor.rs`, `pub(crate)`) empresta as
  entradas do mapa; a filha é RETIRADA do mapa enquanto grava e volta em
  seguida — é o que dá `&mut` à mãe sem conflito de empréstimo.
- Ligado nos dois lugares que têm o mapa: `aplicar_conjunto` (commit) e
  `completar`/`aplicar_uma` (recuperação). Fixar só o commit deixaria a
  recuperação de um commit pai+filha travada em `operacoes IMPOSSIVEIS`.
- **Onde o buraco fica:** o ACID-C (a cascata do `ao_alterar` escreve FORA do
  conjunto de escrita da transação) **continua aberto**. Medido:
  `acidc_a_cascata_entra_no_conjunto_de_escrita_da_transacao` (`#[ignore]`, prova
  vermelha) — o `COMMIT` responde `gravadas:1` com duas tabelas alteradas, e o
  read-your-own-writes não alcança a cascata. Ver `docs/ACID.md` §2.4/§3.3.
