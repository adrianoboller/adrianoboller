# Apagar e recriar um arquivo devolve o MESMO inode: a prova de identidade por inode media a data

**Estado:** PENDENTE

**Descoberto em 07/10/2026, 16:37**, papel F, fechando o pedido 672 (a
conferência da FASE B do 661, `util::ainda_o_mesmo_temporario`, cinco
condições: regular, mesmo inode, um nome só, mesmo tamanho, mesma data).

## 1. O que aconteceu

`tests/migracao-da-cifra.rs:o_novo_trocado_entre_as_fases_e_recusado` dizia
provar a IDENTIDADE do `.novo`: apagava o arquivo e escrevia outro com o mesmo
conteúdo. O sistema de arquivos deu ao arquivo novo o **mesmo número de
inode** do que acabara de sair. A comparação de inode passava; quem recusava
era a comparação da DATA.

## 2. O que eu concluí primeiro, e estava errado

O pedido 672 (escrito lendo, não medindo) dizia que a prova «só troca o inode»
e que tirar o tamanho, o `um_nome_so` ou a data «deixa as três provas verdes».
Para o tamanho e o `um_nome_so`, certo. Para a data, errado: tirá-la derrubava
justamente a prova «do inode».

## 3. O que a medição disse

- Sem a comparação da data: `o_novo_trocado_entre_as_fases_e_recusado` cai
  (`unwrap_err` em `Ok(20)`).
- Sem a comparação do inode (`mesmo_arquivo`): todas as provas do 661 verdes.
- Prova refeita (o velho fica vivo com outro nome, a data reposta, e o teste
  confere `ino` diferente, `nlink` 1, tamanho e data iguais): sem o inode,
  cai; sem a data, verde. Cada uma das cinco condições tem hoje exatamente uma
  prova que cai sem ela.

## 4. A regra

**Prova que troca um arquivo para testar a identidade tem de manter o velho
vivo** (renomear, não apagar) **e igualar tudo o que não é a condição sob
prova** — senão o inode volta e a prova mede outra coisa.

## 5. Como está guardado hoje

As cinco entradas `fase-b-aceita-*` e `fase-b-segue-com-o-novo-que-nao-se-le`
no `bancada/guardas/catalogo.py`, uma por condição, com RED medido à mão.
Buraco que fica: a condição «regular» (`is_file`) não tem prova própria —
trocar o nome por um link simbólico muda também o inode do `lstat`, e nenhuma
troca isola só ela.
