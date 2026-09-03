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
| `phxsql-cli` | 1 | 819 | 79 | 106 | 73 | 1077 |
| `phxsql-cmd` | 2 | 579 | 110 | 171 | 62 | 922 |
| `phxsql-core` | 26 | 8180 | 2441 | 2047 | 1090 | 13758 |
| `phxsql-ffi` | 7 | 1407 | 1014 | 697 | 235 | 3353 |
| `phxsql-odbc` | 6 | 1637 | 281 | 365 | 123 | 2406 |
| `phxsql-server` | 40 | 31444 | 13611 | 11519 | 3187 | 59761 |
| `phxsql-sql` | 6 | 3495 | 1069 | 554 | 321 | 5439 |
| `phxsql-store` | 18 | 10985 | 2445 | 3042 | 1237 | 17709 |
| **total** | **106** | **58546** | **21050** | **18501** | **6328** | **104425** |

Proporção teste/código (só `src/`, sem comentário nem linha vazia):
**21.050 / 58.546 = 0,36×** — pouco mais de uma linha de teste para cada três
de produto. `phxsql-server` sozinho concentra mais da metade do código
(31.444 de 58.546 linhas, 54%) e quase dois terços do teste (13.611 de
21.050, 65%): é onde mora o protocolo, o SQL embutido nas operações, a
interface HTTP e a réplica — `phxsql-cli` e `phxsql-cmd`, as duas
ferramentas de linha de comando mais finas, têm a proporção mais baixa
(0,10× e 0,19×) porque a lógica que importa já está testada nas camadas de
baixo que elas chamam.

Contagem feita **por um tokenizador de Rust escrito para este extrator**
(comentário de linha e de bloco, string, string bruta `r#"…"#` e caractere
apagados antes de contar chaves), não por `grep` de `{`/`}` — este projeto
tem um parser de JSON (`crates/phxsql-core/src/json.rs`) com literais de
caractere como `'{'` e `'}'` no próprio código, que uma contagem ingênua de
chaves confundiria com abertura/fechamento de bloco de teste. O classificador
foi conferido contra `wc -l` linha a linha em arquivos de amostra antes de
rodar no workspace inteiro.

Além do `src/`: **36** programas de medição em `examples/` (6.806 linhas —
bancada em Rust, não produto nem teste unitário) e **36** arquivos de teste
de integração em `tests/`, fora de `src/` (12.498 linhas).

### 1.2 A interface web embutida no binário

<!-- GERADO: bloco_interface() -->
Lista extraída de `crates/phxsql-server/src/http.rs` — todo
`include_str!`/`include_bytes!` que aponta para dentro de `ui/` — e não
digitada. É a mesma lista, lida do mesmo lugar, que decide o que o binário
`phxsqld` embute:

| arquivo embutido | linhas | KiB |
|---|---:|---:|
| `ui/index.html` | 14.068 | 761,1 |
| `ui/grid/phx-grid.css` | 168 | 12,3 |
| `ui/grid/phx-grid.js` | 1.837 | 88,5 |
| `ui/diagrama-er.js` | 712 | 29,1 |
| `ui/telemetria.css` | 447 | 19,8 |
| `ui/telemetria.js` | 1.799 | 88,0 |
| `ui/multitela.css` | 156 | 8,6 |
| `ui/multitela.js` | 1.549 | 66,5 |
| `ui/claude.js` | 1.346 | 66,6 |
| `ui/grid/CHANGELOG-phx-grid.md` | 205 | 28,7 |
| **total (10 arquivos)** | **22.287** | **1.169,3** |

Em `ui/` mas **fora** desse `include_str!`/`include_bytes!` (não embutidos no
binário — servidos de outro jeito ou remanescentes):
`explorador.css`, `explorador.html`, `explorador.js`,
`grid/LEIAME-phx-grid.md`.

