# Guarda vermelha que não reabre a tabela prova meio conserto

Data e hora da descoberta: **09/09/2026, 06:20 UTC**, na frente G3 do pedido
210 (cifra em repouso: tabela cujas únicas colunas marcadas são externas
nascia em claro).

## 1. O que aconteceu

O pedido 210 dizia, com o fonte na mão, onde estava o defeito: a condição que
liga o material do `.reg` (`crates/phxsql-store/src/reg.rs`, `RegFile::criar`)
lia `faixas.is_empty()`, e `faixas_pessoais` pula `Memo`/`Bin` de propósito.
Trocar essa condição por `esquema.tem_dado_pessoal()` é uma linha.

Fiz só essa linha e rodei a guarda vermelha
`coluna_externa_marcada_sozinha_nao_pode_ir_em_claro` (ignorada desde 05/09).
Ela **passou**: o `.memo` saiu selado, em 0,62 s. Acrescentei três linhas ao
teste — reabrir a tabela e ler a linha 1 — e ela **caiu**:

```
Corrompido("…/fichas.reg foi gravado com coluna cifrada e o esquema nao tem
nenhuma coluna marcada como dado pessoal: desmarcar a coluna nao decifra o
que ja esta gravado")
```

A conferência do `abrir` contra «desmarcar não decifra» (`reg.rs`, ~linha
446) lia a **mesma premissa** da condição trocada — «cifrado, logo há faixa
inline». Uma tabela só de externas tem material e nenhuma faixa: para essa
conferência ela era uma tabela cifrada que alguém desmarcou. Meio conserto
gravava certo e recusava reabrir, e a guarda, que só olhava o `.memo`, dizia
que estava tudo bem.

## 2. O que eu concluí primeiro, e estava errado

Que o irmão da condição era o **slot**: com material cifrado e zero faixas,
`montar_slot_com` selaria uma mensagem vazia e tentaria copiar uma etiqueta
de 16 bytes num rabo de zero (`Material::rabo(0)` é 0), e o primeiro
`inserir` estouraria com `copy_from_slice` de tamanhos diferentes. Cheguei a
escrever no plano: «o conserto só na condição estoura no montar_slot_com».

Não estoura. `Material::selar` devolve `Vec::new()` para mensagem vazia
(`cofre.rs`, linha 487), e `abrir` faz o mesmo: o slot já era coerente com
«cifrado sem faixa» por desenho do cofre, e não por sorte. O irmão de verdade
estava na **reabertura**, do outro lado do ciclo, numa conferência escrita
para proteger o caso oposto.

E um segundo erro, menor: julguei o `remarcar_dado_pessoal` fora do caso,
porque «ele não deriva Material». Ele não deriva, mas depende dele: não
recalcula as faixas nem o `slot_size`, e só não doía porque a conta do
`slot_size` recusava **toda** remarcação em tabela cifrada por acidente (não
contava o rabo da etiqueta). Com rabo zero a conta passa, e marcar `nome`
depois deixaria as faixas velhas na memória e a reabertura seguinte cairia em
«slot_size não bate». O conserto do 210, sozinho, abriria esse caminho.

## 3. O que a medição disse

| o que foi feito | resultado |
|---|---|
| guarda com o defeito, `--ignored` | `FAILED` na linha 628: `.memo` em claro, 0,64 s |
| só a condição do `criar` trocada | guarda **passa**, 0,62 s |
| idem, com reabertura e leitura no teste | `Corrompido(… desmarcar a coluna nao decifra …)` |
| conserto inteiro (criar + abrir + remarcar), 15 testes | 15 verdes, 8,26 s |
| condição do `criar` reposta | caem 3 (a guarda no `.memo`, o `marcar` depois, o `acrescentar` inline), seguem 12 |
| conferência do `abrir` reposta | caem os **mesmos 3**, todos na reabertura (`Table::abrir(...).unwrap()`) |
| guarda do `remarcar` reposta | cai 1: o teste dela, na mensagem da recusa |
| executor do catálogo, `--so coluna-externa-sozinha` | `PROVADA`, 3/3 caíram, árvore limpa verde |

O número que decide: **dois** lugares liam a premissa, e a guarda vermelha
exercitava **um** ciclo de vida pela metade (gravar e olhar o disco), que é
exatamente a metade que o primeiro lugar cobre.

## 4. A regra

**Quando o conserto troca uma premissa, procure toda conferência que a lê —
inclusive a que protege o caso oposto — e faça a guarda percorrer o ciclo
inteiro: gravar, fechar, reabrir, ler, alterar.** Guarda que só olha o disco
depois de gravar prova o `criar` e cala sobre o `abrir`.

## 5. Como está guardado hoje

- A guarda `coluna_externa_marcada_sozinha_nao_pode_ir_em_claro` reabre a
  tabela, lê as dez linhas, altera uma, reabre e relê; e confere o formato
  contra uma gêmea criada sem cofre (versão 5, cabeçalho 192, slot **igual**).
- O `abrir` lê `Schema::tem_dado_pessoal`, com o comentário dizendo por que
  não é `faixas.is_empty()` e o que foi medido.
- `remarcar_dado_pessoal` recusa mudar o conjunto de colunas marcadas numa
  tabela cifrada, nomeando «reselar», e a conta do `slot_size` passou a contar
  o rabo — teste `marcar_coluna_depois_numa_tabela_cifrada_e_recusado_e_o_grau_pode_mudar`.
- Entrada `coluna-externa-sozinha-em-claro` no `bancada/guardas/catalogo.py`,
  com 3 `caem` e 5 `seguem`, provada pelo executor.
- `docs/FORMATO.md` (§1, versão 5 e §1.1) e `docs/SEGURANCA.md` §11.6 e §11.9.

**Onde o buraco ficou:** as tabelas que nasceram em claro pelo defeito
continuam em claro — não há recifragem, e é decisão (guarda nova entra
pedida). O que existe é o conferidor `--example tabela-marcada-nasceu-em-claro`
e o campo `material` da op `esquema`, para que a meia-verdade do
`cifra.ligada: true` tenha resposta por tabela. E o `acrescentar_coluna`
marcada em tabela nascida em claro **continua em claro**, nomeado e provado
como decisão, não consertado.
