# Cognição: quando uma frente LEVANTA uma limitação, a recusa não some — muda de motivo, e os testes que a guardavam não se apagam, se reescrevem

- **Assunto:** `UPDATE`/`DELETE` por faixa (item 1 do roteiro «SQL para nota 9»)
- **Descoberta:** 16/09/2026, 02:48
- **Arquivos:** `crates/phxsql-server/src/servidor.rs`
  (`executar_dml_por_faixa`), `crates/phxsql-sql/src/dml.rs`
  (`traduzir_atualizacao`/`traduzir_exclusao`, `tem_chave_unica`)

## 1. O que aconteceu

O `UPDATE`/`DELETE` só sabia descer até uma chave **única** de igualdade. Abrir
o caminho por faixa (colher os `rowid` que casam com `coletar_rowids`, aplicar
`ler`+`atualizar`/`excluir` por linha) fez **cinco testes que já existiam**
falharem de uma vez — não porque o código novo estava errado, mas porque cada um
**afirmava a recusa antiga**:

- no servidor, `indice_nao_unico_recusa_de_ponta_a_ponta` esperava «NAO e unico»;
- no tradutor, `indice_que_nao_e_unico_recusa_pelo_nome`,
  `sem_indice_nenhum_recusa_listando_os_unicos`, `chave_composta_nao_atende`,
  e duas entradas da lista de `o_que_falta_recusa_pelo_nome`
  (`WHERE id > 5` e `WHERE id <> 5`, que esperavam erro de parse com «por faixa»).

## 2. O que eu concluí primeiro, e estava errado

Vendo cinco testes vermelhos ao ligar a funcionalidade, o reflexo é «a régua da
casa é *catraca só desce* — não posso mexer nos testes que travam a recusa».
Isso está **errado por dois motivos**, e confundi os dois no começo:

1. Estes não são catracas (`TETO`). São testes de **comportamento** de uma
   limitação, e a limitação foi levantada **de propósito** — é o próprio item 1
   do roteiro. A régua de catraca não se aplica.
2. O segundo erro, mais sutil: pensei «então a proteção que eles davam some com
   eles». Não some. A recusa de «gravar sobre uma faixa que só vi o começo»
   continua — ela só **mudou de gatilho**.

## 3. O que a medição disse

- O que fazia os cinco falharem era **um** fato, medido com o defeito reposto no
  laço (`for rowid in rowids.into_iter().take(1)`): `UPDATE ... WHERE valor > 3`
  numa tabela de cinco dava `afetadas: 1` em vez de **2**, e só uma linha mudava.
  Revertido, `afetadas: 2` e as duas certas mudam — prova real nos dois sentidos.
- A proteção **não** se perdeu: onde antes a recusa era «esta coluna não tem
  chave única», agora é o **teto** do `coletar_rowids` (`TETO_COLETA_ROWIDS` =
  1.000.000). Uma faixa maior do que ele consegue prometer inteira é recusada
  **nomeando o limite**. A régua deixou de ser «tem índice?» e passou a ser
  «cabe inteiro?» — o mesmo princípio (recusar é melhor que trabalho pela
  metade), outro eixo.
- E a assimetria com o `SELECT` ficou medida e é **de desenho**: o `varrer` do
  `SELECT` **pagina**, então um `SELECT` filtrado sem índice responderia sobre a
  primeira página com cara de inteiro — por isso ele ainda recusa; o
  `coletar_rowids` **recusa acima do teto** em vez de paginar, então ele pode
  prometer a tabela inteira (até o teto) e por isso pode varrer o filtro que o
  `SELECT` não pode. Mesma casa, dois eixos, nenhuma contradição.

## 4. A regra

**Quando uma frente levanta uma limitação, ache onde a proteção daquela
limitação se reancora antes de apagar o teste que a guardava — e reescreva o
teste para provar o caminho novo, não o delete.** Teste vermelho ao ligar uma
funcionalidade nova é, na maioria das vezes, um teste que afirmava a ausência
dela; ele vira a melhor prova real do caminho novo, porque já conhece o dado.

## 5. Como está guardado hoje

Os cinco testes foram **reescritos**, não apagados, e cada um agora prova o
caminho por faixa: `update_por_faixa_no_indice_comum_alcanca_todas_as_que_casam`
e o irmão do `DELETE` (servidor), e no tradutor
`update_por_faixa_no_indice_comum_traduz_o_coletar`,
`delete_por_faixa_sem_indice_na_coluna_traduz_o_coletar`,
`desigualdade_vira_faixa_com_o_simbolo_do_comparador` e
`chave_composta_nao_atende_e_vira_faixa`. A recusa reancorada tem prova própria
em `testes_coletar_rowids::passar_do_teto_recusa_em_vez_de_truncar`. Documentado
em `docs/SQL.md` §6.1 e na assimetria com o `SELECT` na §5 («O que a op `sql`
ainda não faz»).