Esta é exatamente a lista que o `CLAUDE.md` do projeto manda tratar assim: a
receita do KiB de interface do rodapé do dossiê "era uma lista de três
arquivos copiada no script"; hoje ela sai do próprio `http.rs`, e o número
que ela produz — **1.169,3 KiB**, 10 arquivos — é maior que os 1.032 KiB
registrados na última correção do dossiê (feita com 9 arquivos na lista; o
`CHANGELOG` da grade é o décimo, presente hoje em `http.rs`). Não importa
exatamente quando cada arquivo entrou: o que importa é que este número saiu
de ler `http.rs` agora, não de repetir um número de uma rodada anterior — se
este documento tivesse copiado o 1.032 antigo, estaria errado pelo mesmo
motivo que o rodapé já errou uma vez.

### 1.3 Outras linguagens

<!-- GERADO: bloco_outras_linguagens() -->
| o que | onde | arquivos | linhas |
|---|---|---:|---:|
| JavaScript (prova ponta a ponta) | `testes-web/` | 26 | 4.545 |
| Python (bancada de medição) | `bancada/` | 44 | 17.866 |
| Shell (empacotar, zelador, provas) | todo o repositório | 9 | 1.365 |
| Markdown (documentação técnica) | `docs/` (raso; não recursivo em `dossie/`, `design/`, `video/`) | 53 | 28.744 |
| Python (geradores de dossiê/pedidos) | `docs/dossie/` | 6 | 2.010 |

Não incluído acima porque já está na tabela 1.1: os `.rs` de `examples/` e
`tests/` (bancada e prova em Rust). Não medido: HTML/CSS/JS fora de
`crates/phxsql-server/ui/` (não há outro em produção) e o volume do próprio
`marca/` (imagens, não código).

---

## 2. Dependências, e o que a escolha comprou

A regra do projeto é **zero dependências externas** — só a `std`. Provado
pelo arquivo, não pela lembrança:

<!-- GERADO: bloco_dependencias() -->
`Cargo.lock` lista **8** pacotes: `phxsql-cli`, `phxsql-cmd`, `phxsql-core`,
`phxsql-ffi`, `phxsql-odbc`, `phxsql-server`, `phxsql-sql`, `phxsql-store` —
os 8 crates deste próprio workspace, um por um. Nenhuma linha `source = ` no
arquivo inteiro: todo pacote é `path`, nenhum vem de um registro (crates.io)
nem de um `git`. Pacotes externos ao workspace: **0**.

Conferido também pelo `[dependencies]` de cada `Cargo.toml`, crate por crate:

| crate | dependências |
|---|---|
| `phxsql-cli` | `phxsql-core`, `phxsql-store` |
| `phxsql-cmd` | `phxsql-core`, `phxsql-server` |
| `phxsql-core` | (nenhuma) |
| `phxsql-ffi` | `phxsql-core`, `phxsql-store` |
| `phxsql-odbc` | `phxsql-core` |
| `phxsql-server` | `phxsql-core`, `phxsql-store`, `phxsql-sql` |
| `phxsql-sql` | `phxsql-core` |
| `phxsql-store` | `phxsql-core` |

`phxsql-core` é a base e não depende de nada — nem de outro crate deste
workspace. Todo o grafo de dependência é uma árvore de quatro níveis dentro
da própria casa.

### O que essa escolha pagou, medido

