# As tecnologias, e por que cada uma

Inventário do que foi usado — **para fazer o produto** e **para fazer o
trabalho**, que são coisas diferentes e as duas contam.

Os números vêm de contagem no repositório, não de lembrança.

---

## 1. O produto

### Linguagens, medidas

| | Linhas | Onde |
|---|---:|---|
| Rust | **106.413** | `crates/` — o motor inteiro |
| Markdown | 23.558 | `docs/` |
| HTML | 12.870 | a interface, num arquivo só |
| Python | 12.813 | bancadas, sondas e geradores |
| JavaScript | 6.911 | grade, telemetria, multitela, diagrama ER |
| CSS | 771 | |

### A decisão que define tudo: zero dependências externas

**Só a `std` do Rust.** Nenhuma crate de terceiro — conferido: as únicas linhas
sob `[dependencies]` apontam para outros crates do próprio workspace.

Isso não é purismo. É o que comprou, medido nesta sessão:

- **Compilação cruzada de primeira** para ARM64 e ARMv7, sem nenhum `gcc`
  cruzado instalado — o ligador é o `rust-lld` que já vem junto. Com uma crate
  de C no meio, cada uma teria de compilar cruzado também, e é aí que a
  compilação cruzada costuma morrer.
- **`cargo build --offline`** funciona.
- **Imagem Docker `FROM scratch`** de 6,42 MB, possível só porque não há
  carregador dinâmico nem libc a arrastar.
- **O motor sozinho pesa 790–930 KiB**, o que torna a pergunta do ESP32
  respondível em vez de absurda.

O preço: **26 módulos escritos e mantidos aqui.**

### O que foi escrito à mão no `phxsql-core`

`base64` · `crc` (CRC-32) · `cifra` (ChaCha20-Poly1305, 982 linhas) ·
`datahora` · `desafio` (desafio-resposta) · `ed25519` · `fio` (aperto de mão
Noise, 935 linhas) · `frogcript` · `hash` (SHA-256) · `hkdf` · `json` ·
`keyenc` · `paginacao` · `paralelo` · `schema` · `senha` (PBKDF2, 286 linhas) ·
`sha1` · `sha512` · `types` · `uuid` · `value` · `x25519` · `zip`

### Criptografia se confere contra vetor oficial

Nada foi aceito por «parecer certo». As normas citadas nos testes:

**FIPS 180-4** (SHA) · **RFC 8439** (ChaCha20-Poly1305) · **RFC 7748**
(X25519) · **RFC 5869** (HKDF) · **RFC 4231** e **RFC 2104** (HMAC) ·
**RFC 2898** (PBKDF2) · **RFC 8032** (Ed25519) · **RFC 9562** (UUID) ·
**RFC 4648** (Base64) · **RFC 1951** (deflate) · **RFC 5802** e **RFC 7677**
(SCRAM) · **RFC 4180** (CSV) · **RFC 5322**, **2045** e **2047** (correio)

Dezessete normas. A escolha do ChaCha20 sobre AES tem motivo escrito: AES em
software puro se escreve com tabelas, e **tabela em cache vaza a chave pelo
tempo de acesso**; o ChaCha20 é soma, XOR e rotação — tempo constante por
construção, sem tabela nenhuma. É a mesma escolha que o TLS 1.3 e o WireGuard
fazem para máquina sem AES-NI.

### Formato em disco

Sete arquivos por tabela (`.reg` `.ndx` `.bin` `.memo` `.log` `.trash`
`.reason`) mais três condicionais (`.lgpd` `.bkp` `.pag`), no modelo de
arquivos separados do HFSQL®. Endereçamento **O(1) pelo rowid**, slots de
tamanho fixo com **CRC-32** — o que torna escrita rasgada *detectável* em vez
de silenciosa. Em `docs/FORMATO.md`.

### Protocolo e interface

- **JSON sobre TCP**, com o analisador escrito aqui.
- **Interface web sem framework**: JavaScript direto, tudo embutido no binário
  por `include_str!` — por isso o servidor é **um arquivo só**, sem pasta de
  assets para se perder.
- **Driver ODBC** próprio (`cdylib`), testado pela ABI sem passar pelo
  `unixODBC`.

---

## 2. O trabalho

Esta metade costuma não ser escrita, e é a que mais se reaproveita.

### Orquestração por agentes em worktrees isolados

Cada frente roda num **git worktree próprio**, com **faixa de portas
reservada**. Sem faixa, dois agentes se derrubam com «porta em uso»; sem
worktree, dois editam o mesmo arquivo ao mesmo tempo.

A integração é **sequencial**, uma frente por vez, com portões completos entre
cada uma — e é lá que aparecem os defeitos que nenhuma frente sozinha podia
ver.

### Prova de outra arquitetura sem VM

`qemu-user-static`. A máquina não tinha `/dev/kvm` nem flag de virtualização —
VM completa estava fora. Mas `qemu-user` emula o **binário**, não a máquina.
Foi assim que o servidor ARM64 subiu, autenticou com um hash PBKDF2 gerado pelo
próprio binário emulado, e gravou e leu 50 linhas.

