---
description: Escolhe entre opcoes de projeto com probabilidade, confere petrea e aplica a regua dos motores (PG 4, MariaDB 3, MySQL 2, SQLite 1)
argument-hint: "<decisao> : <opcao A> | <opcao B> | ..."
---

Use a skill `phxjev` e aplique o preset **escolher** em: $ARGUMENTS

1. Estado: o codigo tocado, o `CLAUDE.md` do projeto (as petreas) e, se a
   decisao e comportamento de banco, o que o help/fonte de PostgreSQL,
   MariaDB, MySQL e SQLite dizem — com a fonte de cada um.
2. `fere_petrea` (noul) por opcao, citando a petrea. `≥ 0,30` → a opcao sobe
   ao dono como **choque com petrea**, e nao concorre.
3. Se for comportamento de banco:
   - os tres maduros convergem → a opcao deles vence, **sem pergunta**;
   - divergem → soma dos pesos por opcao, **conta feita por comando**
     (`python3 -c`), nunca de cabeca; o numero vai na saida.
4. `melhor_opcao` (choice) sobre as que sobraram.
5. Empate pela secao 4 que a pesquisa nao desfaz → sobe ao dono nomeando qual
   das tres: choque com petrea, empate real ou produto.

Registre tambem a opcao perdedora, com a `p` e o motivo em uma linha.

Saida: JSON da secao 5 da skill passado ao `phxjev.py veredito`; mostre a saida dele sem editar.
