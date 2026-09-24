# PhxZip — o 7-Zip em Rust

Pedidos 450 e 454. Crates: `crates/phxzip` (motor, `no_std`),
`crates/phxzip-cmd` (o `phxzipcmd` de terminal) e `crates/phxhash` (CRC-32 e
SHA-256 que saíram do `phxsql-core` para o motor rodar sem sistema
operacional; o core os reexporta, nenhum caminho de chamada mudou).

## 1. O que existe (24/09/2026)

| peça | estado | prova |
|---|---|---|
| leitor 7z (cabeçalho cru e codificado, sólido e não sólido, subfluxos, datas, atributos) | feito | abre 6 arquivos gravados pelo 7-Zip 23.01 (`tests/dados/`) |
| LZMA e LZMA2 — decodificador | feito | idem, e ida e volta |
| LZMA2 — codificador (cadeia de dispersão + preguiçoso de um passo) | feito, **sem análise ótima** | o `7z` abre o que ele grava; ver §3 |
| 7zAES (AES-256-CBC, chave por SHA-256 iterado 2^19) | feito, nos dois sentidos | FIPS-197 C.3, SP 800-38A F.2.5/F.2.6; o `7z` abre o nosso e nós o dele, com nomes cifrados |
| recusa nomeada: Deflate, Deflate64, BZip2, PPMd, BCJ/BCJ2 e filtros, ZipCrypto, Zstd | feito | arquivos BZip2, PPMd e Deflate do 7-Zip recusados pelo nome |
| zip-slip no motor (`Entrada::caminho`, `caminho_seguro`) | feito | caminho absoluto gravado pelo 7-Zip (`-spf`) recusado |
| tetos (`Limites`: pasta, entradas, cabeçalho; ciclos do 7zAES ≤ 24) | feito | teto da pasta recusa antes de alocar |
| `phxzipcmd` a / x / l / t, `-p-` pela entrada padrão | feito | extração recusa ligação simbólica e ligação já existente no destino |
| servidor web numa porta de socket | **não feito** | frente seguinte (pedido 454) |
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
| 1 | 687.728 | 608.455 | +13,0% | 0,06 s | 0,09 s |
| 5 | 622.350 | 553.740 | +12,4% | 0,20 s | 0,47 s |
| 9 | 593.287 | 553.221 | +7,2% | 2,11 s | 0,49 s |

Descomprimir o mesmo arquivo: 0,04 s nos dois.

**A diferença tem causa conhecida:** o codificador escolhe cada símbolo pelo
maior casamento, e o 7-Zip escolhe o caminho pelo **preço em bits**
(`GetOptimum` em `LzmaEnc.c`). E o nível 9 daqui é lento porque a
profundidade de cadeia 1024 paga sem o preço para guiá-la. Próxima frente do
codificador: análise ótima por preço.

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

## 5. Como rodar a prova

```bash
cargo test -p phxzip                       # vetores, fixtures do 7-Zip, tetos
cargo build --release -p phxzip-cmd        # target/release/phxzipcmd
cargo build -p phxzip --target thumbv7em-none-eabihf   # sem SO
```

O teste `o_7zip_abre_o_que_o_phxzip_grava` precisa do `7z` na máquina; sem
ele imprime **NÃO MEDIDO** em vez de passar calado.
