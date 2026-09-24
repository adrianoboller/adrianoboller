# PhxZip — o 7-Zip em Rust

Pedidos 450 e 454. Crates: `crates/phxzip` (motor, `no_std`),
`crates/phxzip-cmd` (o `phxzipcmd` de terminal), `crates/phxzip-web` (o
`phxzipweb`, porta 4000) e `crates/phxhash` (CRC-32 e
SHA-256 que saíram do `phxsql-core` para o motor rodar sem sistema
operacional; o core os reexporta, nenhum caminho de chamada mudou).

## 1. O que existe (24/09/2026)

| peça | estado | prova |
|---|---|---|
| leitor 7z (cabeçalho cru e codificado, sólido e não sólido, subfluxos, datas, atributos) | feito | abre 6 arquivos gravados pelo 7-Zip 23.01 (`tests/dados/`) |
| LZMA e LZMA2 — decodificador | feito | idem, e ida e volta |
| LZMA2 — codificador (dispersão de 4 bytes; nível 1–4 guloso, 5–9 análise ótima por preço) | feito | o `7z` abre o que ele grava; ver §3 |
| 7zAES (AES-256-CBC, chave por SHA-256 iterado 2^19) | feito, nos dois sentidos | FIPS-197 C.3, SP 800-38A F.2.5/F.2.6; o `7z` abre o nosso e nós o dele, com nomes cifrados |
| recusa nomeada: Deflate, Deflate64, BZip2, PPMd, BCJ/BCJ2 e filtros, ZipCrypto, Zstd | feito | arquivos BZip2, PPMd e Deflate do 7-Zip recusados pelo nome |
| zip-slip no motor (`Entrada::caminho`, `caminho_seguro`) | feito | caminho absoluto gravado pelo 7-Zip (`-spf`) recusado |
| tetos (`Limites`: pasta, entradas, cabeçalho; ciclos do 7zAES ≤ 24) | feito | teto da pasta recusa antes de alocar |
| `phxzipcmd` a / x / l / t, `-p-` pela entrada padrão | feito | extração recusa ligação simbólica e ligação já existente no destino |
| servidor web numa porta de socket (`phxzipweb`, 4000, login opcional) | feito | 5 testes pelo soquete; vídeo `testes-web/video-phxzip.mjs` |
| ligar ao `config.json` como `.phz` | **não feito** | etapa 2 do pedido 450, espera o 372 |

## 2. Alvos

| alvo | resultado nesta máquina |
|---|---|
| x86_64 Linux | **roda** — suíte e interoperabilidade |
| `armv7-unknown-linux-musleabihf` (ARM 32) | **roda** sob qemu-arm — suíte inteira, nenhum teste pulado, e ida e volta com sha256 igual |
| `s390x-unknown-linux-gnu` (big-endian) | **roda** sob qemu-s390x — suíte inteira e ida e volta |
| `x86_64-pc-windows-gnu` | **roda** sob wine — ida e volta com sha256 igual |
| `aarch64-linux-android` | **liga** (NDK r27c); não rodou aqui |
| macOS (aarch64, x86_64), iOS | **compila** a biblioteca; ligar exige o SDK da Apple |
| `thumbv7em-none-eabihf` (Cortex-M), `riscv32imc-unknown-none-elf` (ESP32-C3) | **compila**, sem SO |
| ESP32 clássico (Xtensa), AVR | **NÃO MEDIDO** — o Rust estável não tem esses alvos sem `-Z build-std` |

Medido por `bancada/phxzip/plataformas.sh`, 24/09/2026 09:23 UTC; o cru, com o
comando de cada alvo, está em `bancada/phxzip/resultados.json`.

## 3. Compressão, medida contra o 7-Zip

Mesmo conteúdo (1.619.709 bytes: `PENDENCIAS.md` + o próprio binário + três
pequenos), filtros desligados no 7-Zip (`-mf=off`) para os dois fazerem o
mesmo trabalho, um fio (`-mmt=1`). 24/09/2026, x86_64, release.

| nível | PhxZip | 7-Zip 23.01 | diferença | tempo PhxZip | tempo 7-Zip |
|---|---|---|---|---|---|
| 1 | 659.837 | 608.455 | +8,4% | 0,08 s | 0,12 s |
| 5 | 573.581 | 553.740 | +3,6% | 0,63 s | 0,54 s |
| 9 | 564.018 | 553.221 | +2,0% | 1,52 s | 0,50 s |

Descomprimir o mesmo arquivo: 0,04 s nos dois.

**Como se chegou aqui, hipótese por hipótese** (nível 5, mesma entrada):

