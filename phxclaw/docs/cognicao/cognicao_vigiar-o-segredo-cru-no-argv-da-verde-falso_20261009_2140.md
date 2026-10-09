# Vigiar o segredo CRU no argv dá verde falso: o argv leva o cabeçalho pronto

**Estado:** PENDENTE

**Evidência (para quem for validar):** `crates/phxclaw-agent/tests/fluxo_git_rede.rs::push_e_pull_de_rede_com_credencial_do_broker`.
Com o cabeçalho da credencial passado ao git por `-c http.<url>.extraHeader=...` (argv, `// REPOSTO`)
e o servidor falso vigiando só o valor cru do segredo no `/proc/*/cmdline`, o teste PASSOU com o
defeito. Vigiando também o base64 do `Basic`, o mesmo defeito deu 57 processos com o segredo no argv
e o teste caiu; restaurado o `GIT_CONFIG_*` (ambiente), 0.

## O que aconteceu

A prova de «a credencial do push de rede não vai pelo argv» lia o `cmdline` de todo processo a
cada pedido que o git fazia ao servidor falso, procurando o token. O RED que devia derrubá-la
(cabeçalho por `-c`) não derrubou.

## O que eu concluí primeiro, e estava errado

Que a leitura do `/proc` não alcançava os processos do bwrap (outro espaço de PID). Alcança: o que
o argv carregava era `Authorization: Basic <base64(usuario:token)>` — o token cru nunca aparece ali.
A guarda procurava a forma que o segredo tem no broker, e o vazamento tinha a forma que ele tem no
fio.

## A regra

Guarda de «o segredo não aparece em X» vigia TODA forma que ele toma no caminho até X (cru, base64
do Basic, `Bearer <t>`, URL-codificado), e o RED tem de vazar pela forma real do caminho — senão a
guarda é verde por construção. É a mesma família do «teste que passa por engano», com o engano na
agulha e não no palheiro.

## Como está guardado hoje

`phxclaw_test_support::git_http::Servidor::subir(.., vigiar: Vec<String>)` recebe a lista de formas,
e o comentário do módulo diz para vigiar o valor e o base64. O teste de disco do mesmo arquivo
procura as duas formas também.
