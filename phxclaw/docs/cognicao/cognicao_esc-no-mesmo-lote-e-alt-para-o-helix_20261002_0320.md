# ESC seguido de byte no mesmo lote é Alt+tecla para o Helix — o `:q!` nunca chegava

**Estado:** FRUTÍFERO

**Evidência:** `crates/phxclaw-agent/tests/ide_web.rs::o_websocket_do_terminal_recusa_token_errado_e_bash_e_abre_so_o_helix` (falhava com `"\u{1b}:q!\r"` num lote só: 3 mensagens e nenhum `encerrado` em 30 s; passa com o ESC mandado sozinho e 150 ms antes do resto); `tests/desktop/ui_paineis.mjs` confere os dois lotes do clique na trilha.

## O que aconteceu

A trilha do IDE manda um comando ao Helix (`:goto N`, `:open PASTA`) escrevendo bytes crus no
PTY: ESC para voltar ao modo normal e depois o comando. Mandado como UM texto
(`\x1b:goto 7\r`), o Helix não abria a linha de comando; o teste do `:q!` ficou 30 s sem o
`encerrado`.

## O que eu concluí primeiro, e estava errado

Que o Helix ainda não tinha subido quando as teclas chegaram (e o primeiro RED foi mesmo
isso). Corrigi a ordem — esperar o `NOR` na grade — e continuou falhando: a causa era outra.

## O que a medição disse

O crossterm (leitor de teclado do Helix) trata ESC seguido de um byte **no mesmo `read`**
como `Alt+<byte>`: `\x1b:` vira `Alt+:`, que não abre a linha de comando. Com o ESC num lote
e `:q!\r` 150 ms depois, o `encerrado` chega em 0,4 s.

## A regra

Quem escreve ESC num PTY para um programa de tela manda o ESC **sozinho** e o resto num lote
seguinte. Vale para a tela (`comandoHelix` em `assets/ide.js`, 80 ms) e para qualquer teste
que dirija o Helix por bytes.