### Bancada de replicação em contêiner

Docker com rede isolada — e foi **só** o isolamento de rede que revelou uma
opção de autorização que ninguém lia: um vizinho com configuração vazada levava
200 de 200 eventos.

### Compilação cruzada

`x86_64-pc-windows-gnu` (com `mingw-w64`), `aarch64-unknown-linux-musl` e
`armv7-unknown-linux-musleabihf` (com `rust-lld`, sem `gcc` cruzado), e
`x86_64-unknown-linux-musl` para a imagem `scratch`.

### Medição

Exemplos `cargo` dedicados (`onde-doi`, `custo-da-trava`, `custo-do-excluir`,
`custo-do-alter`) que instrumentam **por dentro** em vez de cronometrar por
fora. Mediana e dispersão de várias corridas, com a carga da máquina anotada —
porque a máquina raramente está quieta.

### Bancada de três motores no mesmo trabalho

`bancada/comparacao/` mede PhxSql, MySQL(R) 8.0.46 e SQLite(R) 3.45.1
**intercalados na mesma rodada** — somar bancadas de dias diferentes daria três
colunas e nenhuma comparação. O SQLite(R) vem da biblioteca padrão do Python
(o módulo `sqlite3` é extensão em C, não Python interpretado), o MySQL(R) é o
cliente `mysql` recebendo o comando por arquivo, e o PhxSql é o
`--example carga` cronometrado **por dentro**.

Duas peças que valem reaproveitar:

- **A fase `conferir`**, que não mede tempo — mede se os motores chegaram ao
  mesmo estado. Contagem, soma de `valor` e soma de `cadastro`, em três marcos,
  conferidas contra a **forma fechada** calculada à parte. Divergiu, a bancada
  recusa publicar.
- **O piso do formato**: 20.000 instruções que não fazem nada (`DO 1;`) pelo
  mesmo caminho, para separar o motor do transporte quando os lados não têm a
  mesma forma.

### Prova real, mecanizada

Um **catálogo de guardas** onde cada entrada guarda o trecho original e a troca
que **repõe o defeito**. Roda-se a mutação e conta-se quantas *não pegaram*.
Hoje são 37.

### Catraca de dívida

Um conferidor que conta o que falta (textos por traduzir) e **reprova quando o
número sobe** — e também quando alguém melhora e esquece de baixá-lo.

### Documentação gerada

**Seis** geradores escrevem todo número visível do relatório. Nenhum se digita.
O sexto insere o gráfico dos três motores e **recusa** se a figura for mais
velha que a medição — a lição do binário velho aplicada a uma figura: um
gráfico desenhado da corrida anterior publica o passado com data de hoje, e
nada no desenho denuncia isso.

E a prosa que acompanha um número medido também é gerada: um modo
`--so-prosa` refaz as ressalvas a partir dos números já guardados, sem
remedir. Sem ele, corrigir uma palavra custaria quinze minutos de bancada — e
o que se faria em vez disso é editar o JSON à mão, que é como número gerado
vira número digitado.

### Zelador de ambiente

Script em horário que libera disco **sem nunca tocar no que está em uso** —
confere por `cwd` de processo vivo, e não mata processo nenhum.

---

## 3. O que foi avaliado e recusado, com o número

Recusa medida é resultado, e é o que impede a mesma proposta de voltar.

| Proposta | Veredito |
|---|---|
| **CUDA / GPU** | O `SUM` já roda a 28.234 MiB/s — **1,79× o pico teórico do PCIe 3.0 x16** —, então não existe limiar onde a GPU compense. E 99,4% de uma inserção não é aritmética. A alternativa sem dependência deu **3,90×** em 4 núcleos |
| **WAL + group commit + LSM** | Receita para o gargalo do InnoDB. Medido o nosso: das dez propostas, **cinco já existiam**, duas miravam problema que não temos, uma quebraria a ordem de digitação, **duas eram reais** |
| **Duas larguras de slot** | O formato não permite (o tamanho é um campo só), e cobraria **2,36× em toda leitura** para poupar uma passada uma vez |
| **Lote de replicação 500 → 2.000** | Quadruplicaria o pior caso de memória de quem serve |
| **MVCC** | Quebraria o rowid-como-endereço e a replicação por rowid. **Decisão pendente do dono** |
| **Somar as duas bancadas existentes para comparar três motores** | Daria três colunas e nenhuma comparação: medidas de dias diferentes carregam o ambiente junto. Custou uma bancada nova, e ela achou a violação da regra 1 que as duas anteriores escondiam |
| **Publicar a variante `2ind` do SQLite(R)**, que é a que estruturalmente se parece com o nosso | Ela é mais lenta em todas as quatro fases (1,04× a 1,31×), então publicá-la melhoraria três dos nossos quatro números **sem o motor ter feito nada**. Publica-se a `rowid`, que casa com o InnoDB e nos desfavorece |
| **TLS 1.3 à mão** | Milhares de linhas e risco real: TLS mal escrito é pior que TLS ausente, porque **parece** seguro |
