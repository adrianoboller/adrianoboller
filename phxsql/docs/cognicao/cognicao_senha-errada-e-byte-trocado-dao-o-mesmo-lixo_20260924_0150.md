# Senha errada e byte trocado dão o mesmo lixo — e o formato só separa os dois se guardar o CRC do cifrado

**Descoberta:** 24/09/2026, 01:50, pedido 450 (PhxZip, `.phz`).

## 1. O que aconteceu

O contrato pedia erros **distintos** para «senha errada» e «conteúdo corrompido
(CRC)». O 7zAES é AES-256 em CBC **sem autenticação**: uma chave errada não é
recusada pela cifra — ela produz lixo do tamanho certo. Um byte trocado no dado
cifrado também produz lixo. Nos dois casos o que falha depois é o mesmo: o
LZMA2 que não fecha ou o CRC do conteúdo que não bate.

## 2. O que eu concluí primeiro, e estava errado

Primeiro: «CRC do conteúdo não bateu → `Corrompido`». Errado para arquivo
cifrado: com senha errada isso manda o operador procurar o backup quando o que
falta é a senha. Depois, já com o conserto, escrevi no teste que **todo byte**
do `.phz` do PhxZip estava debaixo de algum CRC que a senha não alcança — e o
teste exaustivo (cada byte trocado, um de cada vez) achou o byte 7: é a versão
**menor** do formato, que nenhum CRC cobre e que o 7-Zip não confere
(`C/7zArcIn.c:1489` só olha a maior). Trocado, o conteúdo sai igual.

## 3. O que a medição disse

- Arquivo gravado pelo **7-Zip 23.01**: o `PackInfo` não traz `kCRC` do dado
  cifrado. Senha errada nos três fixtures (`lzma2` com e sem cabeçalho cifrado,
  `lzma`) → o leitor só pode dizer `SENHA_ERRADA_OU_CORROMPIDO`.
- Arquivo gravado pelo **PhxZip**: o escritor preenche o `kCRC` do `PackInfo`
  (o formato prevê; o 7-Zip lê e ignora — `7z t` passa). Senha errada →
  `SENHA_ERRADA`; cada um dos 325 bytes trocado → nunca «senha», e o do meio do
  cifrado → `CORROMPIDO`. O único que passa é o byte 7, com o conteúdo idêntico.
- Guarda `phxzip-crc-do-cifrado`: sem conferir o CRC do cifrado, o byte trocado
  vira `SENHA_ERRADA` — **provada**.

## 4. A regra

Cifra sem autenticação não tem erro de senha: tem erro **depois** da decifração.
Só afirme «senha errada» quando um CRC do dado **cifrado** provar que o que está
gravado chegou intacto; sem ele, diga que não sabe qual dos dois.

## 5. Como está guardado hoje

`crates/phxzip/src/erro.rs` (as duas variantes e o porquê), `leitor.rs`
(`depois_da_cifra`), o teste `byte_trocado_em_qualquer_lugar_e_corrupcao_e_nao_senha`
e as guardas `phxzip-crc-do-cifrado` e `phxzip-crc-do-conteudo` no catálogo.
**Buraco que fica:** arquivo gravado pelo 7-Zip continua sem essa separação — é
do formato, não do leitor.
