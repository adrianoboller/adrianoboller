# Backlog dos tipos/modos de base — kanban do time

**12/09/2026.** Scrum Master: papel A. Backlog numerado e sequencial das quatro
frentes que o dono pediu em ultra-urgência (Vetorial, Colmeia/Hive, Blockchain,
Correio), integrado da Fase 1 (PDCA de 5 agentes: J-Vetorial, J-Colmeia,
J-Blockchain, H-Correio, H-Kanban). Os detalhes técnicos de cada item saem dos
PDCA; aqui fica a **sequência, os donos e os portões**.

**Papéis convocados:** A (orquestração/Scrum Master), J (premissas), C (DBA —
formato), B (código, front/back), F (prova real), G (guarda/catraca), H (docs),
I (commit/backup). **Dispensados com motivo:** E (designer) — só entra quando um
item tocar tela; D (zelador) — roda fora da janela de build.

## Kanban — colunas e regra de saída

`Backlog` → `A medir premissa` → `Em desenho` → `Bloqueado (aval do dono)` →
`Em build` → `Em prova` → `Integrado`; hipótese que morre medida vai a
`Fechado/recusado` **com o número**, nunca some do quadro.

## Definição de pronto (todo item)

- Prova real **nos dois sentidos**: falha com o defeito reposto, passa com o conserto.
- O que depende do SO provado **contra o SO** (soquete/processo), não só unit.
- `cargo fmt --all` sem diff; `cargo clippy --workspace --all-targets` **zero avisos**; `cargo test --workspace` verde.
- Guarda nova (se houver) no catálogo de QA, com o defeito que a motivou; catraca só desce.
- Doc de área no mesmo commit (`FORMATO.md` se mudou formato; número **medido**, nunca de memória).
- **Só o integrador comita**, `git add` por caminho explícito — nunca `-A`/`-u`/`.`, nunca reset/amend/force sobre história empurrada (cognição 12/09).

## Os três portões

1. **Pipeline (disco 6 GiB, medido):** um só `cargo build`/`test` por vez entre as frentes. Pesquisa/desenho/doc/prova-de-navegador rodam em paralelo; o que **compila** entra em fila. Zelador fora da janela de build.
2. **Aval de formato do dono:** todo item que grava **formato novo em disco** para em `Bloqueado` até o «ok» datado. Atinge Colmeia (PSHV + `.tx` v2), Vetorial (PSCH v10 + `.vec`, e `.hnsw` à parte), Correio (5 decisões). **Blockchain não entra** — não muda formato.
3. **P0 — atomicidade do commit pai+filho (aberto):** `Table::conferir_fks` (`table.rs:1205`) lê a mãe do disco e não enxerga o pai empilhado (`ACID.md §0`, `PENDENCIAS #189`). Os dois pesquisadores cravaram: **erguer motor novo (Vetorial/Colmeia) antes de fechar isto é anti-padrão**. **Blockchain é exceção** — o ledger não depende de transação (append-only + hash + Uuid256 já existem).

## Ordem de ataque entre frentes

1. **E / Blockchain** — sem aval de formato, sem dependência do P0, esquema já roda: **ocupa o primeiro slot de build agora**.
2. **P0** (atomicidade pai+filho) — decisão do dono: é o gate de Vetorial e Colmeia; recomendação do time é fechá-lo antes desses dois motores.
3. Em **paralelo** ao build de E (não competem, não compilam): medir a premissa de **V** (força-bruta escalar basta?) e de **H** (protótipo colmeia × Padrão), e refinar o formato de **D** — tudo esperando os avais.
4. Ao liberar o slot: itens **sem gate de formato** com premissa confirmada (candidato: `.vec` força-bruta da Vetorial, se dispensar ANN).
5. Colmeia, `.hnsw` e Correio entram em build **só após** os avais + P0.

---

## Frente E — Blockchain (ledger encadeado privado, MODO sobre o Padrão)

> **ESTADO 12/09/2026: E1–E3 INTEGRADOS** — commit `540e5cd`, módulo
> `crates/phxsql-store/src/ledger.rs` (`hash_do_bloco`/`preparar_bloco`/
> `verificar_cadeia`), 8 testes verdes com prova real dos três casos de
> adulteração + buraco de altura + hash reproduzível fora do motor;
> `fmt`/`clippy`/`test` verdes; formato em disco **intocado**. **E4** (Merkle) e
> **E5** (Ed25519) ficam **sob demanda**; **E6** (medidor `custo-do-bloco` +
> texto da UI) é do orquestrador, pendente. Frente E: **entregue no núcleo.**