<!-- GERADO: bloco_empacotamento_zero_deps() (docs/EMPACOTAMENTO.md §5) -->
> O pedido 9 diz *«tudo em Rust, sem dependência»*. Continua verdade, e o
> número sai de comando, não do teclado:
>
> ```bash
> cargo metadata --offline --format-version 1   # pacotes no grafo == os
>                                                # crates deste repositório,
>                                                # 0 com "source"
> ```
>
> O `Cargo.lock` inteiro cabe em poucas dezenas de linhas e não cita registro
> nem git. A prova que vale é a do diretório limpo: com `CARGO_HOME` vazio
> (zero entradas), `CARGO_NET_OFFLINE=true` e as variáveis de proxy apagadas —
> **28,6 s, 30,3 s e 34,3 s** em três medições, para os crates do workspace e
> os quatro binários finais (`phxsqld`, `phxsql`, `phxsqlcmd`,
> `libphxsql_odbc.so`). Nada foi baixado porque não há nada para baixar.

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
| arquivo | o que implementa | norma citada no próprio código | teste(s) que conferem |
|---|---|---|---|
| `sha1.rs` | SHA-1, só para falar o protocolo do MySQL(R). | FIPS 180-4 | `vetores_do_fips_180_4` |
| `sha512.rs` | SHA-512, conferido contra o FIPS 180-4. | FIPS 180-4, RFC 8032 | `vetores_oficiais` |
| `hash.rs` | SHA-256, HMAC-SHA256 e PBKDF2-HMAC-SHA256, sem dependências externas. | FIPS 180-4, RFC 2104, RFC 2898, RFC 4231 | `sha256_vetores_oficiais`, `hmac_vetores_rfc4231`, `pbkdf2_vetores_conhecidos` |
| `ed25519.rs` | Ed25519: assinatura com chave pública e privada, conferida contra a RFC 8032. | RFC 8032 | `vetores_da_rfc_8032`, `o_vetor_de_1023_bytes` |
| `x25519.rs` | X25519: a troca de chaves da RFC 7748, sem dependências externas. | RFC 7748, RFC 8032 | `vetor_1_da_secao_5_2`, `vetor_2_da_secao_5_2` |
| `hkdf.rs` | HKDF-SHA256, a derivação de chave da RFC 5869, sobre o HMAC que já existe. | RFC 5869 | `caso_1_do_anexo_a`, `caso_2_do_anexo_a`, `caso_3_do_anexo_a` |
| `cifra.rs` | ChaCha20-Poly1305 (RFC 8439), sem dependências externas. | RFC 8439, draft-irtf-cfrg-xchacha-03 | `bloco_do_chacha20_bate_com_o_rfc`, `cifragem_do_chacha20_bate_com_o_rfc`, `poly1305_bate_com_o_rfc`, `chave_de_uma_vez_so_bate_com_o_rfc`, `aead_bate_com_o_rfc` |
| `base64.rs` | Base64 (RFC 4648), sem dependências externas. | RFC 4648 | `vetores_rfc4648` |
| `uuid.rs` | Identificadores: UUID de 128 bits (v4 e v7) e identificador de 256 bits. | FIPS 180-4, RFC 9562 | `v7_tem_o_layout_do_rfc_9562` |
| `crc.rs` | CRC-32 (IEEE 802.3, refletido, polinômio 0xEDB88320). | (nenhuma citada — CRC-32 não tem RFC próprio) | `vetores_conhecidos` |
| `json.rs` | Leitor e escritor de JSON, sem dependências externas. | (nenhuma citada) | (nenhum com esse padrão de nome — ver nota) |
| `zip.rs` | Arquivo ZIP: escrita e leitura, com o DEFLATE escrito aqui. | RFC 1951 | (nenhum com esse padrão de nome — ver nota) |
| `pg/scram.rs` | SCRAM-SHA-256 (RFC 5802 + RFC 7677), do lado do CLIENTE — para falar com PostgreSQL(R) 10+. | RFC 5802, RFC 7677 | `troca_do_rfc_7677` |

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

---

## 4. As ferramentas do trabalho

### 4.1 Como se orquestrou

A cláusula pétrea dos dez papéis (`~/.claude/CLAUDE.md`, ecoada em
`CLAUDE.md` do projeto) governa quem convoca quem. A escolha de **escalão**
de modelo por frente — nunca o nome do modelo, só "projeto e risco" contra
"mecânico e verificável" — fica registrada em `docs/MODELOS.md`:

