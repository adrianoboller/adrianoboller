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
| x86_64 Linux | roda — suíte e interoperabilidade |
| `thumbv7em-none-eabihf` (Arduino ARM Cortex-M, sem SO) | compila |
| `riscv32imc-unknown-none-elf` (ESP32-C3, sem SO) | compila |
| Windows, ARM 32, Android, macOS, iOS, s390x | **NÃO MEDIDO nesta rodada** — a crate é `no_std` sobre bytes; a prova alvo a alvo do pedido 450 ainda falta |

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

O que ainda falta para empatar, **não medido**: a árvore binária de busca
(`bt4` do 7-Zip) e os casamentos de 2 bytes por dispersão própria. O nível 9
é 3× mais lento que o do 7-Zip pelo mesmo motivo — a cadeia paga
profundidade que a árvore não paga.

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

## 6. Como rodar a prova

```bash
cargo test -p phxzip                       # vetores, fixtures do 7-Zip, tetos
cargo build --release -p phxzip-cmd        # target/release/phxzipcmd
cargo build -p phxzip --target thumbv7em-none-eabihf   # sem SO
```

O teste `o_7zip_abre_o_que_o_phxzip_grava` precisa do `7z` na máquina; sem
ele imprime **NÃO MEDIDO** em vez de passar calado.