| hipótese | resultado | veredito |
|---|---|---|
| guloso + preguiçoso, dispersão de 3 bytes (primeira versão) | 622.350 (+12,4%) | ponto de partida |
| análise ótima por preço (`otimo.rs`, o `GetOptimum` reescrito) | 586.061 (+5,8%), mesma velocidade do 7-Zip | **entrou** |
| refazer os preços a cada 256 bytes em vez de 2.048 | 585.626 (−0,07%), 38% mais lento | morreu |
| janela do plano de 4.096 nós em vez de 2.048 | 586.031 (−0,005%) | morreu |
| onde está o resto? texto sozinho +8,9%, binário sozinho +2,1% | a busca perde no texto repetitivo | diagnóstico |
| profundidade 128 no nível 5 | 569.262, 2× mais lento | caro demais |
| dispersão de 4 bytes (menos candidatos inúteis na cadeia) | 573.581 (+3,6%) e mais rápido em todo nível | **entrou** |

### 3b. A árvore binária (a ideia do `bt4`, reescrita) — a diferença fechou

Corpus congelado de 3.979.642 bytes (`PENDENCIAS.md`, `PHXZIP.md`, um PNG de
2,3 MB, o binário `phxzipcmd`, um texto com acento), com os mesmos parâmetros
acima. Foi a menor de 5 corridas, tempo de CPU, 24/09/2026:

| nível | antes | depois | 7-Zip 23.01 | depois × 7z | CPU antes → depois (7z) |
|---|---|---|---|---|---|
| 1 | 3.004.747 | 2.978.831 | 2.952.884 | +0,88% | 0,37 → 0,46 s (0,36) |
| 5 | 2.916.987 | 2.896.482 | 2.896.132 | **+0,012%** | 2,43 → 1,72 s (1,17) |
| 9 | 2.906.924 | 2.895.954 | 2.895.649 | **+0,011%** | 3,43 → 1,75 s (1,17) |

Só texto + binário (1.641.960 bytes, perto do corpus de cima):

| nível | depois | 7z | depois × 7z | CPU (7z) |
|---|---|---|---|---|
| 1 | 644.483 | 618.012 | +4,3% | 0,11 s (0,11) |
| 5 | 562.099 | 562.406 | **−0,05%** | 0,75 s (0,54) |
| 9 | 561.447 | 561.944 | **−0,09%** | 0,81 s (0,57) |

**Piora registrada:** no texto + binário o nível 5 ficou 21% mais lento
(0,62 → 0,75 s). O ganho de tempo no corpus inteiro vem do dado
incompressível. O `7z t` dá «Everything is Ok» nos três níveis.

| # | hipótese | número (bytes, níveis 5 / 9) | veredito |
|---|---|---|---|
| H1 | árvore binária nos níveis 5–9 | −13.315 / −4.143 | **entrou** |
| H2 | casamentos de 2 e 3 bytes com cabeças próprias | −3.746 / −3.744 | **entrou** |
| H3 | nível 1 com o trabalho do `7z -mx1` (256 KiB, prof. 16, «bom» 32) | −25.916; 0,37 → 0,46 s | **entrou** |
| H4 | cabeças da árvore acompanhando a janela | bytes iguais, 1,67 → 1,36 s | **entrou** |
| H7 | zerar só os nós tocados do plano | bytes iguais, 1,75 → 1,67 s | **entrou** |
| H9 | preços de comprimento numa passada | −5,5% de instruções | **entrou** |
| H10 | arestas compostas do `GetOptimum` | −2.599 / −2.610 | **entrou** |
| H11 | casamento ≥ «bom»: o plano termina ali | −504 / −23 | **entrou — era defeito** (cognição de 24/09 08:57) |
| H5 | profundidade 48 no nível 9 | +99 | morreu |
| H6 | casamentos curtos no guloso (níveis 1–4) | +767 / −383, mais lento | morreu |
| H8 | atalho de literal sem casamento | igual, sem ganho | morreu |

Memória: a árvore custa 8 B por byte de janela (a cadeia custa 4 B). As
cabeças da árvore custam 2 B por byte de janela, com teto de 16 MiB. As
cabeças curtas ficam em no máximo 512 KiB.

## 4. Decisões

- **7z, e não ZIP; só o conjunto atual** — decisão do dono (450, 454). O
  `zip.rs` do `phxsql-core` (ZIP/DEFLATE do backup) **fica onde está**: o 454
  fala do PhxZip, e o backup em ZIP é formato que o cliente abre sem instalar
  nada. Não é duplicação — é outro algoritmo respondendo outra pergunta.
- **Nomes cifrados por padrão** quando há senha: sem legado a proteger, e sem
  isso `senhas-da-producao.txt` vaza pelo nome. Quem quer nomes visíveis pede
  (`--nomes-visiveis`).
