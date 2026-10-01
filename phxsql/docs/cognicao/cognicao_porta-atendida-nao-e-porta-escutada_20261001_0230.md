# Porta que atende não é porta que o servidor escutou — pedidos 352 e 401

**Estado:** PENDENTE

Evidência candidata, para quem for promover:
`crates/phxsql-server/tests/porta-tomada-pelo-vizinho.rs::o_apoio_recusa_a_porta_que_nao_e_do_servidor`
e a guarda `apoio-engole-a-falha-do-bind` (PROVADA em 01/10/2026).

## O que aconteceu

`o_conflito_de_unicidade_para_o_par_marcado` (`tests/laco-do-unico-secundario.rs`)
caiu na suíte cheia com «database loja ja existe», e o
`cifra-das-portas-http` caiu no `Servidor::novo`. Os três arquivos que
ainda usavam `porta_livre()` (sortear, soltar, ligar depois) eram
`laco-do-unico-secundario.rs`, `cluster-cifrado.rs` e
`identidade-do-pulso.rs`, com 6 chamadas.

## O que eu concluí primeiro, e estava errado

O pedido 352 nasceu dizendo que o parceiro de replicação, em streaming de
1 s, criava `loja` antes do teste. A leitura do QA já tinha derrubado isso:
o `beta` não tem origem nenhuma, então ninguém replica para ele. A segunda
hipótese («corrida de porta») também estava incompleta: corrida de porta
sozinha daria erro de `bind`, e não «já existe».

## O que a medição disse

A montagem determinística (`o_padrao_velho_conversa_com_o_vizinho_e_ouve_loja_ja_existe`)
põe o vizinho já na porta pedida, com o mesmo token. O `bind` do servidor do
teste falha, e o `let _ = copia.escutar()` engole a falha. O `connect` do
`esperar_porta` passa, porque a porta atende, só que é a do vizinho. Assim, o
primeiro `criar_database loja` responde «ja existe», e
`porta_dos_dados()` do servidor do teste continua `None`. A mensagem do pedido
saiu igual, por construção e não por sorte. Foram necessárias **duas** metades
mentindo juntas: a falha engolida e a espera que confere quem atende, e não
quem escutou.

## A regra

**Para saber se um servidor subiu, pergunte ao servidor, não à porta.** A
espera confere `porta_dos_dados() == Some(porta)` e traz o `Err` do `escutar`
por canal. Quando o número precisa existir antes do servidor, o teste segura
o ouvinte (`comum::ouvinte_reservado`) e o entrega por
`Servidor::escutar_em`, e não sorteia nada.

## Como está guardado hoje

O apoio único está em `tests/comum/mod.rs` (`tentar_no_ar`,
`no_ar_no_ouvinte`, `ouvinte_reservado`). As 6 chamadas de `porta_livre()`
dos três arquivos foram trocadas, e não sobra nenhuma em `tests/`. A guarda
`apoio-engole-a-falha-do-bind` repõe a espera velha e derruba a prova.
**Ainda há um buraco:** dezenas de testes continuam com o
`let _ = copia.escutar()` próprio e esperam por `porta_real`. Ali não há
conversa cruzada, porque a porta lida é a do próprio servidor, mas a falha do
`bind` só aparece como «a porta não ficou disponível em 10 s», sem o motivo.