Sem aval de formato. Não gated pelo P0. Fontes: `hash.rs:165` (SHA-256),
`ed25519.rs:573`, `identificadores.rs:28-72` (tabela `blocos`), `log.rs:1046`
(modelo de prova), `FORMATO.md §13` (Uuid256).

- **E1** — calcular o SHA-256 sobre o **conteúdo canônico** do bloco (reserializa, não recorta) e gravar em `hash`; tirar `Uuid256::aleatorio()` do caminho (`identificadores.rs:63`). Dono B. *Pronto:* trocar 1 byte muda o hash; hash reproduzível fora do motor.
- **E2** — encadear pelo hash real: `anterior_{n+1}=hash_n`, gênese `NULO`. Dono B. *Pronto:* toda altura satisfaz a ligação.
- **E3** — verificação de cadeia por `porAltura` (confere conteúdo, ligação e altura contígua) — **o entregável**. Dono B+F. *Pronto:* prova real dos dois sentidos, medindo **onde** pegou (três casos: conteúdo, ligação, assinatura).
- **E6** — medidor `--example custo-do-bloco` (SHA-256 por bloco; **sem** `-C target-cpu=native`, que mede 0,66× pior) fechando a premissa E do `STATUS-TIPOS.md`, e reescrever o `falta:` velho da UI (`index.html:13906`). Dono J+H.
- **E4** — raiz de Merkle das transações do bloco. **Só sob demanda.** Dono B.
- **E5** — assinatura Ed25519 por bloco. **Só sob demanda.** Dono B.

## Frente V — Vetorial (embeddings, `TipoDatabase::Vetorial`)

> **ESTADO 12/09/2026: V1+V2 INTEGRADOS** — commit desta rodada. Núcleo
> `crates/phxsql-core/src/vetor.rs` (8 testes de valor de referência, prova real
> dos dois sentidos nos bugs semeados «esqueceu o sqrt» e «esqueceu de dividir
> pela norma»); bancada `custo-do-vizinho` →
> `bancada/vetorial/resultados.json` (sweep completo, máquina parada).
> `fmt`/`clippy`/`test --workspace` verdes, conferidos na integração. Formato em
> disco **intocado** (dispensa de C registrada: função pura + bancada em
> memória). **Veredito da premissa: não há «força-bruta basta» universal — o
> corte cai em N=100k para d≥768** (82,8 ms > 50 ms), força-bruta só serve
> corpus pequeno de dimensão baixa; produção em 768/1536 **pede ANN**. Docs
> `VETORES.md §5` e `propostas/vetorial.md §1` atualizados;
> `cognicao_vetorial-forca-bruta-corta-em-100k-nao-em-milhoes_20260912_1525.md`.
> **V3+ segue BLOQUEADO** — P0 + aval do dono do formato; medição não revoga gate.

Gate P0 + aval de formato (PSCH v10). Fontes: `vetorial.md`, `VETORES.md`,
`types.rs` (tag livre 22), `FORMATO.md §1/§3`.

- **V0** — *gate:* P0 fechado. Nada abaixo começa antes.
- **V1** — ✅ núcleo de distância zero-dep (cosseno/PI/euclidiana), provado contra vetor de referência. Dono B. Sem formato. **Integrado.**
- **V2** — ✅ bancada `custo-do-vizinho`: sweep N=1e4/1e5/1e6 × d=384/768/1536, máquina parada, mediana+faixa. **Premissa medida:** força-bruta basta só até ~100k×384; produção 768/1536 **pede ANN**. Dono J. Sem formato. **Integrado.**
- **V3** — `ColumnType::Vetor{elem,dim}` (tag 22) + arquivo `.vec` + força-bruta exata no motor. **[AVAL: PSCH v10 + `.vec`]** Dono C+B.
- **V4** — integridade (vetor é filho da linha, Restrict), transação (`.tx`), cifra em repouso, portão por tabela. Dono B+F.
- **V5** — índice ANN (HNSW/IVF) zero-dep, append-only (lápide + VACUUM). **Só se V2 pedir. [AVAL: `.hnsw`/`.ivf`]** Dono C+B+F.

