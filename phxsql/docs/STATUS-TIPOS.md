# Status dos tipos de base do PhxSql

**Avaliação datada — 12/09/2026.** Pedida pelo dono: *«Vc fez mais de 3 tipos… chama
o time todo pra revisar; gere um status como está isso.»* O orquestrador (papel A)
integrou três frentes **só-leitura**: **C (DBA)**, **J (pesquisador)** e **B+G+F**
(engenharia + QA + prova real). Cada linha traz a fonte **no código ou na medição** —
*número citado é número que não se mede.*

**Papéis convocados:** A (integração), C (formato/garantias de cada tipo), J (as
receitas de fora medidas contra o nosso gargalo), B+G+F (a verdade do enum, do portão
e das provas), H (este documento), I (commit). **Dispensados, com motivo:** D (zelador
— revisão só-leitura, nada a apagar); E (designer — sem tela nesta rodada; vira página
se o dono pedir).

## O que o motor declara de fato — três tipos, não cinco

O enum `TipoDatabase` (`crates/phxsql-core/src/tipo_database.rs:23-31`) tem **três**
variantes: `Padrao`, `Hive`, `Vetorial`. Só `Padrao` responde `motor_pronto()==true`
(`tipo_database.rs:62-64`). O marcador `_database.json` grava o tipo por database
(`catalogo.rs:42`); ausência ou tipo desconhecido **cai em Padrao**, nunca para a
abertura. O portão `exigir_motor_padrao` (`catalogo.rs:507-517`) recusa operar tabela
em Hive/Vetorial com «motor em construção» — **provado em 5 testes** (quatro em
`catalogo.rs:1140-1201`, um no protocolo em `servidor.rs:24009`).

Dos cinco itens que o dono nomeou, **três são (ou seriam) tipo de motor** e **dois são
aplicação/modo sobre o Padrão** — não `TipoDatabase`.

## A tabela

| # | o dono chamou de | no código é | classe | estado | veredito |
|---|---|---|---|---|---|
| A | Padrão | `TipoDatabase::Padrao` | tipo | **motor construído e medido** | existe, provado — nada a fazer |
| B1 | Vetor «tipo SAP HANA» (colunar) | — (não é variante) | layout colunar | **recusa medida** | reabre só com gargalo analítico medido |
| B2 | Vetor de embeddings (IA) | `TipoDatabase::Vetorial` | tipo | **reservado, sem motor** (proposta) | medir a premissa antes do formato |
| C | «Dat do regedit» (colmeia/hive) | `TipoDatabase::Hive` | tipo | **reservado, sem motor** (proposta) | premissa medida (REGF **e** protótipo nosso: 10,6×–13,7× sobre o Padrão); falta P0 + aval do formato PSHV |
| D | base p/ servermail/clientmail | — (tabelas Padrão) | aplicação | **modelo provado, formato pendente do dono** | é aplicação, não tipo |
| E | base p/ Blockchain | — (modo sobre o Padrão) | modo/recurso | **esquema já roda; falta calcular o hash e verificar a cadeia** | é modo, não tipo — e é o mais barato dos três |

## Linha a linha

### A — Padrão (relacional) · `TipoDatabase::Padrao`
Row-store, arquivos separados (`.reg`/`.ndx`/`.bin`/`.fts`/`.log`), PSCH v9. É o único
com motor. I/O medido (corrida do milhão, 08/09, Linux): ler 1 ponto **7,94 µs/op**,
gravar **12,8 µs/op**; insert em lote a 1 M de linhas em **7,87 s**. Fonte:
`bancada/comparacao/um-milhao.json`, `docs/DESEMPENHO.md`.

### B1 — «HANA» = column store in-memory · **recusa medida**
HANA de verdade é colunar/vetorizado (SIMD), OLAP. **Não** é o tipo VECTOR. Nossa
recusa está medida em três documentos (`docs/STATUS.md` linha D nota 2, `docs/MEMORIA.md`
§4, `docs/GPU.md` §7, `docs/VETORES.md` §0):

- Achatar `Vec<Value>` (enum com `String` no monte) → colunar plano custa **uma passada
  inteira pela memória — mais que a varredura que aceleraria**.
