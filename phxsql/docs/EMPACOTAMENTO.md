# Os pacotes de download

Como `./empacotar.sh` monta os três zips, o que cada um leva, o que ele
deliberadamente **não** leva, e como quem baixou confere o que recebeu.

O pedido 17 é este: *«download dos fontes e do compilado Linux/Windows, com
manual»*. Um zip que ninguém consegue conferir não fecha esse pedido — é a
mesma regra que o backup de dados já seguia desde o pedido 43: **cópia mais
manifesto SHA-256, e alguém que leia tudo de volta e confira. Cópia que
ninguém consegue conferir é esperança, não cópia de segurança.**

---

## 1. Os três pacotes

```bash
./empacotar.sh            # os três, em pacotes/
./empacotar.sh linux      # só o de Linux
./empacotar.sh windows    # só o de Windows
./empacotar.sh fontes     # só o dos fontes
./empacotar.sh conferir   # desempacota o que já está em pacotes/ e confere
```

| zip | o que é | para quem |
|---|---|---|
| `phxsql-<versão>-linux.zip` | os três executáveis e o driver ODBC, compilados para `x86_64-unknown-linux-gnu` | quem quer rodar, e não compilar |
| `phxsql-<versão>-windows.zip` | os mesmos, `.exe` e `.dll`, para `x86_64-pc-windows-gnu` | idem, no Windows |
| `phxsql-<versão>-fontes.zip` | o repositório inteiro no `HEAD` | quem quer compilar, ler ou auditar |

Os dois pacotes de binário levam, além dos executáveis:

- `COMECE-AQUI.txt` — sobe o servidor em três passos, e diz como conferir o
  pacote antes de rodar qualquer coisa;
- `demonstracao/config.json` — escuta **só em `127.0.0.1`**, com a senha
  escrita no `COMECE-AQUI.txt`;
- `exemplos/Config_exemplo_01..03.json` — os modelos de verdade: isolado,
  source e réplica;
- `MANUAL.txt`, `README.md`, `CHANGELOG.md`;
- `docs/ODBC.md` e `docs/CONSOLE.md` — os dois documentos que o
  `COMECE-AQUI.txt` cita pelo nome. **Documento citado viaja junto**: quem
  baixou o zip não tem o repositório para ir buscar.

---

## 2. O manifesto, e por que ele tem o formato do `sha256sum`

Cada zip traz um `MANIFESTO.sha256` com o SHA-256 de **todos** os outros
arquivos do pacote, caminho relativo com barra normal, ordenado, uma linha por
arquivo:

```
2e7d2c03a9507ae265ecf5b5356885a53393a2029d241394997265a1a25aefc6  MANUAL.txt
...
```

O formato é o do `sha256sum` de propósito, para o pacote ter **dois
conferidores independentes**:

```bash
./phxsql conferir-pacote      # o binário que veio dentro do próprio pacote
sha256sum -c MANIFESTO.sha256 # para quem prefere não rodar o binário que está conferindo
```

O primeiro é o que funciona em toda parte: o Windows não tem `sha256sum`, e o
`phxsql.exe` está ali do lado. Ele usa o SHA-256 **deste projeto**, que já é
conferido contra os vetores do FIPS 180-4 — não há uma segunda implementação
de hash aqui, como não há uma segunda implementação de senha.

E `pacotes/SHA256SUMS` traz o hash dos **próprios zips**, para quem baixou
saber que o download chegou inteiro *antes* de abrir — o manifesto de dentro
não responde essa pergunta, porque ele só existe depois do `unzip`.

### O conferidor pega três coisas, não uma

`phxsql conferir-pacote` (`crates/phxsql-cli/src/main.rs`) responde `INTEGRO`
ou lista:

- `DIFERE` — o arquivo está no manifesto e o conteúdo mudou;
- `FALTA` — está no manifesto e não está no pacote;
- `A MAIS` — está no pacote e não está no manifesto.

O terceiro é o que quase sempre falta num conferidor. **Conferência de hash só
olha o que o manifesto lista**: quem acrescenta um arquivo ao pacote não mexe
em nenhuma linha do manifesto e passaria batido — que é exatamente o jeito de
entregar um binário a mais junto do pacote legítimo. É a mesma regra que o
`backup.json` já seguia.

E isso foi medido, não suposto. Num pacote de Linux com um `atualizador.sh`
acrescentado e mais nada mexido:

```
$ sha256sum -c MANIFESTO.sha256
...
phxsqld: OK                       ← passa

$ ./phxsql conferir-pacote
1 DIVERGENCIA(S):
  A MAIS  atualizador.sh -- nao esta no manifesto     ← reprova
```

