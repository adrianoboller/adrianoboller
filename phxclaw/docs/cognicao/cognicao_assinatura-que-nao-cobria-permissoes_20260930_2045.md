# A assinatura dos plugins não cobria permissões, rede nem entrypoint

**Estado:** FRUTÍFERO

**Evidência:** `phxclaw-plugin-registry`, teste
`permissao_ou_rede_adulteradas_quebram_a_assinatura_v2`: com a raiz v07 em formato V1 o
manifesto adulterado é **aceito**; em V2, recusado nas três adulterações (30/09). O Python
(`verify_bootstrap.py`) gera a mesma mensagem: 7 de 7 manifestos conferidos pelo `openssl`.

## O que aconteceu

Ao reassinar os builtin com a raiz nova, li o que a assinatura cobria: `uuid`, `name`,
`version` e o sha256 do artefato. Permissões, sandbox (rede, pastas de escrita),
`entrypoint` e dependências ficavam de fora. Editar o manifesto para dar rede a um plugin
mantinha a assinatura válida.

## O que eu concluí primeiro, e estava errado

Que reassinar fechava o portão. A falha de assinatura escondia duas coisas: dependências
na faixa 0.4 com os plugins na 0.5 (a quarentena só apareceu depois da assinatura
valer) e a V1 não cobrir o que o sistema aplica. E meu primeiro teste de adulteração do
`entrypoint` mexia num campo que não existe (`command`; o nome certo é `value`): o serde
o descartava, e o teste «provava» uma adulteração que não havia.

## O que a medição disse

A V2 assina o manifesto inteiro em forma canônica (JSON compacto de chaves ordenadas, a
assinatura vazia). O formato vale por signatário: a raiz v07 só aceita V2, senão as
assinaturas V1 feitas no mesmo dia, antes do achado, deixariam trocar permissões.

## A regra

Assinatura cobre **tudo o que o sistema aplica**, não só a identidade. E teste de
adulteração confere que a adulteração chegou ao objeto: campo inexistente não adultera.

## Como está guardado hoje

`signing_message_v2`, `manifesto_canonico` e `mensagem_do_formato` (o assinador e a
verificação usam a mesma); o teste das três adulterações; o portão
`builtin_plugin_signatures` da certificação passou a rodar esses testes em vez de um
texto fixo de bloqueio.
