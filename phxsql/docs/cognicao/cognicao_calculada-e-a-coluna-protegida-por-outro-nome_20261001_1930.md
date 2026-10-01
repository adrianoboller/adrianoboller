# A calculada é a coluna protegida por outro nome — e o preenchimento em lote promove o vazamento de linha a vazamento de tabela

**Estado:** FRUTÍFERO
**Evidência:** `crates/phxsql-store/tests/cifra-dos-dados.rs::calculada_sobre_externo_selado_nasce_marcada_e_nao_vaza_no_reg`; `crate::servidor::testes_direito_por_coluna::calculada_que_cita_coluna_negada_e_recusada_na_declaracao`; `crate::servidor::testes_direito_por_coluna::calculada_derivada_de_coluna_negada_nao_se_le`; `crates/phxsql-store/tests/acrescentar-coluna.rs::a_recusa_da_calculada_sobre_coluna_marcada_nao_diz_a_linha`

## O que aconteceu

O pedido 245 O2b passou a preencher a calculada nas linhas velhas dentro do
`acrescentar_coluna`. A revisão SEC bloqueou o merge (achado A1): `x = cpf`
por quem tem `cpf` negado copiava todo CPF para uma coluna legível; `copia =
obs` sobre o `.memo` selado gravava o texto do cofre em claro no `.reg`; e a
recusa «não se calcula na linha N» virava oráculo por rowid.

## O que eu concluí primeiro, e estava errado

Que o preenchimento era só semântica de dado: a convergência dos quatro
motores e o parecer do papel C diziam *o quê* gravar, e eu tratei a pergunta
como fechada. O papel C julgou garantia de dado; ninguém julgou que a
calculada **lê** colunas — e quem declara a expressão escolhe o que ela lê.
Antes do preenchimento o mesmo furo existia linha a linha (cada `atualizar`
copiava); o preenchimento só o fez caber num comando.

## O que a medição disse

Com o conserto desligado, cada um dos quatro testes acima cai: o segredo
aparece nos bytes do `.reg`, a declaração entra e o `varrer` devolve o
salário, a leitura devolve `x`, e a recusa diz «linha 2».

## A regra

Toda expressão de esquema é uma leitura: a coluna que ela produz herda a
marca do que lê, e quem não pode ler a origem não pode declarar nem ler a
derivada.

## Como está guardado hoje

`Column::herdar_marca_das_citadas` (core), `direito_coluna::expressao_cita_negada`
(um motor para a declaração e para a leitura), `Servidor::negadas_para_ler`,
`nao_se_calcula` no `table.rs`, e quatro guardas no catálogo. O buraco que
fica: `tapar_pedidos_salvos` não conhece as derivadas (`SEGURANCA.md` §40).
