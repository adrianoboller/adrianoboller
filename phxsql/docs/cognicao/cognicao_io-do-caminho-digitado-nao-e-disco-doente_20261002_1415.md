# Erro de E/S causado pelo caminho que o usuário digitou não é disco doente — e o gate de um aviso agora executa programa

**Estado:** PENDENTE

## 1. O que aconteceu

O gancho do `anotar` dispara o aviso de saúde do disco (e, desde o 249, o
programa do operador, que pode gastar SMS pago) por todo `PhxError::Io`
(5001). Pergunta do pedido 641: um usuário autenticado que digita um caminho
ruim também dispara? A pista era o `restaurar_backup` com `.zip` ilegível.

## 2. O que eu concluí primeiro, e estava errado

Duas hipóteses escritas antes de medir: (H1) o `Io` do caminho pedido dispara
o gancho; (H2) já não dispara, porque os ops com caminho pré-validam e viram
`NaoEncontrado`. Eu apostava na H2 pelo precedente: `conferir_backup` e
`restaurar_backup` já mapeiam «sem backup.json» para 3001. Estava
**parcialmente** errado: esses dois mapeiam; o `profiler_ligar` e o `backup`
não.

## 3. O que a medição disse

Pelo soquete, no código de antes: `profiler_ligar` com `arquivo` = diretório
(EISDIR) → `codigo 5001` e o gancho **executou** (`entrada_saida|profiler_ligar`);
`backup` com `destino` `/proc/nao-existe/x` (ENOENT) → 5001 também. H1
confirmada para dois dos cinco ops com caminho; os outros dois já voltavam 3001
e o `backups` devolve `ok:true`. Como root o EACCES do `.zip` não se
reproduz — a função que decide (`do_caminho_pedido`) tem teste direto.

## 4. A regra

Separe o `Io` do **caminho do pedido** do `Io` do **disco do banco** na origem
(converta no op que recebeu o caminho), nunca por lista de ops no gate: o
gate compara um código, e quem esquecer de converter vira alarme falso. Num op
que também lê o banco, só as formas de caminho/permissão (ENOENT, EACCES,
ENOTDIR) saem do alerta; EIO, ENOSPC, EROFS e EDQUOT continuam avisando.

## 5. Como está guardado hoje

`io_do_caminho_do_usuario_nao_avisa_o_disco` (soquete, com controle de que o
disco de verdade ainda dispara), `do_caminho_pedido_separa_os_dois_io` e duas
guardas (`io-do-caminho-pedido-avisa-o-disco`, `profiler-caminho-pedido-como-io`).
Buraco dito: destino cheio no meio da cópia ainda é `Io` (ENOSPC), e ENOENT de
um arquivo do banco durante o backup deixa de avisar.
