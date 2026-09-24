# O conserto que confere o resultado depois de o chamado já ter destruído a prova

**Estado:** PENDENTE

## O que aconteceu

Pedido 451, M4 da segunda revisão do DBA
(`docs/propostas/parecer-dba-451-2a-2026-09-24.md`). O reparo da trava completa
a marca do `COMMIT` que estava em voo. Se a marca já estava gravada e a
releitura falhasse por E/S (um `EMFILE`, um `EIO`), o `tratar_marca` tratava o
erro como «não confere». O `completar_marca` então apagava a marca, e o reparo,
que só conferia `impossiveis` e `paradas`, devolvia a trava ao serviço com a
transação confirmada pela metade.

O parecer propôs o conserto: `gravada: bool` na marca em voo e, no reparo,
`gravada && completadas == 0` vira falha (H5, `abort`).

## O que eu concluí primeiro, e estava errado

Que o conserto do parecer bastava, porque o processo passava a cair. Só que ele
cai **depois** do dano. Quando o `completar_marca` devolve o relatório, a marca
ilegível já saiu do disco. O arranque que viria salvar a transação não acha
bilhete nenhum. O `abort` troca «a trava serve a metade» por «o processo cai e
sobe com a metade», que é o mesmo dado errado com mais barulho.

É a lei do papel F («a conferência acontecia depois do dano»), só que aplicada
ao **conserto**, e não à prova. Conferir o veredito de uma função que destrói a
evidência antes de devolver não desfaz a destruição.

## O que a medição disse

Pelo processo filho, com a releitura injetada para falhar no `ler_marca`
(`falhar_a_proxima_leitura_de_teste`, só `cfg(test)`):

- sem conserto: «a marca em voo JA GRAVADA nao se releu, o reparo a tratou como
  «nao confere», e a trava voltou a atender com a transacao confirmada pela
  metade», e o `varrer` devolveu 6 linhas;
- só com o conserto do parecer (`gravada` sempre falso no `completar`, o
  `completadas == 0` de pé): «a marca confirmada SUMIU do disco ... left: 0
  right: 1»;
- com a política `NoArranque::Gravada` (a marca gravada que não se relê
  **fica**, e a falha vai para as impossíveis): o processo aborta, a marca fica,
  e o arranque completa `[1..5, 10, 11, 12]`.

## A regra

Quando um conserto reage ao resultado de uma função, pergunte o que a função já
fez **antes** de devolver. Se ela destrói a prova (apaga, sobrescreve, solta),
o conserto mora **dentro** dela, na decisão de destruir, e não depois, no
veredito.

## Como está guardado hoje

- `transacao::NoArranque::Gravada` e `completar_marca_em_voo`; o
  `completadas == 0` ficou como segunda trava no `reparo_da_trava`.
- Teste `marca_em_voo_que_nao_se_rele_derruba_o_processo_e_fica`, que confere o
  **dano** antes do diagnóstico: a marca no disco, e depois o arranque.
- Guarda `reparo-apaga-a-marca-gravada-que-nao-se-rele`.
- **O buraco:** o mesmo «erro de E/S vira não confere» continua no arranque e
  no braço de erro do 426 (N2-i do parecer). Lá o conserto não entrou.