- E a conta não é o gargalo: o `SUM` já anda a **28.234 MiB/s = 1,79× o pico do PCIe 3.0**,
  acima do teto de leitura da RAM (24.047 MiB/s). O gargalo é **banda de memória**, não
  agregação — vetorizar acelera a conta, e aqui não é a conta que dói.
- Que o ganho seria de *layout* e não de SIMD já está medido de lado: ordenar 1 M de
  linhas `Value` = 213,8 ms; as mesmas chaves como `u64` plano = 19,9 ms (**10,7×**).

O que passou no crivo **já está construído**: `TabelaMemoria`/`SelectMemory` (linha
decodificada em RAM, **87×**) e o cache de páginas do `.ndx` (**2,40×**). A única
alavanca que sobra (`MEMORIA.md` §6) é um *zone map* min–max leve **dentro** do
`SelectMemory` para `<`/`>`/`BETWEEN` — **não** um segundo motor em disco — e espera um
número (ver premissas).

### B2 — Vetor de embeddings (pgvector/sqlite-vss/MariaDB 11.7) · `TipoDatabase::Vetorial`, proposta
Guarda um vetor de N floats por linha e responde K-NN por similaridade, com índice
aproximado ANN (HNSW/IVF). Tipo **reservado, sem motor** (`docs/propostas/vetorial.md`).
Não vale a pétrea «três motores convergindo é aceite automático»: o tipo VECTOR é
**recém-nascido em todos** (pgvector extensão, MariaDB 11.7/2024, MySQL 9.x), ainda
divergem em índice, métrica e sintaxe — então vale «medir antes». Onde bate nas pétreas:
o `.vec` (embedding, família do `Memo`) é fácil e transacional; o **`.hnsw` é o osso** —
HNSW é grafo **mutável** e o clássico apaga reescrevendo no lugar, o que a **ordem de
digitação sagrada proíbe**: o nosso divergiria por lápide + `VACUUM` explícito, e o
ROLLBACK de arestas é bem mais caro que desfazer uma linha do `.reg`.

### C — Colmeia/hive («Dat do regedit») · `TipoDatabase::Hive`, proposta
Armazém hierárquico chave→valor mapeado em memória, inspirado no REGF do Windows. Tipo
**reservado, sem motor** (`docs/propostas/colmeia.md` + `colmeia-estrutura.md`). A
premissa do REGF está **medida** (`bancada/registro/resultados.json`, Windows WXSOLUCOES,
11/09), contra o Padrão (Linux, 08/09):

| por operação | Registro (medido) | PhxSql padrão |
|---|---|---|
| ler 1 ponto | **~2–5 µs/op** | 7,94 µs/op |
| gravar 1 ponto | **~255–495 µs/op** | 12,8 µs/op |

A colmeia **ganha na leitura (2–4×) e perde feio na escrita (20–40×)** — não condena a
ideia, **condena usá-la para a coisa errada**: serve para config/metadados (lê-se
milhões, escreve-se um punhado), não para dado transacional. Divergências que as pétreas
forçam (é o que a torna nossa): sem reuso de célula (append-only + `VACUUM` explícito → o
«undelete» acidental do Registro vira propriedade *projetada*); durabilidade pela marca
`.tx` da casa, não `.LOG1/.LOG2`; UTF-8 e zero-dep no lugar de UTF-16LE. Ressalva honesta
que o próprio `resultados.json` carrega: o bench grava N valores sob **uma** chave (infla
a escrita), e os bytes-ao-disco do REGF **ainda não foram medidos**.

### D — servermail / clientmail · **aplicação, não tipo**
Não há variante de `TipoDatabase`. O correio é **dado do PhxSql guardado no PhxSql**:
seis tabelas PSCH comuns (`docs/CORREIO-FORMATO.md`), sobre o **motor Padrão**, herdando
CRC, espelho, diário, replicação e as pétreas de integridade sem uma linha de motor de
arquivo novo. Estado: **modelo provado em memória** (protótipo `correio-e2e.rs`, 24
checagens verdes; bancada `bancada/servermail/resultados.json` toda verde em 11/09), mas
o **formato ainda é proposta pendente do aval do dono** («nada foi gravado»). O SMTP de
**envio** já existe (`crates/phxsql-server/src/email.rs`, é o alerta de disco), **sem
TLS**, relé interno — não sabe **receber**. Botão «Server Mail» está **desligado** no
menu Ferramentas (`ui/index.html`, `disabled`, opacidade .42), com o texto do que falta.