- **A biblioteca não sorteia o IV**: quem chama dá 32 bytes de acaso
  (`Opcoes::acaso`). Em microcontrolador não há fonte comum, e um IV
  previsível não pode nascer calado. O `phxzipcmd` usa o `cifra::sortear` do
  PhxSql — o mesmo gerador, não um segundo.
- **«Senha errada ou arquivo corrompido»**, nunca só «senha errada»: o
  AES-CBC do 7z não tem etiqueta, os dois casos não se distinguem.

## 5. A porta web

```bash
cargo build --release -p phxzip-web
target/release/phxzipweb                                   # http://localhost:4000, sem login
PHXZIP_WEB_SENHA='...' target/release/phxzipweb --usuario adriano   # login exigido
```

- **Porta 4000** sai de uma constante só (`PORTA_PADRAO`); `--porta` troca.
  Nasce presa a `127.0.0.1`; `--endereco` abre, e é escolha escrita.
- **Login pedido, não imposto:** sem `--usuario`, ninguém digita senha. Com
  ele, toda rota de arquivo exige sessão (cookie `HttpOnly`,
  `SameSite=Strict`, 30 min). A senha do login vem de `PHXZIP_WEB_SENHA` ou
  `--senha-` (entrada padrão), nunca da linha de comando; o servidor guarda
  só o hash PBKDF2. Login errado custa 0,5 s.
- **CSRF:** toda rota POST exige `X-PhxZip: 1`.
- **Teto:** corpo acima de `--max-mib` (256) é 413 antes de reservar memória.
- O HTTP é o do `phxsql-server::http` — ganhou `ler_pedido_binario` e
  `responder_bytes`, e o `ler_pedido` de sempre passou a ser o mesmo leitor
  com o teto de 4 MiB: um motor só.
- **Não feito:** a tela tem PT e EN por chave, mas numa tabela própria — a
  ligação com a fábrica de idiomas do PhxSql (`idiomas.rs`) fica pendente.

### A tela: o que protege quem compacta (24/09/2026)

- **A senha se digita duas vezes.** Com uma letra errada, o arquivo fica trancado
  para sempre, e nem o 7-Zip o abre. O campo de repetição só aparece quando há
  senha. Há também um botão de olho que mostra os dois campos.
- **O teto é recusado antes de enviar.** O `/api/estado` passa a publicar o
  `max_corpo` da porta. A fila avisa quando passa dele, e o botão Compactar não
  envia nada. O 413 do servidor só chegaria depois do corpo inteiro. O teste
  pelo soquete cai quando o campo sai.
- **Conferido, não só gerado.** O `.7z` recém-gravado passa pelo mesmo
  `/api/testar` da tela de abrir, com a mesma senha, antes de ser entregue. Se a
  conferência falha, o arquivo não é baixado. Se ele passa do teto, a tela diz
  «não conferido».
- **Celular:** Nível e senha ficam alinhados pelo topo e, abaixo de 520 px, um
  embaixo do outro. Antes, a legenda da senha quebrava e desalinhava os campos.
- **Cada nível diz o que custa**, numa linha abaixo da escolha. São 15 chaves
  novas nos seis idiomas.

- **Progresso e Cancelar.** Todo pedido da tela sai por um caminho só
  (`api()`), sobre `XMLHttpRequest`: é ele que mede o envio e aborta no meio.
  A barra mostra a porcentagem do envio e do recebimento. O trabalho do
  servidor não tem porcentagem, e a barra anda sem número em vez de inventar
  um. O Cancelar garante que nada chega nem é baixado; o que o servidor já
  fazia termina lá e é descartado.
- **Extrair tudo** (`/api/extrair_tudo`) exige um envio só do `.7z`. A
  resposta tem o formato do pedido ao contrário: uma linha JSON com a lista e
  os bytes emendados. O corpo sai em partes, sem juntar tudo num `Vec`, porque
  o 7z guarda os tamanhos e o `Content-Length` se conhece antes de
  descompactar. O cabeçalho sai pelo mesmo motor do `http.rs`
  (`abrir_resposta_de_bytes`, que o `responder_bytes` passou a usar). O
  `testar` roda antes do cabeçalho, para senha errada sair como erro com
  código; o teste pelo soquete cai sem ele. Na tela, onde o navegador grava
  pasta (File System Access), a árvore sai inteira na pasta escolhida. Onde
  não grava, os arquivos baixam um a um, e a mensagem diz qual dos dois
  aconteceu. Ligação simbólica e caminho inseguro ficam de fora, e a tela os
  nomeia.