Os dois conferidores não são intercambiáveis: o `sha256sum -c` é o segundo
par de olhos sobre os arquivos **listados**, e o `conferir-pacote` é o único
que responde pelo pacote **inteiro**.

Sete testes travam isso, em `crates/phxsql-cli/src/main.rs`, e cada um é a
prova de que o conferidor **reprova** o defeito, não só de que aprova o que
está certo:

| teste | o que repõe |
|---|---|
| `pacote_intacto_confere` | nada — a linha de base |
| `um_byte_trocado_reprova` | um bit invertido no `phxsqld` |
| `mesmo_tamanho_conteudo_outro_reprova` | mesmo tamanho, conteúdo diferente |
| `arquivo_a_mais_reprova` | um arquivo que o manifesto não lista |
| `arquivo_que_falta_reprova` | um arquivo removido |
| `pacote_sem_manifesto_avisa_em_vez_de_aprovar` | pacote sem manifesto |
| `manifesto_com_linha_torta_avisa` | manifesto ilegível |

Conferidor que aprova tudo é pior que conferidor nenhum, porque quem baixou
acha que conferiu.

---

## 3. As quatro travas antes de qualquer zip sair

**Um número de versão, quatro lugares.** `confere_versoes()` para o
empacotamento se `Cargo.toml`, `Cargo.lock`, o cabeçalho do `MANUAL.txt` e o
título lançado mais novo do `CHANGELOG.md` não disserem a mesma versão. Um zip
chamado `0.18.0` com um manual de `0.17.0` é pior que zip nenhum: quem baixou
vai acreditar no manual. É a lição do selo da capa do dossiê, que passou quatro
lançamentos dizendo 0.11.0.

**Alvo e ligador antes de compilar.** Para o pacote de Windows,
`confere_ferramentas_windows()` confere o alvo (`rustup target list
--installed`) e o `x86_64-w64-mingw32-gcc`, e imprime o comando exato de quem
não os tem. Falhar no meio de um `cargo build` com `linker not found` não diz a
ninguém o que instalar.

**Árvore limpa para o pacote de fontes.** O `git archive` lê o `HEAD`, e não a
árvore de trabalho: empacotar com mudança por commitar produz um zip diferente
do que o autor está vendo, e ninguém percebe — nem ele, nem quem baixou. O
empacotador recusa e diz o `HEAD` que usaria; `PHXSQL_EMPACOTAR_SUJO=1` aceita
o `HEAD` de propósito.

**O `Cargo.toml` na raiz do zip de fontes.** Rodado de dentro de `phxsql/`, o
`git archive` recorta o subdiretório e o zip nasce com o `Cargo.toml` na raiz —
comportamento sutil, que o empacotador confere depois de extrair. Se o git
mudar de ideia, o pacote deixa de compilar, e o empacotador tem de ser o
primeiro a saber.

E uma quinta, que não é trava e sim recado: rodado de dentro de um zip de
fontes já extraído não há `.git`, e o pacote de fontes não sai dali — ele nasce
do histórico. O empacotador diz isso em uma linha, em vez de repassar o
`fatal: not a git repository` do git embaralhado com a mensagem de árvore suja.
Os pacotes de binário saem normalmente de um diretório extraído.

---

## 4. O que o empacotador **não** inclui

- **`target/`** e os 2,4 GB de `bancada/phxsql/` — o `git archive` respeita o
  `.gitignore` de graça, e é por isso que o pacote de fontes se faz com ele e
  não com um `cp -r`.
- **Os fontes, no pacote de binário; os binários, no de fontes.** São três
  downloads porque são três perguntas diferentes.
- **`config.json` de produção, token ou senha de ninguém.** O único config que
  viaja é o de demonstração, que escuta em `127.0.0.1` e cuja senha está
  escrita em letras grandes no `COMECE-AQUI.txt`.
- **DLLs do mingw.** Medido: o `phxsqld.exe` importa só
  `KERNEL32`, `msvcrt`, `ntdll`, `WS2_32`, `bcryptprimitives` e
  `api-ms-win-core-synch-l1-2-0` — tudo do Windows. Não há
  `libgcc_s_seh-1.dll` nem `libwinpthread-1.dll` para acompanhar.
- **Arquivo de licença.** `Cargo.toml` e `README.md` dizem `MIT OR Apache-2.0`
  e **não existe `LICENSE` no repositório**. Escolher e colar o texto de uma
  licença é decisão do dono, não do empacotador; enquanto não houver arquivo,
  ele não inventa um.

