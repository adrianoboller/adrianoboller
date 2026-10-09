# Manual do PhxZip — PhxZipCmd e PhxZipWeb

Pedidos **454** e **455**, fatia **Z11** do `docs/propostas/plano-0.21.md`. Só em
português nesta versão, por decisão do dono (09/10/2026); a tela do PhxZipWeb já
fala os seis idiomas, e o PhxZipCmd também (`PHXZIP_IDIOMA`).

O PhxZip é o 7-Zip da casa, em Rust e sem dependência nenhuma: lê e grava o
formato **7z** com Copy, LZMA, LZMA2 e a cifra **7zAES** (AES-256), inclusive com
o cabeçalho cifrado. Dois programas usam o mesmo motor (`crates/phxzip`):

| programa | executável | para quem |
|---|---|---|
| **PhxZipCmd** | `phxzipcmd` | o terminal e os scripts |
| **PhxZipWeb** | `phxzipweb` | o navegador, só na própria máquina |

**Nenhum número deste manual está digitado.** Os tetos (§5) saem das constantes
do fonte e a bancada (§6) sai do `bancada/phxzip/resultados.json`; quem os
escreve aqui é o `python3 bancada/phxzip/manual.py`, e o
`python3 bancada/phxzip/manual.py --catraca` reprova o manual quando um deles
envelhece.

## 1. O que o PhxZip faz e o que não faz

- **Lê** 7z com Copy, LZMA, LZMA2 e 7zAES, sólido ou não, com cabeçalho claro,
  comprimido ou cifrado — o que o 7-Zip grava no padrão dele.
- **Grava** 7z com LZMA2 ou Copy (`--copia`), **um bloco por arquivo** (não
  sólido): extrair uma entrada de um arquivo grande não obriga a decodificar
  tudo o que vem antes dela. O preço é comprimir menos que o 7-Zip sólido — a
  §6 mostra quanto.
- O codificador LZMA2 é **guloso**, com um esforço só. O do 7-Zip é ótimo e
  comprime mais; a escolha é para o motor caber num microcontrolador
  (`crates/phxzip/src/lzma_compressor.rs`). Por isso não há `-mx1`…`-mx9`:
  oferecer níveis para um codificador só seria configuração que ninguém lê.
- **Não** lê ZIP nem RAR, e dentro de um 7z recusa Deflate, BZip2, PPMd e o
  filtro BCJ dizendo o nome do método.

## 2. Instalar

Os pacotes saem do `./empacotar-phxzip.sh`, nunca montados à mão, e se chamam
`phxzip-<versão>-<plataforma>.zip`. As plataformas são as mesmas do PhxSql:

| plataforma | pacote | binário |
|---|---|---|
| Linux x86-64 | `phxzip-<versão>-linux.zip` | ELF, glibc |
| Windows x86-64 | `phxzip-<versão>-windows.zip` | `.exe` (PE32+), sem DLL além das do Windows |
| ARM 64 bits (Raspberry Pi 3/4/5) | `phxzip-<versão>-arm64.zip` | ELF `musl`, estático |
| ARM 32 bits (Pi 2, Zero W, roteador) | `phxzip-<versão>-arm32.zip` | ELF `musl`, estático |

Cada pacote leva `phxzipcmd`, `phxzipweb`, este manual, `LICENCA.txt`, o
`fonte-exo2-OFL.txt` (a licença da fonte que o `phxzipweb` embute),
`COMECE-AQUI.txt` e o `MANIFESTO.sha256`. Não há instalador: são dois arquivos.

**Confira antes de rodar.** Fora do zip, o `SHA256SUMS` confere o download; dentro
dele, o `MANIFESTO.sha256` confere cada arquivo:

```bash
sha256sum -c SHA256SUMS                     # o zip chegou inteiro
unzip phxzip-<versão>-linux.zip
cd phxzip-<versão>-linux
sha256sum -c MANIFESTO.sha256               # nada mudou depois de montado
chmod +x phxzipcmd phxzipweb
sudo cp phxzipcmd phxzipweb /usr/local/bin/ # opcional
```

No Windows, o PowerShell confere: `Get-FileHash .\phxzipcmd.exe -Algorithm
SHA256`, comparado com a linha dele no manifesto. Quem tiver o `phxsql` (o
programa de linha de comando do PhxSql) confere a pasta inteira de uma vez —
byte trocado, arquivo a mais e arquivo faltando — com `phxsql conferir-pacote
<pasta>`.

**Plataforma que não montou não some.** Se o alvo de uma plataforma não estava
instalado na máquina que empacotou, sai um `phxzip-<versão>-<plataforma>.NAO-MONTADO.txt`
no lugar do zip, com o motivo e o comando para montar, e o empacotador termina
com código 3.