- **Tema claro:** segue o sistema, o botão troca e a escolha fica lembrada.
  Usa os tons do PhxSql e a regra da marca (laranja em `#C63C0A`). O contraste
  foi medido na página, nos dois temas, e o pior caso ficou em 5,28:1.
- **Árvore com ordenação.** A árvore nasce dos nomes, e a pasta que só existe
  no caminho vira nó sem baixar. As pastas vêm antes em cada nível, e cada
  pasta soma o tamanho e a data dos filhos. As colunas ordenam com
  `aria-sort`. Com filtro, o caminho do que casa fica aberto. Arquivos com mais
  de 200 entradas abrem só o primeiro nível.

O exercício que prova isso no navegador é `testes-web/tela-phxzip.mjs`: **55
verificações, todas OK**, a 1200 e a 375 px, nos dois temas e em alemão.
Achados do exercício:
- o aviso do teto aparecia cortado dentro da lista;
- o ícone da pasta ficava parado numa coluna própria enquanto o nome recuava,
  e a árvore não se lia;
- a zona de soltar encostava na legenda do campo seguinte.

O roteiro também errou duas vezes antes da página: esperava `leia.txt` antes
da pasta `sub`, e o Chromium sem janela anuncia sistema claro.

### O vídeo

`node testes-web/video-phxzip.mjs` grava 92 s contra o servidor de verdade:
o 7-Zip grava com AES e nomes cifrados → o PhxZip abre, testa e extrai
(sha256 igual); o PhxZip grava pela tela → o 7-Zip testa e extrai (`diff`
vazio); BZip2 recusado pelo nome; e a mesma porta com login (senha errada
recusada, certa entra, sair fecha). Os painéis de terminal mostram a saída
real dos comandos, capturada durante a gravação.

Filmar achou **três defeitos** que os testes não pegavam: o `setInputFiles`
do Playwright por caminho descarta calado o arquivo de nome acentuado (o `.7z`
saiu com 4 de 5 e só o `diff` do 7-Zip acusou — hoje os arquivos entram por
buffer e a contagem se confere); «(NaN%)» na compactação (porcentagem sobre o
número já formatado); e a recusa de método repetindo a frase duas vezes.

## 5b. Revisão completa (24/09/2026)

Revisão dos 18 commits da frente (cerca de 10 mil linhas): 8 achados, todos
consertados.

| # | achado | conserto | prova |
|---|---|---|---|
| 1 | Índice de ligação ou de fluxo empacotado igual ao total passava, e `p.coders[k]` estourava: um 7z hostil derrubava a thread da web, o `phxzipcmd` e o arranque do `phxsqld` com um `.phz` hostil | o índice tem de ser menor que o total, senão `Corrompido` | teste novo, que falha com o código antigo |
| 2 | `caminho_seguro` só recusava letra de unidade no 1º componente: `a/C:/x` passava, e no Windows o `PathBuf::push("C:")` troca o destino (zip-slip) | letra de unidade recusada em qualquer componente | casos no teste de recusa |
| 3 | `--deszipar-config`: a releitura passava pelo resolvedor, que acha o `.phz` irmão, e conferia o `.phz` com ele mesmo antes de apagá-lo | relê o `.json` pelo caminho literal | sem teste: reproduzir escrita ruim não é simples |
| 4 | `testar`/`extrair_tudo` custavam O(pastas × entradas) | faixa de entradas por pasta e deslocamentos calculados uma vez | 7z não sólido com 20.000 arquivos: `t` de 1,373 s para 0,028 s |
| 5 | A chave do 7zAES (2^19 SHA-256) era derivada duas vezes por gravação e duas por leitura | uma vez por gravação, cache por leitura | suíte e interoperabilidade verdes |
| 6 | Senha com o `acaso` do `Default` gravava IV zero, calado | `gravar` recusa senha sem acaso | o teste cai com a recusa desligada |
| 7 | O «extrair tudo» mandava o nome cru (`./a.txt`), e o `getDirectoryHandle('.')` quebrava no meio | manda o caminho normalizado | — |
| 8 | Comentário com acento | tirado | — |

Portões depois da revisão: `fmt` ok, `clippy` com zero avisos, 2.936 testes
verdes, e o `phxzip` compila para `thumbv7em-none-eabihf` (`no_std`).

## 6. Como rodar a prova

```bash
cargo test -p phxzip                       # vetores, fixtures do 7-Zip, tetos
cargo build --release -p phxzip-cmd        # target/release/phxzipcmd
cargo build -p phxzip --target thumbv7em-none-eabihf   # sem SO
```

O teste `o_7zip_abre_o_que_o_phxzip_grava` precisa do `7z` na máquina; sem
ele imprime **NÃO MEDIDO** em vez de passar calado.