### E — Blockchain · **modo/recurso sobre o Padrão, não tipo — e o mais adiantado**
Não existe motor; hoje é só rótulo de menu **apagado** (`idiomas.rs:1641`,
`ui/index.html:~13906`, marcado «não existe» no `MANUAL.txt` e no teste da barra). Mas o
substrato de um *ledger* encadeado privado **já está quase todo aqui**, verificado no
fonte:

- **Append-only real, nunca reusa slot** — pétrea «a ordem de digitação é sagrada»
  (`docs/DESEMPENHO.md` #7). É a imutabilidade que uma cadeia pede.
- **SHA-256/CRC-32 escritos à mão, contra vetor** (`crates/phxsql-core/src/cifra.rs`,
  `crc.rs`, FIPS 180-4); o tipo **`Uuid256` (32 bytes) existe porque «um SHA-256 cabe
  exato»** (`docs/FORMATO.md` §13).
- **O esquema do bloco já roda** — `crates/phxsql-store/examples/identificadores.rs`
  monta a tabela `blocos` com `hash` (Uuid256), **`anterior` (Uuid256 = hash do bloco
  anterior — o encadeamento)**, `altura` (Sequence), índices únicos.
- **Diário `.log` append-only com integridade por evento** (`crates/phxsql-store/src/log.rs`):
  CRC-32 por evento e, com cifra ligada, ChaCha20-Poly1305 com o cabeçalho como dado
  associado — mover o corpo derruba a autenticação.

**Não é motor novo, é modo sobre o Padrão** — nenhuma mudança de formato: `Uuid256` +
`Sequence` + append-only bastam. Falta só (a) **calcular** o SHA-256 sobre o conteúdo do
bloco (hoje o exemplo grava `Uuid256::aleatorio()` — o esquema encadeia, mas a prova de
conteúdo ainda não é calculada) e (b) a **verificação de cadeia** (percorrer por altura
conferindo `hash(bloco_n) == anterior_{n+1}`) + raiz de Merkle se o uso pedir.
Blockchain **pública** (consenso/PoW/rede P2P) fica **fora do escopo** — esbarra em
superfície de rede que o projeto não tem; recusar até haver pedido medido.

## A premissa que falta medir antes de construir cada um

- **B1 (colunar/HANA):** quanto do vão de banda (~7×: scan a 3.468 MiB/s vs teto 24.047)
  o *layout* plano recupera **no caminho do FILTRO** — na máquina parada. Se recuperar
  pouco, morre. (`MEMORIA.md` §6, premissa 1.)
- **B2 (VECTOR/embeddings):** para o **N e o d reais do dono**, a força-bruta escalar em
  Rust puro (produto interno à mão, zero-dep) já responde rápido o bastante — ou precisa
  de ANN? Se bastar, nasce sem índice e sem formato novo. (`propostas/vetorial.md` §1.)
- **C (colmeia): MEDIDA (12/09/2026)** — o protótipo PSHV em Rust + nosso cache lê
  config-shaped **10,6×–13,7× mais rápido** que o Padrão (busca de ponto × busca de ponto,
  máquina parada, faixas que não cruzam; `bancada/colmeia/resultados.json`). A premissa
  passou; falta ainda o bytes-ao-disco do próprio REGF (H3, máquina Windows do dono).
  (`colmeia.md` §1.)
- **E (blockchain):** qual o custo do SHA-256 **por bloco calculado sobre o conteúdo** e
  da verificação de cadeia por altura — dado que SHA-256 é serial e a casa mede **2,51×
  em 4 núcleos** por arquivo, não por bloco.

## Leitura de uma linha

Só **um** tipo tem motor (A/Padrão). **B2** e **C** são tipos reservados cujo motor é
proposta — e a regra manda **medir a premissa antes de implementar**. **B1** (colunar
HANA) é **recusa medida**, não buraco. **D** e **E** **não são tipos**: D é aplicação
sobre tabelas Padrão (formato à espera do aval do dono) e E é um modo sobre o Padrão cujo
esquema já roda — o mais barato dos três abertos, faltando calcular o hash e verificar a
cadeia. Não há, portanto, «cinco motores» a medir de I/O: há **um motor medido**, dois
motores propostos com premissa a medir, e duas aplicações/modos sobre o Padrão.
