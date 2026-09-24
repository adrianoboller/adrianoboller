# 7-Zip — fonte de consulta

**Decisão do dono, 24/09/2026:** *«precisa ter acesso aos fontes do 7zip para
funções internas do phxsql ou phxmail ou Phxblockchain»*. Nasceu do pedido 450
(os JSON de configuração gravados como `.phz`, no formato 7z, escrito dentro do
PhxSql).

É consulta, **não dependência**: nada deste fonte compila junto do produto, e a
pétrea de zero dependências continua inteira. O método é o mesmo do SHA-256 e
do Cassandra (`docs/CASSANDRA.md`): ler, entender, reescrever contra as nossas
restrições e provar contra vetor oficial e contra a ferramenta de fora.

## 1. O que foi baixado

| | Versão | Commit | Onde |
|---|---|---|---|
| 7-Zip | 26.03 (tag `26.03`, 2026-09-04) | `0766b733fe3e06dd2a7f9a3cfbf2108ac73abd17` | `/home/user/ip7z/7zip` (fora do repositório) |

Como baixar de novo, em qualquer sessão: `./bancada/referencias/7zip.sh`. Ele
fixa a tag e o commit e **recusa** se o destino estiver noutra versão — citar
linha de uma versão lendo outra dá número de linha errado no documento.

## 2. A licença decide o que cada arquivo pode virar

Lido em `DOC/License.txt` e no cabeçalho de cada arquivo:

| parte | licença | o que pode virar aqui |
|---|---|---|
| `C/` com «Public domain» no cabeçalho — `LzmaDec.c`, `Lzma2Dec.c`, `LzmaEnc.c`, `7zArcIn.c`, `7zDec.c`, `Aes.c`, `Sha256.c`, `7zCrc.c`, entre outros | domínio público | pode ser lido e reescrito sem restrição jurídica. A regra da casa continua: reescrito contra as nossas restrições, nunca colado |
| `CPP/` sem aviso de licença — `CPP/7zip/Crypto/7zAes.cpp`, `MyAes.cpp` e o resto | GNU LGPL 2.1 (arquivo sem aviso é LGPL, diz a `License.txt`) | **só leitura**: entender o algoritmo e escrever o nosso. Colar código LGPL num repositório `MIT OR Apache-2.0` traz as obrigações da LGPL |
| `CPP/7zip/Compress/Rar*` | LGPL com a restrição do unRAR | fora do alcance: não se lê para reescrever |
| `C/ZstdDec.c`, `CPP/7zip/Compress/LzfseDecoder.cpp`, `C/Xxh64.c` | BSD | não usados hoje |

## 3. Onde está o que o pedido 450 precisa

| assunto | arquivo |
|---|---|
| o formato do arquivo 7z (assinatura, cabeçalhos, `EncodedHeader`) | `DOC/7zFormat.txt` |
| a lista de métodos e seus identificadores | `DOC/Methods.txt` |
| LZMA: a especificação | `DOC/lzma.txt` |
| LZMA e LZMA2: o decodificador | `C/LzmaDec.c`, `C/Lzma2Dec.c` (domínio público) |
| o leitor de referência do 7z em C | `C/7zArcIn.c`, `C/7zDec.c`, `DOC/7zC.txt` (domínio público) |
| AES | `C/Aes.c` (domínio público) — o nosso se confere contra FIPS-197 |
| 7zAES: derivação da chave (SHA-256 iterado) e o modo | `CPP/7zip/Crypto/7zAes.cpp` (LGPL — só leitura) |

Para o PhxMail e o Phxblockchain, os candidatos óbvios são os mesmos: LZMA
para compactar arquivo de correio e bloco, e o AES. Nenhum deles tem pedido
ainda; quando tiver, a premissa se mede antes, como manda a lei da casa.