E uma coisa que ele **não promete**: os zips não são reproduzíveis byte a
byte entre duas rodadas. O `demonstracao/config.json` traz um hash PBKDF2 com
**sal novo a cada execução**, porque o hash sai do próprio `phxsqld --senha` e
não de uma constante colada no script — não existe uma segunda implementação
de senha neste projeto. Trocar isso por um hash fixo tornaria os zips
reproduzíveis e poria uma senha pré-computada em circulação; o preço não vale.

---

## 5. Zero dependências externas, medido

O pedido 9 diz *«tudo em Rust, sem dependência»*. Continua verdade, e o número
sai de comando, não do teclado:

```bash
cargo metadata --offline --format-version 1   # 7 pacotes no grafo, os 7 deste
                                              # repositório, 0 com "source"
```

O `Cargo.lock` inteiro cabe em 53 linhas e não cita registro nem git. A prova
que vale é a do diretório limpo, que é o que o dono vai fazer:

```bash
unzip phxsql-<versão>-fontes.zip -d /tmp/limpo
cd /tmp/limpo/phxsql-<versão>-fontes
CARGO_HOME=/tmp/limpo/cargo-home cargo build --offline --release
```

Com o `CARGO_HOME` vazio (zero entradas), `CARGO_NET_OFFLINE=true` e as
variáveis de proxy apagadas: **28,6 s, 30,3 s e 34,3 s** em três medições,
sete crates, quatro binários — `phxsqld`, `phxsql`, `phxsqlcmd` e
`libphxsql_odbc.so`. Nada foi baixado porque não há nada para baixar.

E o laço fecha: de dentro desse diretório extraído, `./empacotar.sh linux`
remonta o pacote de Linux inteiro. O de fontes não sai dali, e o empacotador
diz por quê — ele nasce do histórico do git, que o zip não carrega.

---

## 6. Compilar para Windows a partir do Linux

```bash
rustup target add x86_64-pc-windows-gnu
sudo apt install mingw-w64          # Fedora: mingw64-gcc · Arch: mingw-w64-gcc
cargo build --offline --release --target x86_64-pc-windows-gnu
```

O que dá para conferir sem um Windows por perto:

```bash
file target/x86_64-pc-windows-gnu/release/phxsqld.exe
# PE32+ executable (console) x86-64 ..., for MS Windows

x86_64-w64-mingw32-objdump -p .../phxsql_odbc.dll | grep -c 'SQL'
# os 21 símbolos ODBC que o gerenciador procura
```

Esta seção dizia, até esta rodada: *«O que **não** dá: rodar. Sem Windows e
sem `wine`, o `.exe` é conferido pela forma, pelas DLLs que importa e pelos
símbolos que exporta — nunca por execução.»* A frase estava certa **enquanto
não havia `wine`**, e o «sem `wine`» dela era a saída.

### 6.1 Com o `wine`, «compila e empacota» virou «gravou e leu»

```bash
sudo apt install wine        # ~150 MB de download, ~700 MB em disco
bancada/windows/provar.sh
```

Não precisa de VM, e não pelo mesmo motivo do ARM. Uma VM completa exige
`/dev/kvm`, que esta máquina não tem — ela própria é uma máquina virtual sem
aninhamento. O `qemu-user-static` contorna isso emulando o **binário**; o
`wine` contorna de outro jeito: ele **não emula nada**. O `.exe` é x86-64 e a
máquina é x86-64, então o código roda **nativo** — o que o `wine` reimplementa
são as **DLLs** do Windows sobre a libc do Linux.

O que a prova faz, com a **mesma sonda** da bancada ARM (`bancada/arm/sonda.py`,
que agora recebe o rótulo de fora em vez de se anunciar «ARM64» em toda
corrida):

| Passo | Resultado |
|---|---|
| gerar o hash da senha **com o `.exe`** | `pbkdf2-sha256$…` — o PBKDF2 roda sob `wine` |
| subir o `phxsqld.exe` | no ar, RSS **8.796 e 8.800 kB** em duas corridas (inclui o próprio `wine`) |
| `ping` com token | `ok` |
| `login` com a senha cujo hash o próprio `.exe` gerou | `ok` — a criptografia fecha dos dois lados |
| `criar_database` + `criar_tabela` | `ok` — os sete arquivos nascem pelo `CreateFile` do `wine` |
| inserir 50 linhas | **50 de 50** |
| `varrer` de volta | **50 registros, 50 devolvidas** |

