# Campo de segredo novo nasce fora da lista por nome: só a varredura dos bytes o pegou

**Estado:** PENDENTE

- **Quando:** 2026-10-09, 16:40
- **Onde:** `crates/phxsql-server/src/segredos.rs` (`SEGREDOS`) e
  `crates/phxsql-server/tests/senha-de-execucao.rs`
- **Pedido:** 767, fatia P14

## O que aconteceu

A op `senha_execucao_definir` nasceu com dois campos de senha: `senha_execucao` e
`nova_senha_execucao`. O Profiler tapa segredo por **nome exato** (`segredos::SEGREDOS`),
e nenhum dos dois estava na lista. Resultado: o `perfil.txt` gravou as duas senhas em claro,
`{"senha_execucao":"LoginDaAna-q7w3",...}`. O `"senha"` do mesmo pedido saiu tapado,
porque esse nome estava na lista.

## O que eu concluí primeiro, e estava errado

Que qualquer campo com «senha» no nome seria tapado, porque o `phxsql_core::senha::nome_sigiloso`
procura por conteúdo (`contains("senha")`). Essa regra por conteúdo só vale para nome com
ponto (caminho de configuração) e para o par `campo`/`valor`. Para a chave comum de um pedido,
o que vale é o nome exato.

## O que a medição disse

O teste `a_senha_de_execucao_libera_a_sessao_e_nao_aparece_em_lugar_nenhum` varre os bytes de
todos os arquivos do diretório do servidor e de todas as respostas. Ele achou as duas senhas
no `perfil.txt`, em 2 linhas. Com os dois nomes na lista, achou 0. A régua do próprio
`segredos.rs` (`todo_parametro_com_cara_de_segredo_esta_na_lista`) também teria acusado,
porque os dois campos estão declarados no catálogo. Só que ela roda na suíte inteira, e a
varredura achou antes.

## A regra

Campo de senha novo entra em `SEGREDOS` no mesmo passo em que entra no catálogo. E toda op que
recebe senha ganha um teste que varre os BYTES dos arquivos, não um que confere um campo.

## Como está guardado hoje

Os dois nomes estão na lista, e a varredura roda na suíte
(`tests/senha-de-execucao.rs`). Isso guarda só esta op. Para uma op nova, a guarda continua sendo
a régua do catálogo. Ela não cobre campo que o `despachar` lê e o catálogo não declara.
