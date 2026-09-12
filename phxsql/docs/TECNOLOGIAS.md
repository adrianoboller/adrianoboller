# Tecnologias do PhxSql

Isto não é o `README` (que diz como usar) nem o `MANUAL` (que diz o que o
produto faz). É o inventário do que se usou **para fazer o produto e para
fazer o trabalho** — as duas metades, porque a segunda é a que se reaproveita
e é a que ninguém escreve.

A cláusula pétrea que exige este documento manda também a regra que o rege:
**todo número visível sai de um gerador, ou está errado e ninguém percebeu
ainda.** Nenhuma tabela abaixo foi digitada — todas saem de:

```bash
python3 docs/tecnologias/extrair.py
```

que é código novo desta rodada, em `docs/tecnologias/extrair.py`, só
biblioteca padrão do Python 3. Rodar de novo reproduz as mesmas tabelas
contra o estado atual da árvore — se o código mudou, os números mudam com
ele, o que é o comportamento certo, não um defeito do documento. Onde o
extrator precisou de uma lista (por exemplo, quais arquivos a interface
embute), **a lista sai do código-fonte** (`crates/phxsql-server/src/http.rs`),
nunca copiada para dentro do script — é a mesma lição que já custou um rodapé
de dossiê publicado com 780 KiB quando a interface tinha 1.032.

Onde algo não pôde ser medido nesta rodada, o texto diz isso; nenhum número
foi estimado no lugar de uma medição que faltou.

---

## 1. Linguagens e volume, contados

### 1.1 Rust — por crate, código separado de teste

<!-- GERADO: bloco_linguagens_rust() -->
| crate | arquivos .rs | codigo | teste | comentario | vazias | total |
|---|---:|---:|---:|---:|---:|---:|
| `phxsql-cli` | 1 | 815 | 104 | 113 | 78 | 1110 |
| `phxsql-cmd` | 2 | 579 | 110 | 171 | 62 | 922 |
| `phxsql-core` | 33 | 11449 | 3825 | 2870 | 1463 | 19607 |
| `phxsql-ffi` | 7 | 1408 | 1014 | 703 | 235 | 3360 |
| `phxsql-odbc` | 7 | 2412 | 1086 | 920 | 226 | 4644 |
| `phxsql-server` | 55 | 41686 | 24798 | 19164 | 4585 | 90233 |
| `phxsql-sql` | 10 | 6529 | 3279 | 1906 | 732 | 12446 |
| `phxsql-store` | 23 | 12588 | 3159 | 5134 | 1433 | 22314 |
| **total** | **138** | **77466** | **37375** | **30981** | **8814** | **154636** |

Proporcao teste/codigo (so `src/`, sem comentario nem linha vazia): **37375/77466 = 0.48×**.

Alem do `src/`: **71** programas de medicao em `examples/` (16052 linhas — bancada em Rust, nao produto nem teste) e **55** arquivos em `tests/` de integracao fora de `src/` (18459 linhas).
<!-- /GERADO -->

A proporção teste/código sai medida no bloco acima, não digitada aqui. O
retrato que a tabela desenha é estável: `phxsql-server` sozinho concentra
mais da metade do código e a maior parte do teste — é onde mora o protocolo,
o SQL embutido nas operações, a interface HTTP e a réplica —, enquanto
`phxsql-cli` e `phxsql-cmd`, as duas ferramentas de linha de comando mais
finas, têm a proporção mais baixa, porque a lógica que importa já está
testada nas camadas de baixo que elas chamam.

Contagem feita **por um tokenizador de Rust escrito para este extrator**
(comentário de linha e de bloco, string, string bruta `r#"…"#` e caractere
apagados antes de contar chaves), não por `grep` de `{`/`}` — este projeto
tem um parser de JSON (`crates/phxsql-core/src/json.rs`) com literais de
caractere como `'{'` e `'}'` no próprio código, que uma contagem ingênua de
chaves confundiria com abertura/fechamento de bloco de teste. O classificador
foi conferido contra `wc -l` linha a linha em arquivos de amostra antes de
rodar no workspace inteiro.

### 1.2 A interface web embutida no binário

<!-- GERADO: bloco_interface() -->
Lista extraida de `crates/phxsql-server/src/http.rs` (todo `include_str!`/`include_bytes!` que aponta para `ui/`), nao digitada -- e a mesma lista que decide o que o binario embute:

| arquivo embutido | linhas | KiB |
|---|---:|---:|
| `ui/index.html` | 15338 | 860.0 |
| `ui/grid/phx-grid.css` | 168 | 12.3 |
| `ui/grid/phx-grid.js` | 1860 | 90.1 |
| `ui/diagrama-er.js` | 712 | 29.1 |
| `ui/telemetria.css` | 447 | 19.8 |
| `ui/telemetria.js` | 1799 | 88.0 |
| `ui/multitela.css` | 156 | 8.6 |
| `ui/multitela.js` | 1588 | 69.0 |
| `ui/claude.js` | 1357 | 67.2 |
| `ui/grid/CHANGELOG-phx-grid.md` | 224 | 29.7 |
| **total (10 arquivos)** | **23649** | **1273.9** |

Em `ui/` mas **fora** do `include_str!`/`include_bytes!` (4 arquivos, não embutidos no binário):
- `crates/phxsql-server/ui/explorador.css`
- `crates/phxsql-server/ui/explorador.html`
- `crates/phxsql-server/ui/explorador.js`
- `crates/phxsql-server/ui/grid/LEIAME-phx-grid.md`
<!-- /GERADO -->

Esta é exatamente a lista que o `CLAUDE.md` do projeto manda tratar assim: a
receita do KiB de interface do rodapé do dossiê "era uma lista de três
arquivos copiada no script"; hoje ela sai do próprio `http.rs`, e o total que
a tabela acima soma — maior que os 1.032 KiB registrados na última correção do
dossiê (feita com 9 arquivos na lista; o `CHANGELOG` da grade é o décimo,
presente hoje em `http.rs`) — sai de ler `http.rs` agora, não de repetir um
número de uma rodada anterior. Não importa exatamente quando cada arquivo
entrou: se este documento tivesse copiado o 1.032 antigo, estaria errado pelo
mesmo motivo que o rodapé já errou uma vez.

### 1.3 Outras linguagens

