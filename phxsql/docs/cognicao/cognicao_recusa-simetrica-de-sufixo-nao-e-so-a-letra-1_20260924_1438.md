# Recusa simétrica de sufixo de balde não é «só a letra 1»

**Estado:** FRUTÍFERO

**Evidência:** `crates/phxsql-store/src/catalogo.rs::criar_recusa_sufixo_de_letra_da_particao_sem_perguntar_ao_disco`; `crates/phxsql-store/src/catalogo.rs::criar_recusa_ponto_no_nome_por_colidir_com_o_qualificado`. Validada pelo integrador, que nao e o autor do conserto, em 24/09/2026 na arvore exata: com as duas guardas novas de `exigir_nome_que_volta` neutralizadas, os dois testes caem (`catalogo.rs:2281` e `:2321`); com o conserto, os tres passam.

## O que aconteceu

Pedidos 506/507: estender `exigir_nome_que_volta` (a função ÚNICA das quatro
portas de declaração de nome de tabela) para recusar sufixo de LETRA da
partição alfanumérica, hoje só cobrindo dígitos. O parecer do papel C
(`docs/propostas/parecer-dba-368-expurgo-3a-2026-09-24.md`, N1) media o
defeito com o exemplo `x_A` sozinho e depois `x_B` «sem `x_A` ao lado»
poluindo a árvore.

## O que eu concluí primeiro, e estava errado

Escrevi o teste supondo que, depois do conserto, `criar_tabela("x_A")`
recusaria mas `criar_tabela("x_B")` continuaria **aceito** — porque o exemplo
do parecer descrevia exatamente esse caso como o efeito colateral do defeito
(`x_B` nascia do lado por não ter `_A` para a conferência por disco flagrar).
Escrevi `assert!(db.criar_tabela(None, esquema("x_B")).is_ok())` e só ao
reler a recomendação do DBA («recusando na declaração **qualquer** nome que
termine num sufixo que o catálogo trata como volume ou partição») percebi
que isso descrevia o comportamento **antes** do conserto, não o alvo dele: as
37 letras (`_A`–`_Z`, `_0`–`_9`, `_Outros`) têm de recusar TODAS, sem
condição — exatamente como o sufixo de dígitos já recusa qualquer número, não
só `_001`.

## O que a medição disse

Com a recusa sintática (`nome_termina_em_sufixo_de_volume`, que usa
`sufixo_e_de_volume` — a mesma fonte que `pertence` e `nome_da_tabela` já
usavam via `e_balde`/`BALDES`), `criar_tabela("x_A")` e `criar_tabela("x_B")`
recusam os DOIS, cada um sozinho, sem que um precise existir para o outro
recusar. Isso fecha a cascata do N1 na raiz: se `x_A` nunca nasce, `x_B`
nunca tem `_A` ao lado para se tornar ambíguo também, e o
`excluir_tabela("x")` de 16 arquivos do parecer nunca acontece — mas o motivo
real não é «a cascata quebrou», é «cada sufixo reservado recusa por si».

## A regra

Quando uma guarda estende de UM caso especial (dígito) para uma FAMÍLIA
(37 letras + dígitos), a extensão é para a família inteira, não para o
membro que apareceu no relato do defeito. O exemplo do parecer prova que o
defeito existe; não delimita o tamanho do conserto.

## Como está guardado hoje

No teste citado acima (verde) e nos comentários de
`nome_termina_em_sufixo_de_volume`/`exigir_nome_que_volta` em `catalogo.rs`,
que descrevem a recusa como incondicional para as 37 letras. Um segundo
achado do mesmo trabalho, sem cognição própria por ser o ALCANCE já
documentado pelo parecer (não pétrea nova): a guarda só fecha a PORTA DE
ENTRADA (`criar_tabela` e as três irmãs) — uma tabela `x_A` que chegou ao
disco por outro caminho (teste de baixo nível, migração, binário anterior)
continua abrindo por nome exato (`abrir_tabela`/`abrir_qualificada`), mas
`existe_tabela`/`todas_as_tabelas` continuam ambíguas para ela, porque essas
duas listam o diretório e chamam `nome_da_tabela`, que resolve a letra
perguntando ao disco — o que só o separador de volume do pedido 508
(mudança de formato) fecha por completo. Prova:
`crates/phxsql-store/src/catalogo.rs::testes_copia_entre_bancos::tabela_x_a_ja_existente_continua_abrindo`.
