# O upsert é um `atualizar` disfarçado — e a presença da coluna não é intenção

## 1. O que aconteceu

A revisão do motor de 09/09/2026 (achado A1, `p01_upsert_direito_coluna.py`)
provou pelo soquete que `inserir` com `se_existir: "atualizar"` **zerava** a
coluna que o usuário não altera: ana (não lê `salario`) e gil (lê, não altera)
mandavam a linha sem a coluna, e o salário 5000 virava `None` — pelo
protocolo, pelo SQL `ON CONFLICT DO UPDATE` e dentro de transação. O
`escrita_sob_direito_por_coluna` (`servidor.rs`) só repunha o gravado quando
`op == "atualizar"`, e o upsert grava a linha inteira pelo mesmo
`Table::atualizar` com `op == "inserir"`.

Na mesma manhã, a revisão de tela provou o irmão pelo lado oposto: um usuário
com **qualquer** regra de coluna não conseguia incluir nem salvar **nada** pela
ficha, mesmo mexendo só no permitido, porque a ficha manda a linha inteira com
a coluna que não leu como `null` — e o servidor lia a presença como pedido de
alteração e recusava a operação toda.

## 2. O que eu concluí primeiro, e estava errado

Duas vezes.

**Primeiro**, ao ler o código: que o A1 tinha duas saídas equivalentes — repor
o gravado, ou **recusar o upsert** para quem tem regra de coluna — e que a
segunda era mais barata e igualmente segura. Não era equivalente: recusar era
exatamente o que a ficha já sofria no `atualizar`, e teria trocado «zera
calado» por «não grava nunca». A regra que escolhe entre as duas já estava no
`CLAUDE.md` (*proteção que quebra todo cliente não é proteção, é estrago*); eu
a li como lei sobre o portão de permissão e não a apliquei ao direito por
coluna.

**Segundo**, ao implementar: que a nova regra bastava na função. O teste do
SQL caiu porque `UPDATE folha SET salario = 1` passou a **passar** mantendo o
gravado — certo — mas o envelope do `sql` (`resposta_do_dml`) copia campos da
resposta do passo **por lista de nomes**, e `colunas_mantidas` não estava na
lista: o SQL respondia `afetadas: 1` a uma coluna que não mudou. Campo novo
numa resposta tem de procurar quem re-embrulha a resposta por lista.

## 3. O que a medição disse

- Antes: `salario ficou None (era 5000)` para ana, gil, SQL e transação
  (`antes/saida/p01_upsert_direito_coluna.txt`, 4 FALHA). Depois: 6 OK; a
  única linha vermelha é o controle 3 («valor explícito recusa»), invertido
  de propósito pela decisão do dono.
- A ficha (`p01b_ficha_mantem.py`, 10 de 10): ana inclui com `salario: null`,
  salva mexendo só no nome com `null` e com `999`, e o gravado fica 5000 com
  `colunas_mantidas: ["salario"]`; gil devolvendo a linha como leu passa sem
  nada a dizer; bea (sem regra) grava 7000 como sempre.
- Unidade: 137 de 137 nos módulos tocados, inclusive o comportamento velho
  (`sem_colunas_no_cadastro_nada_muda`).

## 4. A regra

**Para a coluna que o usuário não altera, o servidor mantém o gravado e diz
que manteve — nunca zera, nunca recusa a operação pela presença dela.** E
quem decide isso olha a **ação** (grava por cima de linha que existe?), não
o nome da operação.

## 5. Como está guardado hoje

`escrita_sob_direito_por_coluna` devolve `(pedido, mantidas)`; o upsert acha
a linha pelo mesmo `upsert::escolher_indice` do motor; `resposta_do_dml`
copia `colunas_mantidas` sempre. Guardas `upsert-zera-a-coluna-negada` e
`presenca-da-coluna-negada-recusa-a-ficha` no catálogo, provadas contra o
defeito reposto. Documentado em `docs/SEGURANCA.md` §15, «A escrita». **O
buraco que ficou:** a prova de **tela** (a ficha de verdade, no navegador) é
da frente de front-end, não desta.