<!-- GERADO: bloco_outras_linguagens() -->
| o que | onde | arquivos | linhas |
|---|---|---:|---:|
| JavaScript (prova ponta a ponta) | `testes-web/` | 47 | 10136 |
| Python (bancada de medicao) | `bancada/` | 101 | 41682 |
| Shell (empacotar, zelador, provas) | todo o repositorio | 17 | 2466 |
| Markdown (documentacao tecnica) | `docs/` (nao recursivo em `dossie/`, `design/`, `video/`) | 304 | 74968 |
| Python (geradores de dossie/pedidos) | `docs/dossie/` | 17 | 5263 |
<!-- /GERADO -->

Não incluído acima porque já está na tabela 1.1: os `.rs` de `examples/` e
`tests/` (bancada e prova em Rust). Não medido: HTML/CSS/JS fora de
`crates/phxsql-server/ui/` (não há outro em produção) e o volume do próprio
`marca/` (imagens, não código).

---

## 2. Dependências, e o que a escolha comprou

A regra do projeto é **zero dependências externas** — só a `std`. Provado
pelo arquivo, não pela lembrança:

<!-- GERADO: bloco_dependencias() -->
`Cargo.lock` lista **8** pacotes. Todos: `phxsql-cli, phxsql-cmd, phxsql-core, phxsql-ffi, phxsql-odbc, phxsql-server, phxsql-sql, phxsql-store`.

Nenhuma linha `source = ` no arquivo (contadas: 0) -- todo pacote e `path`, isto e, um crate deste proprio workspace. Pacotes externos ao workspace: **0**.

Confirmando pelo `[dependencies]` de cada `Cargo.toml`:

| crate | dependencias (`[dependencies]`) |
|---|---|
| `phxsql-cli` | phxsql-core.workspace, phxsql-store.workspace |
| `phxsql-cmd` | phxsql-core.workspace, phxsql-server.workspace |
| `phxsql-core` | (nenhuma) |
| `phxsql-ffi` | phxsql-core, phxsql-store |
| `phxsql-odbc` | phxsql-core |
| `phxsql-server` | phxsql-core.workspace, phxsql-store.workspace, phxsql-sql.workspace |
| `phxsql-sql` | phxsql-core.workspace |
| `phxsql-store` | phxsql-core.workspace |
<!-- /GERADO -->

`phxsql-core` é a base e não depende de nada — nem de outro crate deste
workspace. Todo o grafo de dependência é uma árvore de quatro níveis dentro
da própria casa.

### O que essa escolha pagou, medido

<!-- GERADO: bloco_empacotamento_zero_deps() (docs/EMPACOTAMENTO.md §5) -->
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
<!-- /GERADO -->

É o mesmo motivo, dito de outro jeito, que fez a compilação cruzada para
Windows e ARM funcionar sem uma segunda toolchain de dependências para
resolver (§4 abaixo): não há árvore de crates de terceiros para casar contra
`x86_64-pc-windows-gnu` ou `aarch64-unknown-linux-gnu`, só a `std`, que o
`rustup target add` já resolve.

O custo do lado de dentro: reimplementar JSON, CRC-32, SHA-256/512, HMAC,
PBKDF2, HKDF, Ed25519, X25519, ChaCha20-Poly1305, Base64, ZIP/DEFLATE e
SCRAM-SHA-256 à mão — a tabela da §3 é esse inventário, com o teste que
confere cada um contra vetor publicado.

---

## 3. O que foi escrito à mão, e as normas conferidas

<!-- GERADO: bloco_normas() -->
| arquivo | o que implementa | norma citada no proprio codigo | teste(s) que conferem |
|---|---|---|---|
| `sha1.rs` | SHA-1, so para falar o protocolo do MySQL(R). | FIPS 180-4 | `vetores_do_fips_180_4` |
| `sha512.rs` | SHA-512, conferido contra o FIPS 180-4. | FIPS 180-4, RFC 8032 | `vetores_oficiais` |
| `hash.rs` | SHA-256, HMAC-SHA256 e PBKDF2-HMAC-SHA256, sem dependencias externas. | FIPS 180-4, RFC 2104, RFC 2898, RFC 4231 | `sha256_vetores_oficiais`, `hmac_vetores_rfc4231`, `pbkdf2_vetores_conhecidos` |
| `ed25519.rs` | Ed25519: assinatura com chave publica e privada, conferida contra a RFC 8032. | RFC 8032 | `vetores_da_rfc_8032`, `o_vetor_de_1023_bytes` |
| `x25519.rs` | X25519: a troca de chaves da RFC 7748, sem dependencias externas. | RFC 7748, RFC 8032 | `vetor_1_da_secao_5_2`, `vetor_2_da_secao_5_2` |
| `hkdf.rs` | HKDF-SHA256, a derivacao de chave da RFC 5869, sobre o HMAC que ja existe. | RFC 5869 | `caso_1_do_anexo_a`, `caso_2_do_anexo_a`, `caso_3_do_anexo_a` |
| `cifra.rs` | ChaCha20-Poly1305 (RFC 8439), sem dependencias externas. | RFC 8439, draft-irtf-cfrg-xchacha-03 | `bloco_do_chacha20_bate_com_o_rfc`, `cifragem_do_chacha20_bate_com_o_rfc`, `poly1305_bate_com_o_rfc`, `chave_de_uma_vez_so_bate_com_o_rfc`, `aead_bate_com_o_rfc` |
| `base64.rs` | Base64 (RFC 4648), sem dependencias externas. | RFC 4648 | `vetores_rfc4648` |
| `uuid.rs` | Identificadores: UUID de 128 bits (v4 e v7) e identificador de 256 bits. | FIPS 180-4, RFC 9562 | `v7_tem_o_layout_do_rfc_9562` |
| `crc.rs` | CRC-32 (IEEE 802.3, refletido, polinomio 0xEDB88320). | (nenhuma citada) | `vetores_conhecidos` |
| `json.rs` | Leitor e escritor de JSON, sem dependencias externas. | (nenhuma citada) | (nenhum teste com esse padrao de nome) |
| `zip.rs` | Arquivo ZIP: escrita e leitura, com o DEFLATE escrito aqui. | RFC 1951 | (nenhum teste com esse padrao de nome) |
| `pg/scram.rs` | SCRAM-SHA-256 (RFC 5802 + RFC 7677), do lado do CLIENTE. | RFC 5802, RFC 7677 | `troca_do_rfc_7677` |
<!-- /GERADO -->

