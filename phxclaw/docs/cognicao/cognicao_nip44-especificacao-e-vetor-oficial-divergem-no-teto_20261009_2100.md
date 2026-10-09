# NIP-44: a especificação e o arquivo oficial de vetores divergem no teto do texto

**Estado:** PENDENTE (a evidência existe na árvore, falta o commit onde a prova roda)

**Evidência:** `crates/phxclaw-agent/src/canais/nip44.rs::tests::recusa_tamanho_de_texto_fora_dos_limites`
e `tests/dados/nostr/nip44.vectors.json` (sha256 `269ed0f6…5040`, o mesmo que o `44.md` publica).

## O que aconteceu

O `44.md` (master, lido em 09/10/2026) ganhou um prefixo estendido de 6 bytes: texto de 65.536
bytes ou mais passa a ser válido, até 2^32 − 1. O `nip44.vectors.json` que o mesmo documento
amarra pelo sha256 continua com `invalid.encrypt_msg_lengths = [0, 65536, 100000, 10000000]`:
65.536 **inválido**.

## O que eu concluí primeiro, e estava errado

Que o checksum casando provava que especificação e vetores diziam a mesma coisa. O checksum prova
só que o arquivo é o publicado; a regra nova entrou no texto e o arquivo ficou para trás.

## O que a medição disse

Implementar o estendido derruba 1 dos 4 casos de `encrypt_msg_lengths`; não implementar passa os
4. A própria especificação manda cada implementação impor o seu teto antes do base64.

## A regra

Teto da casa = 65.535 bytes (o do arquivo oficial), com o limite em base64 conferido antes de
decodificar (87.472 caracteres). Quando o arquivo de vetores ganhar os casos estendidos, o teto
se revê contra ele — não contra o texto.

## Como está guardado hoje

O cabeçalho do `nip44.rs` diz o teto e o motivo; o teste confere o sha256 do arquivo antes de
usar qualquer vetor.
