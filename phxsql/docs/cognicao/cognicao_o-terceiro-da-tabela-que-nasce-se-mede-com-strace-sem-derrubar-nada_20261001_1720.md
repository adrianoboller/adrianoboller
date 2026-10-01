# O terceiro da tabela que nasce se mede com `strace`, sem derrubar nada

**Estado:** FRUTÍFERO

**Evidência:** `crates/phxsql-server/src/servidor.rs::o_terceiro_so_ouve_ok_depois_do_fsync_da_pasta_e_espera_fora_da_trava`; `crates/phxsql-store/src/catalogo.rs::a_tabela_que_nasce_so_abre_para_outro_depois_do_fsync`; `bancada/durabilidade/terceiro-605.py`

## O que aconteceu

Pedido 605. A cria a tabela sob a trava global e leva a pasta ao disco fora
dela (586/589); B, outra conexao, insere na tabela visivel e ouve «ok» antes
do `fsync` da pasta de A. No ext4 sem diario desta bancada, a queda leva a
tabela e a linha de B juntas. A tentativa anterior de medir isso derrubou o
`/` do conteiner (`cognicao_derrubar-o-fs-pelo-caminho-derruba-o-do-conteiner_20261001_0930.md`).

## O que eu concluí primeiro, e estava errado

Que provar a ORDEM das respostas exigia queda simulada, e que o
`-e inject` do `strace` atrasaria todo `fsync` do processo -- inclusive os de
B, o que tornaria a medicao inutil. Conferido antes de usar: `-P <pasta>`
restringe tambem a injecao. Com `-P d` e `delay_enter=700000`, o `fsync` do
arquivo `d/x` levou 0,001 s e o da pasta `d` levou 0,701 s.

E um segundo erro, do proprio medidor: as expressoes do traco nao previam o
`(DELAYED)` que o `strace` escreve entre o resultado e a duracao da chamada
atrasada, e a primeira corrida «nao achou fsync nenhum». Medidor que nao acha
o evento tem de PARAR dizendo isso, e parou.

## O que a medicao disse

`bancada/durabilidade/terceiro-605.py`, `fsync` da pasta de A atrasado 1,5 s:

| | fim do `fsync` da pasta de A | «ok» de B | veredito |
|---|---|---|---|
| antes (codigo de `7b099a22`) | 1.531-1.535 ms | 19-40 ms | 3/3 VERMELHO |
| depois (saida (c), nasce reservada) | 1.510-1.524 ms | 1.526-1.536 ms | 0/3 VERMELHO |

E o teste do servidor mede a outra metade: com a espera do `executar`
tirada, B espera dentro do `Table::abrir_com` segurando a trava, e a vizinha
C nao anda em 2 s (guarda `terceiro-espera-dentro-da-trava-605`, provada).

## A regra

Ordem de resposta entre conexoes se prova alargando a janela com
`strace -P <caminho> -e inject=<chamada>:delay_enter=` num processo nosso, e
comparando relogios -- nunca derrubando sistema de arquivos.

## Como está guardado hoje

Quatro guardas no `bancada/guardas/catalogo.py` (`*-605`), provadas 4/4 pelo
`provar-guardas.py`. O que NAO esta guardado: o `mapa-da-trava.py` nao enxerga
`Condvar::wait` (a agulha de «espera» e so `sleep`), entao a espera de
garantia DENTRO da trava -- a do nome que nao veio no campo `tabela` -- nao
aparece na catraca `rede-ou-espera`. E a perda em disco continua so
raciocinada do fonte do nucleo (M1); o M4 pede VM descartavel.