A coluna de teste é achada por padrão de **nome** de função (`vetor`, `rfc`,
`fips`, `oficial`, `anexo`, `conhecid`) dentro de cada arquivo da lista — a
lista de arquivos, por sua vez, é fixa no extrator (`ARQUIVOS_NORMA`), porque
"qual arquivo implementa uma norma" é uma decisão de leitura de código que um
script não infere sozinho sem risco de pegar falso positivo. `json.rs` e
`zip.rs` não têm teste com esse padrão de nome porque não conferem contra
vetor de terceiro — JSON não tem "vetor oficial" (é sintaxe, RFC 8259, sem
suíte de casos publicada com a autoridade de um FIPS/RFC de criptografia) e o
DEFLATE de `zip.rs` é conferido por round-trip (escreve, lê, compara) e por
medição de desempenho, não por vetor externo — os dois têm suíte de teste
própria, só não com esse padrão de nome.

Dois hashes fora da tabela porque não citam RFC/FIPS no próprio comentário,
mas valem registrar: `frogcript.rs` (codificação própria do projeto, não uma
norma de terceiro) e `keyenc.rs` (codificação de chave que preserva ordem
para a B+tree — formato próprio, documentado em `docs/FORMATO.md`, não em
RFC nenhuma).

### A B+tree e o formato em disco

O índice (`crates/phxsql-store/src/ndx.rs`) é uma B+tree escrita do zero,
sem norma externa — o formato é próprio do PhxSql e a especificação vive em
`docs/FORMATO.md`. A garantia que substitui "conferir contra vetor" aqui é
outra: cada entrada de folha grava a chave do usuário seguida do `rowid` em
big-endian, o que faz toda chave completa ser única e a comparação byte a
byte desempatar por `rowid` sem ambiguidade — provado por teste de
propriedade (`mod tests`/`mod testes` do próprio `ndx.rs`), não por vetor de
terceiro, porque não há terceiro: o formato é nosso.

### A rodada das dezoito capacidades do comparativo, e das junções

Escrito à mão nesta rodada, sem depender de crate nenhuma — a mesma disciplina
das normas acima, aplicada à camada SQL. As contagens de linhas e testes de
cada peça saem de `python3 docs/tecnologias/extrair.py` (que chama
`cargo test`, então não rodam aqui); o que segue é o mapa de arquivo e
documento, para quem for medir depois.

| peça | onde mora | documentado em |
|---|---|---|
| Expressões no esquema (`padrao`, `check`, coluna `calculada`, índice parcial por `onde`, índice por expressão) | `phxsql_core::expressao` — a mesma gramática que os gatilhos já usavam | `docs/SQL.md`, `CHANGELOG.md` («Não lançado — as dezoito do comparativo…») |
| `agrupar` (`GROUP BY` genérico, com os agregadores do `pivotar`) | operação `agrupar` no catálogo | `docs/SQL.md` |
| `consultar` (composição de sub-pedidos: `de`, `em` para `IN (SELECT…)`, `escalar`, `janela`, `juntar` por igualdade com nome qualificado) | `crates/phxsql-server/src/consultar.rs` | `docs/SQL.md` §4 e §7 |
| Visões (`criar_visao`, `visoes`, `excluir_visao`) | catálogo — a visão guarda texto, analisado a cada uso dentro de um `consultar` | `docs/SQL.md` |
| `diferencas` (diz ONDE duas tabelas com a mesma chave única divergem, não só SE divergem) | extraído do `aplicar_para_ca` do DbLink para um lugar só | `CHANGELOG.md`, `docs/DBLINK.md` |
| Direito por COLUNA (`tabelas.<t>.colunas`, com `ler`/`alterar` por coluna) | `crates/phxsql-server/src/direito_coluna.rs` | `docs/SEGURANCA.md` |
| PITR (`restaurar_backup` com `ate`/`ate_ms`, reaplicando o diário vivo pelo mesmo `Table::aplicar_evento` da replicação) | `crates/phxsql-server/src/servidor.rs` (aplica, não julga) | `docs/RESTAURACAO.md` |
| ODBC com parâmetros (`SQLBindParameter` ligando o `?` do lado do driver, `sql.parametros` do lado do servidor) | `crates/phxsql-odbc/`, léxico por TOKEN em `crates/phxsql-sql` | `docs/ODBC.md` |
| Semijunção — **em curso, não fechada**: `existe` (semijunção por espalhamento, pedido 236) é proposta, ainda sem o modelo tipado do `consultar` que ela exigiria | — (proposta) | `docs/propostas/comparativo-19.md` §«existe — semijunção por espalhamento», `docs/MODELOS.md` (linha **C20-CONSULTA**) |

---

## 4. As ferramentas do trabalho

### 4.1 Como se orquestrou

A cláusula pétrea dos dez papéis (`~/.claude/CLAUDE.md`, ecoada em
`CLAUDE.md` do projeto) governa quem convoca quem. A escolha de **escalão**
de modelo por frente — nunca o nome do modelo, só "projeto e risco" contra
"mecânico e verificável" — fica registrada em `docs/MODELOS.md`:

<!-- GERADO: bloco_modelos() -->
`docs/MODELOS.md` registra **17** rodadas: Rodada de 1–2 de setembro de 2026 — NÃO CUMPRIDA; Frente «toda tabela é PhxGrid» — 2 de setembro de 2026; Rodada das sprints abertas — 4 de setembro de 2026; Rodada do comparativo — 7 de setembro de 2026; Rodada do batimento e dos geradores — 7 de setembro de 2026; Rodada da pergunta e do botão — 7 de setembro de 2026; Rodada do quórum — 7 de setembro de 2026; Rodada das 26 perguntas — 7 de setembro de 2026; Rodada das diretivas HFSQL e do fluxo do auto number — 7 de setembro de 2026; Rodada de 7 de setembro de 2026 (noite) — channel binding do login; Rodada de 8 de setembro de 2026 — onda dos gaps (4 frentes paralelas); Onda 2 da rodada dos gaps — 8 de setembro de 2026 (as 4 frentes cifradas); Rodada do acelerador de memoria (o `.tbm`) — 8 de setembro de 2026; Rodada da corrida de I/O — 8 de setembro de 2026; Rodada das dezoito do comparativo — 8 de setembro de 2026; Rodada dos limites nomeados — 9 de setembro de 2026; Rodada dos limites nomeados e dos gaps — 9 de setembro de 2026 (continuação)
<!-- /GERADO -->