### 2.1 Montar os pacotes a partir dos fontes

```bash
./empacotar-phxzip.sh              # as quatro plataformas
./empacotar-phxzip.sh linux        # uma só: linux, windows, arm64 ou arm32
./empacotar-phxzip.sh conferir     # confere tudo o que está em pacotes/phxzip/
```

Antes de compilar ele confere a versão nos mesmos lugares que o `empacotar.sh`
do PhxSql (é a mesma função) e os números deste manual (`manual.py --catraca`).
A conferência olha a **forma** de cada binário (a arquitetura, pelo `file`),
o `SHA256SUMS` e, dentro de cada zip, o manifesto, pelo `phxsql conferir-pacote`:
**um byte trocado deixa a conferência vermelha** — a prova é o
`python3 bancada/pacote/provar-phxzip.py`.

## 3. PhxZipCmd — o terminal

```text
phxzipcmd listar    ARQUIVO.7z [--confiavel]
phxzipcmd testar    ARQUIVO.7z [--confiavel]
phxzipcmd extrair   ARQUIVO.7z [--destino PASTA] [--sobrescrever] [--confiavel]
phxzipcmd compactar ARQUIVO.7z CAMINHO... [--copia] [--sobrescrever]
                    [--cifrar [--nomes-claros] [--ciclos N]]
phxzipcmd --help | --version
```

Exemplos:

```bash
phxzipcmd compactar relatorios.7z relatorios/          # LZMA2, sem senha
phxzipcmd compactar copia.7z docs --cifrar < arquivo-da-senha
PHXZIP_SENHA='a senha' phxzipcmd extrair copia.7z --destino saida
phxzipcmd testar copia.7z                               # confere os CRC sem gravar nada
```

Código de saída: **0** deu certo, **1** a operação falhou, **2** uso errado.

### 3.1 A senha nunca vem por argumento

`--senha`, `--password`, `--pass`, `--pwd`, `--passphrase` e `-p` (o jeito do
7-Zip, `-pSEGREDO`) são **recusados**, em qualquer caixa e com `=` ou `:`
colados, antes de qualquer outra coisa — inclusive antes do `--help` —, e a
recusa não repete o valor. O motivo: o argumento aparece no `ps` de qualquer
usuário da máquina e fica no histórico do shell.

A senha vem, nesta ordem:

1. da variável `PHXZIP_SENHA` (o `environ` de um processo só o dono e o root
   leem — por isso a variável é aceita e o argumento não);
2. da entrada padrão: num terminal é perguntada **sem eco**; num cano, é a
   primeira linha, com o teto da §5.

Caminho que começa com hífen vai com `./` na frente (`./-planilhas`), senão
seria lido como opção — e `-p…` como senha por argumento.

### 3.2 A extração nunca sai da pasta e nunca segue link

- Nome com `..`, caminho absoluto, letra de unidade ou NUL é recusado
  (*zip-slip*), com `\` lido como separador. Nome de dispositivo do Windows
  (`CON`, `NUL`, `COM1`…) e componente terminado em ponto ou espaço também,
  em toda plataforma: o mesmo arquivo tem de extrair igual em todas.
- Entrada marcada como link simbólico vira **arquivo comum**, com aviso: o
  PhxZip não cria link.
- Pasta do destino que é link recusa a entrada.
- Arquivo que já existe recusa, a menos que venha `--sobrescrever` — e então o
  que se apaga é o arquivo ou o link, **nunca o alvo** dele.
- Conteúdo que veio cifrado nasce com permissão `0600`.

### 3.3 A pasta precisa ser do dono

Nenhuma pasta do caminho de destino pode aceitar escrita de outro usuário, nem
ser de outro usuário: ele poderia trocar uma pasta por um link no meio da
extração e mandar o arquivo para fora. O `/tmp` (modo `1777`) é recusado de
propósito; extraia numa pasta sua (`mkdir saida && phxzipcmd extrair … --destino saida`).

### 3.4 A compactação não segue link

Link e arquivo especial (dispositivo, *socket*, cano) são pulados com aviso;
arquivo trocado por link durante a leitura aborta. O `.7z` nasce `0600`. Com
`--cifrar`, o cabeçalho também é cifrado (os nomes somem de quem não tem a
senha); `--nomes-claros` deixa os nomes à vista. `--ciclos N` escolhe o custo
da derivação da chave (2^N rodadas de SHA-256).

### 3.5 Os tetos, e o `--confiavel`

Todo arquivo é aberto como «de fora»: o motor recusa, **antes de alocar**, o
que passa dos tetos da §5 — a bomba de descompressão, o cabeçalho que declara
milhões de entradas, os ciclos de derivação que tomariam minutos. O
`phxzipcmd` lê o arquivo inteiro para a memória, e por isso também tem um teto
de tamanho de arquivo.

`--confiavel` abre com a folga do 7-Zip (a coluna «com `--confiavel`» da §5).
**Use só para arquivo que você mesmo gravou.**

### 3.6 Idioma

`PHXZIP_IDIOMA` (`pt`, `fr`, `en`, `it`, `de`, `es`, ou o nome: `Ingles`) e, sem
ela, o `LANG` do sistema. Idioma desconhecido fala português. Nomes de comando
e de opção (`extrair`, `--destino`) não se traduzem: são protocolo, e
traduzi-los quebraria o script de quem os digita.

## 4. PhxZipWeb — o navegador

```bash
phxzipweb                          # a porta padrão da §5, só download
phxzipweb --porta 7800             # outra porta
phxzipweb --pasta ~/extraidos      # libera «extrair na pasta»
phxzipweb --envio 67108864         # baixa o teto do envio (só baixa)
```

Ele imprime o endereço (`PhxZipWeb em http://127.0.0.1:<porta>/`); abra no
navegador. A tela compacta, lista, testa, espia e extrai — e o arquivo nunca
sai da máquina: o navegador manda para o `127.0.0.1` e recebe de volta.