## Frente H — Colmeia/Hive (`TipoDatabase::Hive`, formato PSHV)

Gate P0 + aval de formato (PSHV + `.tx` v2). Fontes: `colmeia.md`,
`colmeia-estrutura.md`, `catalogo.rs:507`, `ndx.rs` (CachePaginas), `transacao.rs`.

- **H1** — protótipo PSHV mínimo (ler/gravar 1 ponto), só `std`, com read-back conferido. Dono B/J. Sem formato (protótipo de bancada).
- **H2** — medir a premissa × Padrão, **mesma máquina**, config-shaped (poucos valores por chave). **Gate do projeto:** «lê N× mais rápido que o Padrão» ou morre medida. Dono J.
- **H3** — bytes-ao-disco do REGF (Process Monitor/ETW) — fecha o «durável ≤ preguiçosa». Dono J. (Depende da máquina Windows do dono.)
- **H4** — **[AVAL]** aprovar o formato PSHV (bloco base, bins, células, `montagem.json`, extensão `.hive`). Dono C+dono.
- **H5** — **[AVAL]** marca `.tx` v2 aprende o append de célula hive; recuperação de marca órfã provada. Dono C+B.
- **H6** — portão `exigir_motor_hive` + roteamento por `TipoDatabase::Hive`; `motor_pronto()` passa a incluir Hive. Dono B.
- **H7** — as seis operações (`ler`/`gravar`/`listar`/`apagar`/`compactar`/`montar`); `apagar` recusa Restrict; `compactar` prova append-only. Dono B+F.
- **H8** — camada de montagem (`montagem.json`); montar/desmontar não reescreve colmeia. Dono B.
- **H9** — célula SEGURANÇA (direitos + cifra em repouso); `.hive` vazado não revela texto puro. Dono B+F.

## Frente D — Correio (aplicação sobre o Padrão)

Serviço não gated por formato; tabelas gated por 5 decisões do dono. Fontes:
`CORREIO-FORMATO.md`, `email.rs`, `PHXMAIL-vs-SMTP.md`.

**Decisões de formato do dono (travam D6):**
- **D1** — chave Masson (segredo do 2º usuário / chave da empresa / algoritmo?).
- **D2** — recuperação de senha existe? (se sim, escrow em `correio_contas`, enfraquece E2E).
- **D3** — forward secrecy (ratchet) na v1, ou estático aceito?
- **D4** — revogação de confiança (4º estado `revogada`, sem reescrever o passado?).
- **D5** — **[AVAL]** PSCH v10 `rowts` (µs, monotônico `max(agora,último+1)`, valor de origem na réplica) — pré-requisito de gravar qualquer tabela do correio.

- **D6** — **[AVAL: D1–D5]** gravar as seis tabelas PSCH do correio; F prova pai-antes-do-filho, FK restringir, motivo obrigatório. Dono C+B+F.
- **D7** — SMTP de **receber** (`TcpListener`, protocolo servidor completo), zero-dep, sem TLS (relé interno). **Não gated por formato.** Dono B.
- **D8** — prova real por soquete do D7 (dot-stuffing, CRLF, injeção de cabeçalho, recusa/log de `STARTTLS`). Dono F.
- **D9** — medir o caso de exposição real (relé interno basta, ou precisa de TLS → pendência do dono, zero-dep é pétrea). Dono J.
- **D10** — ligar o botão «Server Mail» no menu **só** após D6+D7. Dono E.

---

## Decisões que dependem do dono (o Scrum Master não decide sozinho)

1. **P0 primeiro?** O time recomenda fechar a atomicidade do commit pai+filho antes de erguer Vetorial e Colmeia. Blockchain segue sem esperar.
2. **Formato Colmeia (H4/H5):** PSHV + `montagem.json` + `.tx` v2.
3. **Formato Vetorial (V3):** PSCH v10 + tag `Vetor` + `.vec`; `.hnsw` só se a medição pedir.
4. **Correio (D1–D5):** as cinco perguntas acima, antes de gravar qualquer byte.

Enquanto o dono não decide 2–4, essas frentes acumulam pesquisa/desenho/medição
(colunas à esquerda do quadro) e **não gravam formato**. E/Blockchain executa já.