A contagem de rodadas sai do bloco acima, não daqui; o que elas ensinam, não.
Duas valem o exemplo: a primeira, a "Rodada de 1–2 de setembro de 2026",
entrou como **NÃO CUMPRIDA** — sem nenhum agente convocado depois da retomada
da sessão, e o próprio documento diz isso, porque "papel que não está
cumprindo tem de aparecer como não cumprindo"; e a "Rodada do comparativo",
que registra **dez papéis um a um** — cinco convocados e cinco **dispensados
com o motivo escrito**, que é o que a cláusula realmente cobra.

### 4.2 Como se mediu

<!-- GERADO: bloco_bancadas() -->
`bancada/` tem **50** frentes de medicao (acid, alfanumerica, alter, arm, bateria, carga, cifra, cifra-do-fio, cluster, cobertura-da-tela, comparacao, comparativo, concorrencia, conexoes, dblink, diretivas, dns-cloudflare, docker, durabilidade, embutido, exclusao, fts, gaps-sql, gestao, guardas, jobs, manual, mvcc, odbc, pacote, particao-por-faixa, phxsql, pitr, profiler, proibidos, quorum, registro, replicacao, rest, rotinas, seguranca, sequencias, servermail, sql-exemplos, sqlite, telemetria, transacoes, usuarios, utilizacao-padrao, windows), das quais **37** documentam a propria metodologia em `LEIA-ME.md`.
<!-- /GERADO -->

A carga do lado do motor é
`crates/phxsql-store/examples/carga.rs`, rodando cada fase num processo
separado para que os contadores de `/proc` sejam só daquela fase. As quatro
regras que fazem a bancada contra outros motores valer — mesmos dados, mesmo
esquema, mesma forma de pergunta, mesma quantidade de trabalho — estão em
`bancada/LEIA-ME.md`, com os dois erros reais já cometidos aqui: um
`WHERE id IN (…)` contra vinte mil buscas separadas (41× a favor do MySQL(R)
pela forma da pergunta) e um `COUNT(*)+SUM` sobre 1.250.000 linhas contra a
leitura de 20.000 (5× a favor do PhxSql sem o motor ter feito nada por isso).

### 4.3 Como se provou

<!-- GERADO: bloco_conferidores() + bloco_catracas() + bloco_guardas() -->
Conferidores em `crates/phxsql-server/src/`: `conferidor.rs`, `conferidor_botoes.rs`, `conferidor_dependencias.rs`, `conferidor_grades.rs`, `conferidor_inventario.rs`, `conferidor_temporarios.rs`, `conferidor_vermelhas.rs`. Executaveis de prova em `crates/phxsql-server/examples/`: `botoes-sem-prova.rs`, `grades-fora-do-padrao.rs`, `prova-dblink.rs`, `prova-exportar.rs`, `textos-fora-da-fabrica.rs`.

| constante | valor | arquivo |
|---|---:|---|
| `TETO_COLADO` | 0 | `crates/phxsql-server/src/conferidor.rs` |
| `TETO_FRASE_REPETIDA` | 0 | `crates/phxsql-server/src/conferidor.rs` |
| `TETO_ROTULOS_E_CRASE` | 1_049 | `crates/phxsql-server/src/conferidor.rs` |
| `TETO_BOTAO_SEM_PROVA` | 194 | `crates/phxsql-server/src/conferidor_botoes.rs` |
| `TETO_TABELA_NA_MAO` | 0 | `crates/phxsql-server/src/conferidor_grades.rs` |
| `TETO_INVENTARIO_DESCASADO` | 0 | `crates/phxsql-server/src/conferidor_inventario.rs` |
| `TETO_TEMP_DIR_SOLTO` | 0 | `crates/phxsql-server/src/conferidor_temporarios.rs` |
| `TETO_VERMELHA_SEM_PEDIDO` | 0 | `crates/phxsql-server/src/conferidor_vermelhas.rs` |
| `TETO_DO_CAMPO` | 120 | `crates/phxsql-server/src/profiler.rs` |
| `TETO_DO_ERRO` | 500 | `crates/phxsql-server/src/profiler.rs` |
| `TETO_DO_CABECALHO` | 400 | `crates/phxsql-server/src/profiler.rs` |
| `TETO` | Duration::from_secs(60) | `crates/phxsql-server/src/replica.rs` |
| `TETO_DO_LOTE_SERVIDO` | 16 * 1024 * 1024 | `crates/phxsql-server/src/servidor.rs` |
| `TETO_PIVOT` | 5_000_000 | `crates/phxsql-server/src/servidor.rs` |
| `TETO_JUNCAO` | 500_000 | `crates/phxsql-server/src/servidor.rs` |
| `TETO_ANINHAMENTO` | 8 | `crates/phxsql-server/src/servidor.rs` |

**16** catracas (`TETO*`) encontradas em `crates/phxsql-server/src/`.

`bancada/guardas/catalogo.py` cataloga **124** defeitos repostos, contados de `len(GUARDAS)` depois de importar o modulo (nao por regex no texto -- entradas com `trocas` tem mais de um `{` cada, e uma contagem de chaves as conta em dobro ou mais). Linhas do arquivo: 4572. Refazer a prova: `python3 bancada/guardas/provar-guardas.py`.
<!-- /GERADO -->

- **Ponta a ponta, pelo navegador**: os arquivos `.mjs` de `testes-web/` —
  contados no bloco de outras linguagens (§1.3), não redigitados aqui — falam com o
  servidor de verdade pelo soquete e pela tela, não com um duplo em memória.
  `bateria.mjs` é o comando que roda tudo.
- **Conferidores de estilo/texto, com catraca que só desce.** Os
  conferidores, os seus executáveis de prova e as constantes `TETO*` com o
  valor de cada uma saem do bloco gerado logo acima (`bloco_conferidores()` +
  `bloco_catracas()` + `bloco_guardas()`), que **acha os arquivos no disco** e
  **lê a constante no fonte** — não são digitados aqui. Nem toda `TETO*` é
  catraca de varredura: parte é **limite de funcionamento** (tamanho de campo
  e de lote no `profiler.rs` e no `servidor.rs`), e confundir as duas é o erro
  que `docs/CATRACAS.md` existe para não cometer.

  **A lição que este trecho pagou, e que só fecha nesta rodada:** ele já
  esteve marcado `GERADO` trazendo números **digitados à mão embaixo da
  marca** — em 07/09/2026 dizia `TETO_ROTULOS_E_CRASE = 1.720` com o medido em
  1.051, e «ao todo 10 constantes» quando eram outras. *Marca de gerador não
  era gerador.* Esta rodada fecha o buraco: o `docs/tecnologias/extrair.py`
  agora **escreve** o número dentro da marca, e o `portao-dos-geradores.py`
  reprova quem editar à mão — a marca passou a ser cumprida, não só declarada.