O que ele recusa, e por quê:

| recusa | por quê |
|---|---|
| escutar fora de `127.0.0.1` — **não existe opção** de endereço | a porta aceita senha e extrai arquivos: ela nasce presa à máquina |
| `--pasta` que outro usuário pode escrever, ou que é de outro usuário | a mesma regra da §3.3; a porta **não sobe**, e o erro sai no terminal de quem escolheu a pasta, não num cartão do navegador horas depois |
| caminho de disco vindo do navegador | nenhuma rota recebe caminho: o que entra chega no corpo, o que sai é download — ou vai para a `--pasta`, que é da linha de comando |
| `Host` que não é `127.0.0.1:<porta>` nem `localhost:<porta>` | um site de fora não religa o próprio nome para `127.0.0.1` (religação de DNS) |
| `Origin` de outro site | um site aberto em outra aba não conversa com a porta |
| envio acima do teto | recusado pelo `Content-Length`, **antes de ler o corpo**, com `413` |
| `/../` ou arquivo fora da lista embutida | a tela é embutida no binário; não existe «servir a pasta» |

A senha vai no corpo do pedido e só nele — nunca na URL, nunca em cabeçalho —,
e o servidor não a registra, não a devolve e não guarda a chave derivada. Não
há estado entre pedidos. O contrato HTTP completo está em `docs/PHXZIP-WEB.md`.

## 5. Os tetos

Saem das constantes do fonte pelo `bancada/phxzip/manual.py`:

<!-- phxzip:tetos:inicio -->
| teto | padrão | com `--confiavel` | de onde sai |
|---|---:|---:|---|
| arquivo lido pelo `phxzipcmd` | 1 GiB | sem teto | `TETO_DO_ARQUIVO` |
| soma descompactada de um arquivo | 4 GiB | sem teto | `Limites::total` |
| uma entrada, descompactada | 256 MiB | 256 MiB | `Limites::entrada` |
| um bloco, descompactado | 256 MiB | 256 MiB | `Limites::bloco` |
| cabeçalho | 8 MiB | 128 MiB | `Limites::cabecalho` |
| entradas declaradas | 65.536 | 1.048.576 | `Limites::entradas` |
| ciclos do 7zAES (2^n rodadas de SHA-256) | 19 | 24 | `Limites::ciclos` |
| senha pela entrada padrão | 1.024 bytes | igual | `TETO_DA_SENHA` |

| teto do `phxzipweb` | valor | de onde sai |
|---|---:|---|
| porta padrão (só `127.0.0.1`) | 7700 | `PORTA_PADRAO` |
| envio (corpo de um pedido; `--envio` só baixa) | 256 MiB | `ENVIO_MAX` |
| cabeça JSON do envelope | 1 MiB | `CABECA_MAX` |
| espiar uma entrada | 256 KiB | `ESPIAR_MAX` |
| operações pesadas ao mesmo tempo | 2 | `SIMULTANEAS` |
| conexões abertas | 32 | `CONEXOES_MAX` |

A porta web abre todo arquivo com os limites **padrão**: o que chega pelo
navegador é sempre «de fora».
<!-- phxzip:tetos:fim -->

## 6. A bancada contra o 7-Zip

O PhxZip contra o `7z` do sistema (`/usr/bin/7z`), compactando e extraindo o
**mesmo** conjunto de arquivos, com **trabalho igual** (as quatro regras do
`bancada/LEIA-ME.md`):

