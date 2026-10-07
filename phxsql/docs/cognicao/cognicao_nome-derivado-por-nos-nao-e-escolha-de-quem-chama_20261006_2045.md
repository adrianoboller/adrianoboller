# Nome derivado por nós não é «escolha de quem chama»: o `.retrato.part` seguia link

**Estado:** PENDENTE

## O que aconteceu

Pedido 651 (a) pedia uma prova adversa da árvore temporária do zip em duas
passadas (`<pasta>/<nome>.zip.retrato.part/`, `backup.rs`). O teste novo
`retrato_part_plantado_como_link_nao_leva_a_copia_para_fora` plantou o nome
como link para uma pasta-isca com `Z/cadastroClientes.reg` dentro: o backup
**reescreveu o arquivo da isca** com o conteúdo do banco (`REG-DA-ISCA` virou
`registros aqui`). O irmão `retrato_part_plantado_ja_cheio_nao_entra_no_zip`
plantou uma pasta de verdade com `Z/intruso.reg`: o intruso **entrou no zip**.

## O que eu concluí primeiro, e estava errado

Que o pedido era só confirmar cobertura: os arquivos de dentro já nascem pelo
`recriar_no_destino` (569, `create_new` + `nlink`/dono) e a travessia pelo
descritor da `Pasta` (568) não segue link. O comentário no `copiar_tudo`
dizia: «o proprio `arvore` e o unico nome que se segue: ele e escolha de quem
chama». Para a cópia em árvore isso é verdade; para o zip, o `arvore` é um
nome que **nós** derivamos (banco, admin, minuto) dentro de uma pasta onde
terceiros escrevem por desenho — previsível, e não escolhido por ninguém.

## O que a medição disse

Os dois testes FALHARAM no código de antes (o `.reg` da isca reescrito; o
intruso no zip) e passaram com a árvore nascendo por `mkdir` sem `-p` pelo
descritor da mãe, entrada sem seguir link, e a pasta aberta vazia e do dono do
processo. O link físico no nome de uma cópia já era recusado — a defesa do 569
estava certa; o furo era um nível acima, no diretório.

## A regra

Quando um comentário diz «este nome se segue porque é escolha de quem chama»,
confira se TODO chamador passa um nome escolhido — o nome que o próprio motor
deriva dentro de um destino alheio tem de nascer na corrida, não ser
aproveitado.

## Como está guardado hoje

Guarda `zip-retrato-part-aproveitado` (`bancada/guardas/catalogo.py`) e os
dois testes em `crates/phxsql-store/src/backup.rs`. O buraco que fica: a
conferência «vazia e do dono» depois do `mkdir` não tem teste (pede corrida
com outro uid), e é dita em `docs/SEGURANCA.md` §42.