<!-- GERADO: bloco_modelos() -->
`docs/MODELOS.md` registra **2** rodadas até esta escrita: a "Rodada de 1–2
de setembro de 2026", registrada como **NÃO CUMPRIDA** (27 commits, todos no
escalão de projeto e risco, sem nenhum agente convocado depois da retomada da
sessão — o próprio documento diz isso, porque "papel que não está cumprindo
tem de aparecer como não cumprindo"), e a frente "toda tabela é PhxGrid", que
registrou a escolha na hora.

### 4.2 Como se mediu

<!-- GERADO: bloco_bancadas() -->
`bancada/` reúne **24** frentes de medição — `alter`, `arm`, `bateria`,
`carga`, `cifra-do-fio`, `cluster`, `comparacao`, `concorrencia`, `dblink`,
`embutido`, `exclusao`, `guardas`, `jobs`, `mvcc`, `odbc`, `phxsql`,
`profiler`, `replicacao`, `rest`, `rotinas`, `sqlite`, `telemetria`,
`transacoes`, `windows` — das quais **14** documentam a própria metodologia
num `LEIA-ME.md` local. A carga do lado do motor é
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
- **Ponta a ponta, pelo navegador**: `testes-web/` — 26 arquivos `.mjs`,
  4.545 linhas — fala com o servidor de verdade pelo soquete e pela tela, não
  com um duplo em memória. `bateria.mjs` é o comando que roda tudo.
- **Conferidores de estilo/texto, com catraca que só desce**:
  `crates/phxsql-server/src/conferidor.rs` (textos fixos fora da fábrica de
  idiomas, teto atual `TETO_ROTULOS_E_CRASE = 1.720`, que **aposentou** o
  antigo `TETO = 1.549` na letra da regra de QA — régua nova, catraca nova,
  nunca a mesma catraca subindo) e `conferidor_grades.rs` (tabela HTML fora
  do padrão `PhxGrid`, teto atual `TETO_TABELA_NA_MAO = 24`). Ao todo, **10**
  constantes `TETO*` no código do servidor — as duas de cima mais oito em
  `profiler.rs` e `servidor.rs`, algumas delas limite de recurso (tamanho de
  campo, de lote) e não catraca de varredura de texto; a lista completa está
  na tabela gerada.
- **Guardas — prova de que a prova pega**: `bancada/guardas/catalogo.py`
  cataloga **77** defeitos repostos (contado de `len(GUARDAS)`, não por
  regex), cada um com o trecho de código de hoje, o trecho de antes do
  conserto, e os testes que têm de cair quando o defeito volta.
  `python3 bancada/guardas/provar-guardas.py` copia a árvore, repõe cada
  defeito, roda os testes nomeados e julga — prova real nos dois sentidos,
  não só "o teste existe". **Achado desta rodada, no próprio extrator**: a
  versão anterior de `bloco_guardas()` contava `"{" seguido de quebra de
  linha` no texto do arquivo, e isso conta certo só enquanto toda guarda é um
  dicionário raso. As guardas que usam o campo `trocas` — a lista documentada
  no próprio `catalogo.py` para o defeito que mexe em mais de um ponto —
  trazem sub-dicionários `{arquivo, trecho, troca}` que batem no mesmo padrão
  sem ser guarda nova, e o número inflava por isso: dizia **142** aqui, e uma
  nova rodada da mesma regex, sem nenhuma guarda a mais, deu **180** — o
  padrão nunca teve relação estável com a contagem certa. Corrigido para
  importar o módulo e contar `len(GUARDAS)`, do mesmo jeito que
  `bancada/guardas/tabela-no-testes.py` já fazia ao lado.