- **mesmo método e mesmo nível:** o 7z do lado igual grava exatamente o que o
  PhxZip grava — LZMA2 não sólido, codificador guloso (`a=0`), cadeia de 48
  visitas (`mc=48`, a `PROFUNDIDADE` do PhxZip), `lc=3 lp=0 pb=2`, cabeçalho
  sem compressão (`-mhc=off`), uma thread (`-mmt=1`);
- **um processo por operação** dos dois lados: o tempo inclui subir o
  executável, para os dois;
- **a prova de que o trabalho foi o mesmo é o conteúdo:** cada extração é
  conferida arquivo por arquivo (SHA-256) contra o original, e nas duas
  direções cruzadas também — o 7z lê o que o PhxZip gravou e o PhxZip lê o que
  o 7z gravou;
- rodadas **intercaladas** (a ordem dos lados gira), e cada número é
  **mediana com a faixa** mín–máx. Vencedor só se diz quando as faixas não se
  cruzam.

O 7z no **padrão** dele (`-mx5`, sólido, codificador ótimo, várias threads)
aparece como **referência**, marcado como tal: é o que quem baixa o 7-Zip obtém
sem mexer em nada. **Não é trabalho igual** — comprime o conjunto inteiro como
um bloco só e usa mais de um núcleo —, e esconder essa linha seria publicar só a
comparação que favorece.

<!-- phxzip:bancada:inicio -->
Medido em **2026-10-09 13:02:54**, commit `c70e99ee`, 5 rodadas intercaladas, 4 núcleos, carga média no início 2,05 / 2,2 / 3,22 e no fim 3,28 / 2,89 / 3,28.
`phxzipcmd 0.20.0 (c70e99eec5b2) x86_64-unknown-linux-gnu` contra `7-Zip 23.01 (x64) : Copyright (c) 1999-2023 Igor Pavlov : 2023-06-20`.

Os lados:

- `phxzip`: phxzipcmd (LZMA2, nao solido, guloso, 1 thread) — trabalho igual
- `7z-igual`: 7z -t7z -m0=LZMA2:a=0:mf=hc4:mc=48:fb=273:lc=3:lp=0:pb=2:d=64m -ms=off -mmt=1 -mhc=off — trabalho igual
- `7z-padrao`: 7z no padrao (-mx5, solido, otimo, varias threads) — **referência — NÃO é trabalho igual**

**Conjunto `fontes`** — os fontes crates/ do commit c70e99ee (texto, comprime): 858 arquivos, 19.648.090 bytes.

| lado | compactar (mediana, faixa) | extrair (mediana, faixa) | tamanho do `.7z` | do original |
|---|---:|---:|---:|---:|
| `phxzip` | 1,28 s (1,16–1,45) | 0,88 s (0,86–0,96) | 6.100.681 | 31,0% |
| `7z-igual` | 1,09 s (1,05–1,27) | 0,73 s (0,49–0,82) | 5.974.106 | 30,4% |
| `7z-padrao` (referência) | 9,39 s (9,13–11,27) | 0,67 s (0,45–0,80) | 4.094.178 | 20,8% |

PhxZip × 7z com trabalho igual, no `fontes`:

- compactar: **empate dentro do ruído** — as faixas se cruzam (medianas 1,28 s × 1,09 s);
- extrair: o 7z é mais rápido (1,21× pela mediana; as faixas não se cruzam);
- tamanho: o do PhxZip é 126.575 bytes maior que o do 7z (2,12%).

**Conjunto `aleatorio`** — um arquivo de 8 MiB por SHA-256 encadeado (nao comprime): 1 arquivo, 8.388.608 bytes.

| lado | compactar (mediana, faixa) | extrair (mediana, faixa) | tamanho do `.7z` | do original |
|---|---:|---:|---:|---:|
| `phxzip` | 33,06 s (30,19–34,55) | 0,08 s (0,06–0,13) | 8.389.241 | 100,0% |
| `7z-igual` | 4,05 s (3,78–4,73) | 0,02 s (0,02–0,03) | 8.389.373 | 100,0% |
| `7z-padrao` (referência) | 0,85 s (0,59–1,06) | 0,02 s (0,02–0,08) | 8.389.326 | 100,0% |

PhxZip × 7z com trabalho igual, no `aleatorio`:

- compactar: o 7z é mais rápido (8,16× pela mediana; as faixas não se cruzam);
- extrair: o 7z é mais rápido (3,26× pela mediana; as faixas não se cruzam);
- tamanho: o do PhxZip é 132 bytes menor que o do 7z (0,00%).
<!-- phxzip:bancada:fim -->

Para refazer:

```bash
cargo build --release --offline -p phxzip-cmd
python3 bancada/phxzip/medir.py          # grava bancada/phxzip/resultados.json
python3 bancada/phxzip/manual.py         # leva os números para cá
```
