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

O `Cargo.lock` inteiro cabe em 30 linhas e não cita registro nem git. A prova
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

O que **não** dá: rodar. Sem Windows e sem `wine`, o `.exe` é conferido pela
forma (PE32+ x86-64), pelas DLLs que importa e pelos símbolos que exporta —
nunca por execução. Dizer mais que isso seria inventar.