E o binário provado pode ser **o do pacote**: sem `target/`, o script pega o
`.exe` de `pacotes/phxsql-*-windows.zip`. É até melhor — o que se prova aí é o
arquivo que o usuário baixa, e não um subproduto da compilação.

### 6.2 O que isto **não** prova, e continua sem prova

- **Desempenho no Windows.** As 50 linhas saíram a 4,5 ms/linha numa corrida e
  60,3 ms noutra, com a máquina carregada no meio. Isso é ruído, não o custo do
  `wine` nem o do motor. Para desempenho não há substituto: é preciso um
  Windows.
- **Compatibilidade completa.** O `wine` reimplementa as DLLs; ele não *é* o
  Windows. O caminho que o script exercita é estreito de propósito — arquivo,
  soquete, relógio e criptografia —, e é o caminho do servidor.
- **O driver ODBC.** A `phxsql_odbc.dll` continua conferida só pela forma e
  pelos 21 símbolos: provar um driver ODBC exige o **gerenciador** do Windows
  carregando-o, e o `wine` traz o dele, mas isso é outra corrida.

Detalhes e as três decisões do script em `bancada/windows/LEIA-ME.md`.

---

## 7. ARM: as placas pequenas, e onde a resposta para de ser medida

A pergunta que abriu esta seção foi «será que roda num IoT, num Android e num
iOS?». As três respostas são diferentes, e só a primeira é sim.

### 7.1 O que foi medido

Dois alvos novos entraram no `empacotar.sh` — `./empacotar.sh arm64` e
`./empacotar.sh arm32`:

| Alvo | Placa típica | Binário | Compilou em |
|---|---|---:|---:|
| `aarch64-unknown-linux-musl` | Raspberry Pi 3/4/5, gateway industrial, ARM de nuvem | **6,8 MB** | 25,9 s |
| `armv7-unknown-linux-musleabihf` | Raspberry Pi 2, Zero W, roteador com flash | **6,7 MB** | 23,0 s |

Os dois saíram **de primeira**, sem um `gcc` cruzado instalado: o ligador é o
`rust-lld` que já vem com a ferramenta. Isso é consequência direta da regra da
casa — **zero dependências externas**. Com uma crate de C no meio, cada uma
delas teria de compilar cruzado também, e é aí que a compilação cruzada
costuma morrer.

Os dois são **estáticos**: um arquivo só, sem carregador dinâmico, sem depender
da libc que a distribuição da placa trouxe. É o mesmo motivo que fez a imagem
Docker `FROM scratch` funcionar.

### 7.2 O consumo, que é o que decide numa placa

Medido no servidor parado, com a interface web desligada:

| | |
|---|---:|
| RSS parado | **4,9 MiB** |
| threads | **3** |
| RSS depois de aceitar conexão | 5,1 MiB |
| cache de páginas do `.ndx`, quando cheio | 2.048 × 4 KiB = **8 MiB** |

Ou seja: **~5 MiB de piso e ~13 MiB de teto** na configuração padrão. O teto é
`recursos.cache_paginas`, e numa placa apertada ele desce — é o mesmo campo que
comprou 2,40× de leitura no `DESEMPENHO.md`, então baixá-lo é uma troca
consciente entre memória e velocidade, não um ajuste cego.

### 7.3 A fronteira, que durou uma hora

A primeira versão desta seção dizia, com todas as letras, que **os binários ARM
não tinham sido executados** — esta máquina é x86 e não havia emulador. Estava
certo enquanto durou, e a distinção era o ponto: compilar não é rodar.

Durou até alguém perguntar se dava para testar. **Não era preciso VM.** Uma VM
completa exige `/dev/kvm`, que este ambiente não tem (é ele próprio uma máquina
virtual sem aninhamento). Mas o `qemu-user-static` emula o **binário**, não a
máquina, e por isso não depende de KVM nenhum:

```bash
sudo apt install qemu-user-static
bancada/arm/provar.sh
```

O que a bancada faz, e o resultado:

| Passo | Resultado |
|---|---|
| gerar o hash da senha **com o binário ARM** | `pbkdf2-sha256$210000$…` — o PBKDF2 roda em ARM |
| subir o `phxsqld` aarch64 sob emulação | no ar, **11,9 MiB** de RSS |
| `ping` com token | `ok` |
| `login` com a senha cujo hash o próprio ARM gerou | `ok` — a criptografia fecha dos dois lados |
| `criar_database` + `criar_tabela` | `ok` |
| inserir 50 linhas | **50 de 50**, 1,1 ms por linha *sob emulação* |
| `varrer` de volta | **50 registros, 50 devolvidas** |

