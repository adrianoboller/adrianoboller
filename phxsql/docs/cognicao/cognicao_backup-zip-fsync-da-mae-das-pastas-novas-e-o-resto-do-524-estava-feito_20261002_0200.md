# O que sobrava do 524 era uma pasta nova: o `fsync` do diretório do ZIP não alcançava a mãe

**Estado:** PENDENTE

## 1. O que aconteceu

O pedido 524 dizia que o `fsync` do DIRETÓRIO do backup «vai com o 467». O 467
já estava fechado e o motor (`sincronia::trocar_duravel_sem_abortar`) já era
chamado por `finalizar_zip`; a árvore já sincronizava pastas (579/593).
Medido sob `strace` nos dois caminhos: o `rename` do `.part` **tinha** `fsync`
da pasta do zip; o que faltava era a **mãe de cada pasta que a corrida criou**
(`novo/zips`: a entrada de `zips` mora em `novo`, a de `novo` em quem está
acima). Numa queda, a cadeia inteira, zip dentro, podia sumir depois do
«concluído». A árvore já cobria isso, sem prova contra o núcleo.

## 2. O que eu concluí primeiro, e estava errado

Que o item pedia trocar o `rename` do zip pelo ajudante do 467. O texto do
pedido estava velho: 467, 579 e 593 já o tinham feito. Medir antes de mexer
poupou reescrever o que existia — a falta era outra, e só o `strace` com
cadeia de pastas nova a mostrou.

## 3. O que a medição disse

- RED: `o_zip_sincroniza_o_diretorio_depois_do_rename` cai com
  «a entrada de `zips` ficou sem o fsync da mãe (`novo`)»; a primeira
  asserção (fsync da pasta do zip depois do `rename`) já passava.
- GREEN: `finalizar_zip` sincroniza `Nascida::mae()` por `sincronizar_pasta`
  (o mesmo ajudante da árvore), depois do `rename`, fora da trava.
- Árvore: `a_arvore_sincroniza_a_mae_de_cada_pasta_que_criou` já nasceu verde;
  a guarda `arvore-pasta-nova-sem-fsync-da-mae` cai com o laço removido.
- Guardas `zip-pasta-nova-sem-fsync-da-mae` e
  `arvore-pasta-nova-sem-fsync-da-mae`: PROVADAS (1/1 cairam cada).
- Catracas `alcancam-fsync-3` = 24 e `rede-ou-espera-2` = 11 inalteradas;
  `PISO_DAS_ENTRADAS` 726 -> 728.

## 4. A prevenção

Pedido cuja metade «vai com outro» se confere contra o outro quando ele
fecha: o resto pode ter sido feito, ou virado outra coisa. E `fsync` de
diretório se prova perguntando PARA ONDE aponta o descritor que recebeu o
`fsync`, uma vez por pasta que a corrida criou, não só pela pasta do arquivo.

## 5. Evidência para promover

Pendente: precisa do commit do integrador com `./portoes.sh` verde.
