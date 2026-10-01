# Derrubar um sistema de arquivos pelo CAMINHO derruba o do contêiner quando a montagem falha calada

**Estado:** INFRUTÍFERO
**Causa:** para medir o pedido 605 (o `fsync` de criar tabela protege um
terceiro?), a frente escreveu `bancada/catastrofes/terceiro-605.sh`: monta um
ext4 sobre loop e simula a queda com `FS_IOC_SHUTDOWN` +
`EXT4_GOING_FLAGS_NOLOGFLUSH` no ponto de montagem. O script não tinha `set -e`
nem conferia a montagem. A montagem falhou sem ninguém ver, o ponto de
montagem continuou sendo um diretório comum do `/dev/vda`, e o ioctl derrubou
o ext4 RAIZ do contêiner sem descarregar o cache. Todo `open` passou a
devolver EIO, para todas as frentes e para o integrador, até o contêiner ser
reiniciado.
**Prevenção:** o alvo de todo ioctl destrutivo se confere pelo DISPOSITIVO,
não pelo caminho: `st_dev` do alvo diferente do `st_dev` de `/` e igual ao do
loop recém-montado, e `mountpoint -q` antes de cada derrubada, com `set -e`.
Sem essas três conferências o script não roda. Melhor ainda: queda simulada só
em VM descartável. O script ficou DESARMADO (primeira linha sai com 99) no
ramo da frente, e não se integra sem as conferências.

## O que aconteceu

Três frentes rodavam em paralelo (601+603, 605+615, 616). A do 605 rodou o
script; as três pararam com EIO no mesmo minuto. Nada comitado se perdeu (o
`origin` estava em `aca24c08`); depois do reinício o disco voltou inteiro:
a edição do 616 (59+/29-) sobreviveu, e o `salvar-frentes.sh` a gravou em
`refs/salvas` no primeiro minuto.

## O que eu concluí primeiro, e estava errado

Que o `queda.py` da casa, que já usa o mesmo ioctl, provava que o recurso era
seguro aqui. Ele é seguro porque confere a montagem; o perigo nunca foi o
ioctl, foi o caminho que deixa de apontar para onde se pensa quando o passo
anterior falha calado.

## O número

1 contêiner derrubado, 3 frentes paradas, 0 commits perdidos, ~5 s de diário
em risco (não medido o que de fato se perdeu: a edição do 616 voltou inteira).