- **Guardas — prova de que a prova pega**: `bancada/guardas/catalogo.py`
  cataloga os defeitos repostos (a contagem sai do bloco acima, de
  `len(GUARDAS)`, não de regex), cada um com o trecho de código de hoje, o
  trecho de antes do conserto, e os testes que têm de cair quando o defeito
  volta. `python3 bancada/guardas/provar-guardas.py` copia a árvore, repõe
  cada defeito, roda os testes nomeados e julga — prova real nos dois
  sentidos, não só "o teste existe". **Por que a contagem sai de
  `len(GUARDAS)` e não de regex**: a versão anterior de `bloco_guardas()`
  contava `"{" seguido de quebra de linha` no texto do arquivo, e isso conta
  certo só enquanto toda guarda é um dicionário raso. As guardas que usam o
  campo `trocas` — a lista documentada no próprio `catalogo.py` para o defeito
  que mexe em mais de um ponto — trazem sub-dicionários `{arquivo, trecho,
  troca}` que batem no mesmo padrão sem ser guarda nova, e o número inflava
  por isso: uma leitura da regex deu **142**, e uma nova rodada dela, sem
  nenhuma guarda a mais, deu **180** — o padrão nunca teve relação estável com
  a contagem certa. Corrigido para importar o módulo e contar `len(GUARDAS)`,
  do mesmo jeito que `bancada/guardas/tabela-no-testes.py` já fazia ao lado.
- **Executáveis de prova dedicados**, em `crates/phxsql-server/examples/` —
  achados por glob e nomeados no bloco acima, não listados de novo aqui, pelo
  mesmo motivo de sempre: lista digitada envelhece calado.
- **A cobertura por área**, já contada e mantida por outro gerador desta
  mesma casa (não duplicado aqui): `docs/dossie/cobertura-por-area.py`
  regrava a tabela de `docs/TESTES.md` §1 a partir de `#[test]` por arquivo,
  agrupados por assunto — o documento certo para "quantos testes tem a
  criptografia" ou "quantos tem o DbLink" é aquele, não este.

### 4.4 Como se compilou para outra arquitetura

<!-- GERADO: bloco_empacotamento_plataformas() (docs/EMPACOTAMENTO.md §7.7) -->
| Plataforma | Estado | O que falta |
|---|---|---|
| Linux x86-64 | **roda, exercitado** | — |
| Windows x86-64 | **roda: gravou e leu 50 linhas sob `wine`** (§6.1) | um Windows de verdade, para desempenho e para o driver ODBC |
| Linux ARM64 / ARMv7 | **roda: gravou e leu 50 linhas sob emulação** | o desempenho real, que só a placa mede |
| Android (Termux) | **compila; link precisa do NDK** | o NDK, e uma corrida real |
| Android (dentro de app) | **a biblioteca existe e roda** (`cdylib`, provada em x86-64 e ARM64) | a camada JNI, e o NDK para o alvo bionic |
| iOS | **a biblioteca existe e roda** (`staticlib` aarch64, exercitada sob emulação) | Mac com Xcode, o alvo `aarch64-apple-ios`, e o invólucro em Swift |

O `phxsqld` como daemon continua **não** sendo o caminho nesses dois últimos, e
isso não é limitação nossa: é o que os dois sistemas permitem.
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
<!-- /GERADO -->

### 4.5 Testes, medidos agora

<!-- GERADO: bloco_testes() -->
`cargo test --workspace`: **2275** testes passaram, **0** falharam (medido em 2026-09-12 07:54:27, commit `8e578531`, do `CAPABILITIES.json`).
<!-- /GERADO -->

Esta é a única linha deste documento que muda legitimamente a cada rodada, e
por um motivo bom: testes entram na árvore. O que **não** muda mais é de onde
o número vem. Até 11/09/2026 este extrator rodava o seu próprio
`cargo test --workspace` e somava os `test result:` — uma **segunda** medição,
paralela à do `numeros-do-projeto.py`, que já mede a mesma coisa e a escreve no
README, no `docs/TESTES.md` e no `CAPABILITIES.json`. Duas medições da mesma
coisa divergem no dia em que uma é recolada e a outra não: foi assim que este
documento passou a dizer **1.659** enquanto o resto do projeto já media
**2.209** — 550 testes atrás, calados. Agora o número sai do
`CAPABILITIES.json`, fonte única, e este bloco nem toca no `cargo` (lê um
JSON). Quem **aborta se a suíte falhar** é o `numeros-do-projeto.py` que emite
esse arquivo; então, se o `CAPABILITIES.json` existe, nenhum teste falhou.

(O README e o `docs/TESTES.md` §1, mantidos pelo mesmo
`docs/dossie/numeros-do-projeto.py`, registram o mesmo número — por construção,
não por coincidência: os três leem o `CAPABILITIES.json`. O buraco que a
cognição de 11/09/2026 registrava — este extrator **imprimia** e alguém
**colava**, fora de qualquer portão — está fechado nesta rodada: ele agora
**escreve** os blocos no lugar, e o `docs/dossie/portao-dos-geradores.py`
reprova quem editar um número à mão.)

### 4.6 Como se prova uma garantia DE TIPO — o par de doctests

Ferramenta nova de 05/09, e ela merece uma linha porque não é teste de
comportamento: é teste de que **um erro deixou de ser possível**.

Quando a garantia é «este tipo não consegue escrever», o teste não pode rodar
o código — ele tem de provar que o código **não compila**. O Rust dá isso de
graça, sem nenhuma dependência, num doctest marcado `compile_fail`, e
`cargo test --workspace` já o roda:

```rust
/// ```compile_fail
/// fn grava(t: &mut phxsql_store::leitura::TabelaLeitura) { let _ = t.inserir(&[]); }
/// ```
```

**E ele nunca vai sozinho.** Um `compile_fail` passa por QUALQUER erro de
compilação — um nome de método digitado errado o faz «passar» sem provar nada,
que é a definição de teste que passa por engano. Então ele vem em par com um
**controle** que tem de compilar, com o mesmo corpo contra o tipo que
legitimamente escreve:

```rust
/// ```
/// fn grava(t: &mut phxsql_store::table::Table) { let _ = t.inserir(&[]); }
/// ```
```

Se alguém errar o nome do método, o controle cai. Se alguém der à fachada um
`Deref` para o tipo gravável, o `compile_fail` cai. É a mesma exigência de
sempre — *prova real é nos dois sentidos* — aplicada onde não há execução para
observar.

Custo: zero dependências, zero tempo de execução, e aparece na saída do
`cargo test` como dois testes com o sufixo `- compile fail`.

---

## 5. O que foi avaliado e RECUSADO, com o número

Esta é a seção que mais poupa tempo depois — recusa medida impede a mesma
proposta de voltar sem medição nova.

### 5.1 Pedidos recusados, do próprio `PENDENCIAS.md`

<!-- GERADO: bloco_recusados() -->
`docs/PENDENCIAS.md` tem **245** pedidos numerados; **28** trazem a palavra RECUSADO no proprio texto:

| # | pedido |
|---:|---|
| 83 | **Comandos SQL reconhecem `matriz.estoque` e `filial.estoque`** |
| 101 | **Cifrar e compactar `.log`, `.trash` e `.reason`** |
| 114 | **Índice não único fora do caminho crítico** |
| 148 | **`ALTER TABLE ADD COLUMN` preservando o rowid** (sprint 25) |
| 153 | **Criar VM para provar o binário Windows e o Android** |
| 156 | **Auditoria técnica externa da 0.18.0 — «melhorias»** |
| 159 | **Furo na numeração por exclusão: renomear entra, renumerar fica RECUSADO** |
| 160 | **Phoenix Web Absorber FX SDK — RECUSADO com número** |
| 161 | **Impressão / relatório — RECUSADO por escopo** |
| 169 | **A cascata só planejava um nível — a três, recusava DEPOIS de gravar** |
| 174 | **A auto-referência sai da cascata em SILÊNCIO — e os dois motores de referência RECUSAM** |
| 176 | **A tabela que aponta para si devolve «o índice ficou para trás numa queda» quando não houve queda nenhuma** |
| 180 | **O comboio do fecho de janela é real, e nem `RwLock` nem MVCC o consertam** |
| 186 | **O fecho da janela de durabilidade não sincronizava o arquivo de DADOS** |
| 191 | **Bateria de testes de utilização padrão: criar base, incluir 20.000 registros em tabela complexa, com e sem binários e memos** |
| 192 | **Testes de paginação alfabética** |
| 194 | **Senha própria por tabela na cifra em repouso — medir primeiro, decidir depois** |
| 196 | **A lista `cifra.tabelas` no `config.json`, qualificada por banco — e o campo diz o que faz e o que NÃO faz** |
| 200 | **Índice de texto (`.fts`): o motor fecha a maior lacuna do HFSQL(R), e a tela ainda não** |
| 203 | **A réplica com credencial recusada bloqueia o IP — e derruba o operador junto** |
| 204 | **O batimento de comunicação de 15 em 15 minutos** |
| 215 | **Injeção de SQL não bloqueia ninguém: 311.250 tentativas por minuto sem entrar na blacklist** |
| 217 | **Escalonar o cluster a quente não funciona — e o caminho que funciona custa 0,367 s no master, sem eleição** |
| 219 | **Três recusas do SQL que dizem a coisa errada, achadas exercitando os 56 comandos** |
| 220 | **`comandos_proibidos` é global, não por banco — e o pedido era «para um banco x»** |
| 224 | **O catálogo documenta valores que o motor recusa: `unir` com `distinto`, e o exemplo colável do `pivotar`** |
| 229 | **Auto number: tres defeitos de produto medidos pela F8** |
| 245 | **Seis observações da revisão do motor, menores, para varrer numa rodada de higiene (O1-O6)** |
<!-- /GERADO -->

Os dois mais relevantes para este documento —
porque avaliam receita de fora contra o nosso gargalo, o mesmo teste que a
cláusula pétrea do pesquisador exige:

- **#160 — Phoenix Web Absorber FX SDK, RECUSADO com número.** O SDK nasce
  com as mesmas restrições desta casa (Rust `std`-only, zero dependências,
  ES5, preview offline sem CDN) — raro e considerável — mas o buraco que ele
  tapa não existe aqui: **47 tokens de CSS usados na interface, 46
  definidos**, e o único ausente já tem *fallback*. O `fx-grid` dele
  (2.211 linhas) também não compra nada: o `PhxGrid` (1.837 linhas) já
  agrupa, ordena, filtra, busca, congela, exporta CSV, totaliza e pagina. O
  que foi absorvido é a **ideia**, não o código — a regra "CSS sempre
  renderizável" virou a guarda `token_sem_definicao_e_sem_fallback` em
  `conferidor.rs`, escrita do zero. Nenhuma linha do SDK entrou.
- **#161 — Impressão/relatório, RECUSADO por escopo.** A lacuna real que a
  avaliação do FX SDK expôs: o PhxSql não imprime nada, medido — não há
  `window.print` nem `@media print` na interface inteira. Fica fora por três
  motivos medidos: não está em nenhuma das 55 sprints do roteiro; o caminho
  crítico até 1.0 é motor (concorrência, MVCC, SQL), não tela; e exportar já
  existe (CSV, XLSX, DOCX), que resolve "levar o dado para fora". Volta se
  alguém precisar do relatório paginado com cabeçalho e rodapé — e não há
  esse pedido ainda.

Os outros sete pedidos com RECUSADO no texto (não citados por extenso aqui,
ver a fonte): #83 (qualificação `matriz.estoque`/`filial.estoque` em SQL),
#101 (cifrar/compactar `.log`/`.trash`/`.reason`), #114 (índice não único
fora do caminho crítico — reaberto e resolvido de outra forma na §5.3
abaixo), #148 (`ALTER TABLE ADD COLUMN` preservando `rowid`), #153 (VM
dedicada para provar Windows/Android), #156 (uma das "melhorias" da
auditoria externa) e #159 (renumerar por exclusão — RECUSADO; renomear
entrou no lugar).

### 5.2 GPU/CUDA — recusado com número, `docs/GPU.md`

<!-- GERADO: bloco_gpu_veredito() -->
> **Não compensa, e não é por pouco.** O trabalho pesado deste motor não é
> aritmético: **99,4% de uma inserção** é descida de B+tree e escrita — o
> CRC-32, único candidato lá dentro, custa **0,58%**, e mesmo instantâneo
> deixaria a inserção 1,006× mais rápida. No backup, o maior bloco contíguo de
> CPU que este motor produz, **63,0% é DEFLATE** — busca de repetição num
> dicionário que depende do byte anterior, o oposto do que uma GPU acelera — e
> o SHA-256, que é o candidato, é **12,1%**: de graça, o backup ganharia
> **1,14×**.
>
> **A agregação morre na conta do barramento, e morre em qualquer tamanho:**
> o `SUM` sobre uma coluna anda a **28.234 MiB/s** nesta CPU, **1,79× o pico
> teórico do PCIe 3.0 x16**. Não há limiar que conserte — a CPU já consome os
> bytes mais depressa do que o barramento os entregaria.
>
> **O que o dono pediu — mais velocidade no processamento pesado — existe, e
> sem CUDA:** dividir pelos 4 núcleos com a `std` que já está aqui dá
> **3,90× no ChaCha20-Poly1305, 3,59× no CRC-32 e 2,51× no SHA-256**, medidos;
> e `ORDER BY` tem **1,51×** parado numa troca de algoritmo de ordenação que
> não depende de placa nenhuma.
<!-- /GERADO -->

Medido na própria máquina desta casa (4 núcleos, sem `/dev/nvidia*`, sem
`nvcc`/`nvidia-smi`/`clinfo`/`rocm-smi`, nenhuma lib CUDA/OpenCL no
`ldconfig`): `cargo run --release --example onde-a-gpu-ajudaria -- 1000000`.

### 5.3 A arquitetura LSM/WAL — cinco já existiam, duas miravam gargalo que não temos, uma quebraria o formato, duas eram reais

<!-- GERADO: bloco_dez_propostas() (docs/DESEMPENHO.md §3) -->
| # | Proposta | Estado no PhxSql | Veredito |
|---:|---|---|---|
| 1 | WAL exclusivamente sequencial | O `.reg` **já é** *append-only*: `rowid = slots + 1`, endereço por multiplicação, nenhuma página reescrita | **Aponta para o arquivo errado.** Um WAL existe para transformar escrita aleatória de página em sequencial. Não há escrita aleatória no `.reg` — há no `.ndx` |
| 2 | MemTable em RAM | Existe `TabelaMemoria`/`SelectMemory` (87× medido), mas é cache de **leitura** | **Meia peça, do outro lado.** Como buffer de escrita ajudaria o `.ndx` |
| 3 | Single writer + fila MPSC | O servidor **já** serializa tudo numa trava global única | **Já é assim** — e o roteiro quer o contrário: trava por tabela. O gargalo de concorrência é o excesso de serialização, não a falta |
| 4 | Três modos de durabilidade | Existem, com esses três nomes: `por_operacao`, `por_lote`, `sistema` | **Já existe, e medido:** 1.289 → 18.264 → 24.858 → 26.301 linhas/s (20,4×) |
| 5 | Não atualizar índice secundário na hora | Todos os índices são mantidos dentro da inserção | **REAL, e é o maior.** Ver §4 |
| 6 | UUID v7 ou sequência, nunca v4 | `Uuid` v4/v7 (RFC 9562), `Uuid256` e `Sequence` prontos; o dossiê tem uma seção sobre por que v7 | **Já existe** |
| 7 | Não alterar o arquivo principal no INSERT | O `.reg` só anexa. Sem *double-write*, sem divisão de página no arquivo de dados | **Já é assim** |
| 8 | Segmentos imutáveis, SSTable, compactação | — | **Incompatível.** Ver §5 |
| 9 | Buffers grandes em vez de escritas pequenas | Escreve por slot; são 2,06 páginas de `.ndx` gravadas por linha, medidas | **Medido, e é pequeno.** Um `lseek` custa 0,10 µs: mesmo 41 chamadas por linha dariam 4,0 µs de 15,9. O que custa nessas gravações é o **CRC** (4,8 µs), não a chamada |
| 10 | Pré-alocar o WAL | Os volumes crescem conforme escrevem | **Aplicável aos volumes**, ganho provavelmente pequeno pela mesma razão do item 9 |
<!-- /GERADO -->

O item 5 — índice fora do caminho crítico — é o único que a medição sustenta
com número grande, mas só a metade dele: índice **não único** adiado é
seguro (ganho medido 1,45×); índice **único** adiado não é, porque a
conferência de unicidade tem de acontecer antes de qualquer escrita — o
`.reg` nunca reaproveita slot, e uma inserção recusada depois de gravar
deixaria um buraco permanente. Medido também contra a hipótese errada: um
terceiro caminho ("ordenar as chaves do lote antes de descer a árvore")
tinha o alvo certo (83,5% do tempo de inserção estava mesmo no `.ndx`) e a
causa errada (não era localidade, era reler e recalcular o CRC-32 da mesma
página a cada descida) — um cache de páginas de leitura comprou 2,40×; ordenar
teria comprado quase nada e custado uma garantia. É o pedido 113/114 do
`PENDENCIAS.md`, e a frase que resume a lição está no `CLAUDE.md` do
projeto: *medir a premissa do item vem antes de implementar o item —
inclusive quando o item é nosso.*

### 5.4 Transações: o que não entrou, e por quê — `docs/TRANSACOES.md` §11

<!-- GERADO: bloco_transacoes_nao_entrou() -->
Subsecoes de `docs/TRANSACOES.md` §11 ("O que NAO entrou, e o motivo de cada um"):

- 11.1 MVCC — não implementar
- 11.2 WAL, undo log, PageLSN, full-page-write, VACUUM
- 11.3 Detecção de deadlock
- 11.4 DDL transacional
- 11.5 Transação entre databases
<!-- /GERADO -->

- **11.1 MVCC — não implementar.** Aqui o `rowid` é o endereço; uma segunda
  versão da linha pediria um segundo `rowid`, quebrando a ordem de digitação
  e a replicação (`aplicar_evento` para quando o `rowid` diverge do que o
  source mandou). É incompatibilidade de formato, não falta de vontade.
  *Readers non-blocking* — metade do que se quer do MVCC — já existe por
  outro caminho, sem MVCC: nada vai a disco antes do `COMMIT`, então um
  leitor nunca vê escrita não confirmada e nunca espera por escritor.
- **11.2 WAL, undo log, PageLSN, full-page-write, VACUUM — não entram**
  porque resolvem um problema que este desenho não tem: não há página suja
  confirmada para refazer nem versão velha para limpar. O full-page-write
  foi conferido à parte: o `.reg` já detecta escrita rasgada por CRC-32 por
  slot e já repara pelo espelho `.bkp` — o que faltaria é uma terceira cópia
  para um caso que o espelho já cobre.
- **11.3 Detecção de deadlock — não entra nesta rodada.** Entre tabelas
  declaradas, a ordem canônica já *impede* o ciclo (mais forte que
  detectá-lo); entre linhas, o `LOCK TIMEOUT` o transforma num erro nomeado.
  Entraria com uma medição mostrando espera de `LOCK TIMEOUT` cheio em
  produção — que não existe ainda.
- **11.4 DDL transacional — não implementado nesta rodada.** O `ALTER TABLE
  ADD COLUMN` já tem duas fases e ponto de compromisso, mas DDL **dentro**
  de uma transação de linha é recusado, e a recusa é explícita — nunca
  confirmado em silêncio.
- **11.5 Transação entre databases (*two-phase commit*) — recusa
  fundamentada**, ver `docs/TRANSACOES.md` §2.3.

### 5.5 O que a comparação com motores maduros deixou de fora, e por quê

<!-- GERADO: bloco_comparacao_fora() (docs/COMPARACAO.md) -->
**`OPTIMIZE TABLE` (compactação).** Aqui ele esbarra numa regra do projeto: o
`.reg` **nunca reaproveita slot excluído**, e a ordem de digitação é a garantia
que o TopSpeed(R) não dava. Compactar significa reescrever `rowid`, e `rowid` é
endereço — quem guardou um passa a apontar para outra linha. Uma tabela com
muitas exclusões cresce e não encolhe, e isso é hoje uma **consequência aceita**
da garantia, não um esquecimento. Mudar exige a sua decisão, não a minha.

**`ANALYZE TABLE` (estatísticas para o planejador).** Não há planejador: quem
escolhe o índice é quem escreve a operação. Estatística sem consumidor é
arquivo para manter atualizado sem ninguém ler.

**`EXPLAIN`.** Faz sentido **depois** da camada SQL. Antes dela, o equivalente
honesto seria «esta junção vai ler N linhas de A e M de B» — e isso as
estatísticas já contam depois do fato.

**`information_schema` com 79 tabelas.** O catálogo daqui são `sistabelas` e
`siscolunas`, e eles cobrem o que existe. Tabela de catálogo para recurso que
não existe seria promessa em forma de esquema.

**Transações, `SAVEPOINT`, `XA`, tabelas temporais, replicação de verdade.**
São recursos, não superfície de operação — já estão no roteiro com o que falta
de cada um.
<!-- /GERADO -->

### 5.6 TLS e a Sombra — parados por decisão do dono, não por falta de código

Os dois itens mais recentes do comparativo de 19 capacidades (`docs/
COMPARATIVO.md`) que continuam `❌` para o PhxSql, e por que não são buraco:

- **TLS no transporte.** Esbarra na mesma pétrea das zero dependências: TLS
  1.3 em casa exigiria X.509/ASN.1, assinatura ECDSA P-256 (Ed25519 os
  navegadores não aceitam em certificado) e gestão de certificado — uma
  superfície inteira de crate ou de código próprio que nenhuma outra peça
  desta casa pediu ainda. A cifra do fio (Noise, `cifra.rs`) já protege a
  porta de dados; o que falta é só o canal do navegador. Avaliado com o custo
  em `docs/propostas/comparativo-19.md` («TLS no transporte»).
- **Isolamento acima de `READ COMMITTED` — a Sombra.** O desenho existe e
  está medido (`docs/SOMBRA.md`): compra leitura repetível fechando o
  fantasma e tornando o *write skew* sistemático, ao custo de uma faixa
  estreita ao redor de 1× em `por_lote` (o regime padrão) — a pesquisa mede
  1,00×–1,21× conforme a carga, e é mais cara em `por_operacao`. **O dono
  decidiu não construir agora** (05/09/2026): a urgência que justificava
  decidir cedo morreu quando a Sombra em RAM deixou de exigir mudança de
  formato em disco — decidir «quando um cliente pedir leitura repetível»
  passou a custar o mesmo que decidir hoje. Números e a ordem completa das
  alternativas mais baratas em `docs/SOMBRA.md` §3 e §6.

---

## 6. O que este documento NÃO conseguiu medir

Registrado em vez de estimado, como a regra manda:

- **Volume de linhas por commit/autor ao longo do tempo.** O histórico git
  não foi somado aqui; quem quiser essa série tem `git log --numstat`, mas
  não é este extrator que a produz.
- **Cobertura de linha/branch do `cargo test`** (percentual de código
  executado pelos testes). O projeto mede **quantidade** de testes por área
  (`docs/TESTES.md`, gerado por `docs/dossie/cobertura-por-area.py`), não
  cobertura de linha — não há ferramenta de cobertura no `Cargo.toml` porque
  isso quebraria a regra de zero dependências externas para instrumentar o
  binário, e não foi avaliada nesta rodada.
- **Tamanho do binário final por plataforma**, além do que
  `docs/EMPACOTAMENTO.md` já mede (6,8 MB citados na §4.4 acima, para o
  embutido) — os binários de servidor/CLI completos não foram remedidos
  aqui.

---

## Como se refaz

```bash
python3 docs/tecnologias/extrair.py
```

Sem argumento — lê o repositório onde está (raiz calculada a partir do
próprio caminho do script) e **escreve** cada bloco no lugar, entre a sua
marca `<!-- GERADO: … -->` e o `<!-- /GERADO -->`, sem tocar na prosa em volta
(`--imprimir` mantém o comportamento antigo, de só imprimir). Ele **não roda
`cargo`**: o número de testes sai do `CAPABILITIES.json`, fonte única emitida
pelo `numeros-do-projeto.py`, então esta rodada não disputa disco nem árvore
com quem estiver compilando. O que custa é tokenizar o workspace Rust inteiro
para separar código de teste — é demorado, mas não contamina nada.
