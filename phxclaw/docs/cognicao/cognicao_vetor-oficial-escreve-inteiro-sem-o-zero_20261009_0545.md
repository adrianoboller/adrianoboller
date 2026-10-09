# Vetor oficial escreve inteiro sem o zero: o `.rsp` do NIST não é sequência de bytes

**Estado:** PENDENTE (a evidência existe na árvore, falta o commit onde a prova roda)

**Evidência:** `crates/phxclaw-agent/tests/dados/rsa/extrair.py` (o conserto, com o motivo no
comentário) e `crates/phxclaw-agent/tests/canais.rs::rsa_pkcs1_sha256_contra_o_nist_sigver15`.
Sem o zero reposto, o leitor de hexadecimal do teste quebra no primeiro caso (`end byte index 256
is out of bounds for string of length 255`); com ele, 9 válidos aceitos e 45 inválidos recusados
nos 54 casos SHA-256.

## O que aconteceu

O `SigVer15_186-3.rsp` do NIST CAVP (pacote `186-3rsatestvectors.zip`) traz `S`, `Msg` e `e` em
hexadecimal. Lido como sequência de bytes, ele não fecha: dos 54 casos SHA-256, as assinaturas de
4 têm menos dígitos que o módulo pede (255 onde cabem 256, em 1024 bits), 4 mensagens têm 255
ou 254 dígitos (as outras 50 têm 256) e 6 expoentes têm número ímpar de dígitos.

## O que eu concluí primeiro, e estava errado

Que o arquivo do NIST seria, como o do Wycheproof, octetos escritos em hexadecimal — os dois são
«vetor oficial de assinatura RSA», e o do Wycheproof tem sempre `sig` de 512 dígitos num módulo
de 2048. Não é: o `.rsp` escreve **inteiros**, e o zero à esquerda some. O primeiro conserto
(repor só no `S`) ainda quebrou na mensagem, e o segundo, no expoente: o mesmo defeito, um campo
por vez.

## O que a medição disse

Repondo o zero até k bytes no `S` (I2OSP, RFC 8017 §8.2.2 passo 1), até 128 bytes na `Msg` (a
mensagem do SigVer do CAVS) e até número par de dígitos em `n` e `e`, os 9 casos `P` passam e os
45 `F` recusam. A reposição não pode estar errada sem um `P` reprovar, e nenhum reprova.

## A regra

Vetor oficial se lê pelo **tipo** que a norma dele declara, não pela forma que o vizinho usa. O
conserto entra no extrator, uma vez, com o motivo escrito — e não no teste, onde cada leitor
teria de lembrar.

## Como está guardado hoje

`tests/dados/rsa/extrair.py` refaz os dois arquivos a partir dos originais; o cabeçalho do
`nist_sigver15_sha256.txt` diz que os zeros foram repostos e o SHA-256 do `.rsp` de origem.
