# O irmão pode ser um ARQUIVO, e não uma função

*Descoberto em 07/09/2026, 01h30, ao ligar o índice de texto (`.fts`) à
tabela.*

## 1. O que aconteceu

O `.fts` é um `.ndx` por dentro: `FtsFile` embrulha `NdxFile` para não
reescrever a árvore B+. Herdou a máquina toda — e herdou junto a **marca de
«ficou para trás numa queda»**, que o `.ndx` levanta no cabeçalho antes da
primeira página suja ir ao disco.

Herdou a marca. Não herdou o conserto.

- `Table::indice_precisa_reconstruir()` perguntava só ao `self.ndx`.
- `Table::reindexar()` recriava só o `self.ndx`.
- E enquanto a marca estiver de pé, **toda** operação de índice recusa
  (`ndx.rs:887`) — então uma queda deixava a tabela **sem gravar nunca mais**,
  com o `reindexar` respondendo `Ok`.

O agravante, e é o que faz este caso valer um arquivo: a mensagem de erro do
próprio `.fts` já dizia

> `reconstrua o indice de texto com `reindexar``

— **uma ordem que o código não sabia cumprir.** Eu a escrevi horas antes, no
mesmo dia, e ela ficou verdadeira por um dia inteiro só na minha cabeça.

## 2. O que eu concluí primeiro, e estava errado

Duas vezes.

**Primeiro:** que o problema do `reconstruir_fts` chamado duas vezes era
**achar a mais** — a segunda passada acrescentaria a mesma chave e a busca
devolveria o rowid duas vezes, que é mentira sobre o dado. Escrevi o teste com
essa previsão no comentário.

**Segundo, e mais fundo:** que «caminho irmão» queria dizer *função irmã* — o
`recascatear` ao lado do `atualizar`, o `planejar_ao_alterar` ao lado do
`conferir_fks`. Foi assim nas três vezes de 03/09. Procurei irmão entre as
funções de `table.rs` e não achei nenhum, e quase dei a frente por fechada.

## 3. O que a medição disse

A prova real, com o defeito reposto:

| o que eu previa | o que o teste mediu |
|---|---|
| busca devolve o rowid **duas vezes** | `Duplicado("chave completa ja existe no indice")` |
| irmão é uma função ao lado | irmão era o **arquivo** `.fts`, três mil linhas adiante |

A árvore recusa a chave repetida, então **não havia mentira** — havia coisa
pior de outro jeito: `reconstruir_fts` deixava de ser idempotente, e quem ia
bater nessa recusa era o `reindexar`, em toda tabela que já tivesse uma linha.
A previsão errada teria feito eu consertar deduplicando a leitura, e o
`reindexar` continuaria quebrado.

E a segunda medição, o teste `a_queda_marca_o_fts_e_o_reindexar_tem_de_baixar_a_marca`,
mostrou uma armadilha dentro da armadilha: as **duas asserções de bandeira
passavam por engano**. `mem::forget` suja os dois arquivos, então
`indice_precisa_reconstruir()` dizia `true` pelo `.ndx`, e dizia `false` depois
do `reindexar` pelo mesmo `.ndx`. Só a asserção de **comportamento** — inserir
uma linha depois do `reindexar` — via o defeito. Teste que confere bandeira
confere o que o conserto mexe; teste que confere comportamento confere o que o
usuário sente.

## 4. A regra

**Procure o irmão também entre os ARQUIVOS: quem herda a maquinaria herda os
defeitos dela, e o conserto não vai junto.** E quando uma mensagem de erro dá
uma ORDEM, o mesmo commit prova que a ordem se cumpre.

## 5. Como está guardado hoje

Três consertos, e cada um com prova real nos dois sentidos em
`crates/phxsql-store/tests/indice-de-texto.rs`:

| guarda | o defeito que ela repõe |
|---|---|
| `a_queda_marca_o_fts_e_o_reindexar_tem_de_baixar_a_marca` | `reindexar` sem o `.fts`: a tabela não grava mais nada |
| `reconstruir_duas_vezes_nao_duplica` | `reconstruir_fts` sem recriar: `Duplicado` na segunda passada |
| `fts_corrompido_reconstroi_na_abertura_em_vez_de_derrubar_a_tabela` | `abrir` recusando: manda rodar o `reindexar` numa tabela que não abre |

E o buraco que **fica**: não há guarda genérica que pergunte «este arquivo
derivado herdou marca de recuperação sem herdar o reparo?». Ela não se escreve
por padrão de texto — a família de arquivos da tabela tem dez extensões, e só
o `.fts` embrulha outro membro. O que se pode fazer, e é o que ficou, é a
pergunta na hora certa: **ao embrulhar um arquivo em outro, liste o que o de
dentro exige de quem o mantém.**