- **Executáveis de prova dedicados**, em `crates/phxsql-server/examples/`:
  `grades-fora-do-padrao.rs`, `prova-dblink.rs`, `prova-exportar.rs`,
  `textos-fora-da-fabrica.rs`.
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
| Windows x86-64 | **roda: gravou e leu 50 linhas sob `wine`** | um Windows de verdade, para desempenho e para o driver ODBC |
| Linux ARM64 / ARMv7 | **roda: gravou e leu 50 linhas sob emulação** (`qemu-user-static`) | o desempenho real, que só a placa mede |
| Android (Termux) | **compila; link precisa do NDK** | o NDK, e uma corrida real |
| Android (dentro de app) | **a biblioteca existe e roda** (`cdylib`, provada em x86-64 e ARM64) | a camada JNI, e o NDK para o alvo bionic |
| iOS | **a biblioteca existe e roda** (`staticlib` aarch64, exercitada sob emulação) | Mac com Xcode, o alvo `aarch64-apple-ios`, e o invólucro em Swift |

O `phxsqld` como daemon não é o caminho nos dois últimos — não por
limitação nossa, é o que Android e iOS permitem; a forma que cabe num
aparelho é biblioteca embutida (`phxsql-ffi`, `cdylib`/`staticlib`) mais
cliente de sincronia, nunca um mini-servidor escutando porta
(`docs/MOBILE.md`, `docs/EMPACOTAMENTO.md` §7).

### 4.5 Testes, medidos agora

<!-- GERADO: bloco_testes_cargo() -->
Esta é a única tabela deste documento que pode legitimamente sair diferente
a cada rodada de geração, e por um motivo bom: vários papéis mexem na mesma
árvore ao mesmo tempo nesta casa. O extrator roda `cargo test --workspace` —
o mesmo comando que o `CLAUDE.md` do projeto manda rodar antes de
commitar — e soma os `test result:`. Quando a árvore está parada, o número é
a contagem real de testes passando; quando `cargo` pega a árvore no meio de
uma gravação de outra frente, a rodada aborta cedo com poucos binários e
erro de compilação, e o extrator **recusa reportar isso como se fosse o
número final** (ver o texto de aviso que ele mesmo produz nesse caso).

Na medição limpa desta rodada — árvore parada, sem `cargo` concorrente —:

```
cargo test --workspace: 51 binarios de teste, 1.496 testes passaram,
0 falharam, 1 ignorado.
```

(`docs/TESTES.md` §1, mantido por `docs/dossie/numeros-do-projeto.py`,
registrava 1.462 numa medição anterior a esta rodada — a diferença são
testes novos de outras frentes que entraram na árvore compartilhada entre
uma medição e outra, não uma divergência de método: os dois usam a mesma
soma de `test result:`.)

---

## 5. O que foi avaliado e RECUSADO, com o número

Esta é a seção que mais poupa tempo depois — recusa medida impede a mesma
proposta de voltar sem medição nova.

### 5.1 Pedidos recusados, do próprio `PENDENCIAS.md`

<!-- GERADO: bloco_recusados() -->
`docs/PENDENCIAS.md` tem **161** pedidos numerados; **9** trazem a palavra
RECUSADO no próprio texto. Os dois mais relevantes para este documento —
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
> deixaria a inserção 1,006× mais rápida. No backup, o maior bloco contíguo
> de CPU que este motor produz, **63,0% é DEFLATE** — busca de repetição num
> dicionário que depende do byte anterior, o oposto do que uma GPU acelera —
> e o SHA-256, que é o candidato, é **12,1%**: de graça, o backup ganharia
> **1,14×**.
>
> **A agregação morre na conta do barramento, e morre em qualquer
> tamanho:** o `SUM` sobre uma coluna anda a **28.234 MiB/s** nesta CPU,
> **1,79× o pico teórico do PCIe 3.0 x16**. Não há limiar que conserte — a
> CPU já consome os bytes mais depressa do que o barramento os entregaria.
>
> **O que o dono pediu — mais velocidade no processamento pesado — existe, e
> sem CUDA:** dividir pelos 4 núcleos com a `std` que já está aqui dá
> **3,90× no ChaCha20-Poly1305, 3,59× no CRC-32 e 2,51× no SHA-256**,
> medidos; e `ORDER BY` tem **1,51×** parado numa troca de algoritmo de
> ordenação que não depende de placa nenhuma.