A primeira linha lida de volta:

```json
{"rowid": 1, "sensor": "s0", "valor": "20.00", "softdeleted": false, "rownum": 1}
```

Então **«compila» virou «gravou e leu»**. O que continua sem prova é o
desempenho real: 1,1 ms por linha é o custo da *emulação*, não o de uma placa
— o `qemu-user` traduz instrução por instrução. Numa placa de verdade o número
é outro, e provavelmente melhor; **medir isso continua exigindo a placa.**

O RSS de 11,9 MiB também é sob emulação e inclui o custo do próprio `qemu`;
o número nativo medido em x86 é o da §7.2 — 4,9 MiB.

### 7.4 O driver ODBC não vai no pacote ARM, e não faz falta

`musl` não produz `cdylib`, então `libphxsql_odbc.so` não sai para esses
alvos. Isso não é perda: o driver é do lado **cliente** — ele mora na máquina
que roda a ferramenta de relatório, não na placa que guarda o dado. Quem
precisar mesmo de ODBC em ARM compila para o alvo `gnu` da mesma arquitetura.

### 7.5 Android: compila, e para no ligador

O alvo `aarch64-linux-android` **compilou** — todos os crates passaram. Falhou
só no **link**, por não achar a libc do Android (bionic: `-lc`, `-lm`, `-ldl`),
que vem no NDK e não está nesta máquina. É limite do ambiente, não do código.

Mas «linkar» não é «ter um app», e são duas coisas diferentes:

- **Termux** — um servidor de linha de comando num Android com Termux é o caso
  fácil, e é essencialmente o caso Linux ARM da §7.1.
- **Dentro de um aplicativo** — aí o motor precisaria virar biblioteca nativa
  (`cdylib`) com uma camada JNI, **que não existe hoje**. E o Android moderno
  mata processo em segundo plano com liberdade, então um daemon que escuta
  porta não é uma forma que o sistema apoie.

### 7.6 iOS: não é questão de compilar

Aqui a resposta muda de natureza. O alvo `aarch64-apple-ios` exige o SDK da
Apple e o Xcode, que só existem em macOS — não dá nem para tentar aqui. E
mesmo com um Mac, **o formato do iOS não comporta o que o `phxsqld` é**: o
sistema não permite processo em segundo plano de longa duração nem um app
escutando porta para outros apps usarem.

A forma que faria sentido é outra: o motor como **biblioteca estática** ligada
dentro do aplicativo, com uma camada FFI em C/Swift. Isso é plausível
justamente por não haver dependência externa — mas **essa camada não existe**,
e a arquitetura cliente-servidor de hoje teria de virar embutida. É trabalho de
projeto, não de compilação.

### 7.7 Resumindo em uma tabela

| Plataforma | Estado | O que falta |
|---|---|---|
| Linux x86-64 | **roda, exercitado** | — |
| Windows x86-64 | **roda: gravou e leu 50 linhas sob `wine`** (§6.1) | um Windows de verdade, para desempenho e para o driver ODBC |
| Linux ARM64 / ARMv7 | **roda: gravou e leu 50 linhas sob emulação** | o desempenho real, que só a placa mede |
| Android (Termux) | **compila; link precisa do NDK** | o NDK, e uma corrida real |
| Android (dentro de app) | não | `staticlib` + camada FFI em C + camada JNI, e **largar o daemon** |
| iOS | não | Mac com Xcode, camada FFI, e virar biblioteca embutida |

As duas últimas linhas deixaram de ser só «o que falta compilar» e ganharam
documento próprio: **`docs/MOBILE.md`** mede o motor contra o SQLite(R), diz
onde cada um ganha, e desenha a forma que cabe num aparelho — biblioteca
embutida mais cliente de sincronia, e **não** um mini-servidor escutando porta,
porque o iOS proíbe e o Android mata.

Duas correções que aquele documento trouxe para cá, e que valem no ato de
empacotar:

- **`cdylib` não é o caminho no aparelho, `staticlib` é.** A §7.4 já registrava
  que `musl` não produz `cdylib`; para dentro de um aplicativo o que se liga é
  uma biblioteca **estática**, e aí a restrição do `musl` deixa de importar.
- **O binário não é o custo maior.** Os 6,8 MB da §7.1 são o que se soma ao
  aplicativo; o **dado** é o que cresce, e ele ocupa **4,3× o do SQLite(R)**
  nas mesmas 200.000 linhas (`docs/MOBILE.md` §2). Num telefone, é a segunda
  conta que decide.