Medido na própria máquina desta casa (4 núcleos, sem `/dev/nvidia*`, sem
`nvcc`/`nvidia-smi`/`clinfo`/`rocm-smi`, nenhuma lib CUDA/OpenCL no
`ldconfig`): `cargo run --release --example onde-a-gpu-ajudaria -- 1000000`.

### 5.3 A arquitetura LSM/WAL — cinco já existiam, duas miravam gargalo que não temos, uma quebraria o formato, duas eram reais

<!-- GERADO: bloco_dez_propostas() (docs/DESEMPENHO.md §3) -->
| # | Proposta | Estado no PhxSql | Veredito |
|---:|---|---|---|
| 1 | WAL exclusivamente sequencial | O `.reg` já é *append-only* | **Aponta para o arquivo errado** — não há escrita aleatória no `.reg`, há no `.ndx` |
| 2 | MemTable em RAM | Existe `TabelaMemoria`/`SelectMemory` (87× medido), mas é cache de leitura | **Meia peça, do outro lado** |
| 3 | Single writer + fila MPSC | O servidor já serializa tudo numa trava global única | **Já é assim** — o gargalo é excesso de serialização, não falta |
| 4 | Três modos de durabilidade | `por_operacao`, `por_lote`, `sistema` | **Já existe, e medido**: 1.289 → 18.264 → 24.858 → 26.301 linhas/s (20,4×) |
| 5 | Não atualizar índice secundário na hora | — | **REAL, e é o maior** ganho da lista |
| 6 | UUID v7 ou sequência, nunca v4 | `Uuid` v4/v7 (RFC 9562), `Uuid256`, `Sequence` | **Já existe** |
| 7 | Não alterar o arquivo principal no INSERT | O `.reg` só anexa | **Já é assim** |
| 8 | Segmentos imutáveis, SSTable, compactação | — | **Incompatível** com a ordem de digitação |
| 9 | Buffers grandes em vez de escritas pequenas | 2,06 páginas de `.ndx` por linha, medidas | **Medido, e é pequeno** — o custo é o CRC (4,8 µs), não a chamada `lseek` (0,10 µs) |
| 10 | Pré-alocar o WAL | Volumes crescem conforme escrevem | **Aplicável**, ganho provavelmente pequeno |

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
- **`OPTIMIZE TABLE` (compactação).** Esbarra na regra do projeto: o `.reg`
  nunca reaproveita slot excluído. Compactar reescreveria `rowid`, que é
  endereço — quem guardou um passa a apontar para outra linha. Uma tabela com
  muitas exclusões cresce e não encolhe; é consequência aceita da garantia,
  não esquecimento.
- **`ANALYZE TABLE`.** Não há planejador de consultas: quem escolhe o
  índice é quem escreve a operação. Estatística sem consumidor é arquivo
  para manter atualizado sem ninguém ler.
- **`EXPLAIN`.** Faz sentido depois da camada SQL, não antes.
- **`information_schema` com 79 tabelas.** O catálogo daqui —
  `sistabelas`/`siscolunas` — cobre o que existe; tabela de catálogo para
  recurso que não existe seria promessa em forma de esquema.
- **Transações, `SAVEPOINT`, `XA`, tabelas temporais, replicação de
  verdade.** São recursos com roteiro próprio, não superfície de operação
  recusada — a réplica multi-servidor, por exemplo, **já existe e está
  medida** (§4 acima).

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
próprio caminho do script) e imprime, em ordem, todos os blocos marcados
`<!-- GERADO: … -->` acima. `cargo test --workspace` faz parte da rodada e
pode demorar; se `cargo` estiver ocupado por outra frente nesta árvore
compartilhada, o extrator ainda roda e sinaliza a rodada como suspeita em vez
de reportar um número contaminado — refaça quando a árvore estiver parada.
