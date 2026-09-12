# Proposta: o terceiro tipo de banco — o vetorial (embeddings e similaridade)

Proposta **antes do código** (como `docs/SOMBRA.md`, `docs/MEMORIA.md` e a irmã
`colmeia.md`), na mesma rodada em que o dono pediu a infraestrutura dos três
tipos de database (11/09/2026). Ideia: ao lado do **padrão** (relacional) e da
**colmeia** (hierárquico chave→valor), um terceiro tipo —

3. **Vetorial** — um armazém de **vetores** (os *embeddings* que modelos de IA
   produzem: uma linha de N números de ponto flutuante por item) com **busca
   por similaridade**: dado um vetor de consulta, devolver os K itens mais
   próximos por distância (cosseno, produto interno ou euclidiana). Bom para
   busca semântica, recomendação e RAG — o trabalho que um `pgvector`, um
   `sqlite-vss` ou o tipo `VECTOR` do MariaDB 11.7 fazem.

Isto **não troca** o padrão nem a colmeia: é um terceiro tipo para um terceiro
trabalho. O padrão responde «onde `cpf = ?»; o vetorial responde «quais os 10
mais parecidos com este?» — são perguntas de natureza diferente, e o índice que
serve uma é inútil para a outra.

## 0. O que a convergência dos motores diz aqui — e o que ela NÃO diz

A pétrea «três motores maduros convergindo é aceite automático» **não se
aplica** a vetores, e é importante dizer por quê, para não confundir as leis:

- O tipo `VECTOR` e a busca ANN são **recentes em todos**: `pgvector` é
  extensão (não o core do PostgreSQL), o `VECTOR` do MariaDB é de 11.7 (2024–25),
  o do MySQL é de 9.x, e o SQLite depende de extensão de terceiro. Não há o
  **comportamento maduro e comum** que a lei exige — há quatro desenhos ainda
  divergindo em tipo de índice, métrica padrão e sintaxe. Convergência de
  recém-nascidos não é «verdade absoluta»; é moda que ainda pode recuar.
- Então vale a lei de cima, não a de baixo: **receita de fora se mede contra o
  nosso gargalo antes de virar plano.** É pesquisa (papel J), não aceite.

Onde há consenso de comportamento — a distância é simétrica, o K-NN devolve K
itens ordenados por proximidade, empate desempata de forma estável — isso entra
sem discussão, porque é matemática, não arquitetura.

## 1. A premissa, agora MEDIDA (12/09/2026), e que decide se construímos

A lei manda medir a receita de fora contra o nosso gargalo antes de aceitá-la —
e aqui a premissa é dupla. **As duas foram medidas** (V1+V2 da frente,
`bancada/vetorial/resultados.json`, `--example custo-do-vizinho`, máquina
parada). O resultado, antes das explicações do que cada uma pergunta:

> **Não há «força bruta basta» universal — o corte depende de N×d.** Com o limite
> «interativo» em ≤ 50 ms/consulta, a busca exata escalar basta até 100.000
> vetores × d=384 (42,7 ms) e **deixa de bastar** já a partir de 100.000 × d≥768
> (82,8 ms), chegando a 1,66 s em 1M × 1536. O custo cru de uma distância é 0,809
> µs (d=384) / 1,639 µs (d=768) / 3,400 µs (d=1536). **Veredito de formato:**
> corpus pequeno de dimensão baixa pode nascer sem índice; produção (100k+) em
> dimensões reais de LLM (768/1536) **pede ANN**. Tabela completa em
> `docs/VETORES.md` §5.

As duas perguntas que esse número respondeu:

1. **Exato (força bruta) basta, ou precisamos de índice aproximado (ANN)?**
   Buscar os K mais próximos por força bruta é varrer os N vetores e calcular N
   distâncias — `N × d` multiplicações-soma, **exato**, sem índice e sem
   formato novo. Um índice ANN (HNSW, IVF) troca **recall** por velocidade e
   traz um formato de grafo/partição complexo para manter. A pergunta que
   decide o formato: **para o N e o d que o dono tem, a força bruta já responde
   rápido o suficiente?** Se sim, o tipo vetorial nasce sem índice novo — só
   uma coluna de vetor e um `varrer` que ordena por distância. Isso é palpite
   até um medidor rodar sobre vetores de verdade.
2. **Em Rust e só com a `std`, quanto custa a distância?** Sem BLAS, sem SIMD
   estável, sem crate de álgebra — a pétrea de **zero dependências** manda
   escrever o produto interno e a norma à mão, em laço escalar. É o mesmo
   método do SHA-256 e do PBKDF2 desta casa. O custo disso por consulta, contra
   `N × d`, é o número que falta — e é o que separa «força bruta basta» de
   «precisamos de ANN».

*Medir a premissa do item vem antes de implementar o item, inclusive quando o
item é nosso.* Os dois números agora existem, então a escolha de formato deixou
de ser chute: o `.vec` de força bruta serve o corpus pequeno; o `.hnsw` (ANN) é
o que a produção em 768/1536 exige — e V3+ ainda espera o P0 e o aval do dono do
formato, medição não revoga gate.

## 2. As divergências que as nossas pétreas FORÇAM — e é isto que o torna nosso

Lógica que sai diferente da de origem não é cópia, e a prova é a divergência.
Onde o vetorial do PhxSql diverge do `pgvector`/HNSW de referência, e por qual
restrição nossa:

- **Zero dependência: a distância e o índice são escritos aqui.** O `pgvector`
  usa SIMD e, no Postgres, se apoia no executor dele; o FAISS é C++ com BLAS. Nós
  escrevemos o produto interno, a norma e (se a medição exigir ANN) o próprio
  grafo, em Rust puro. Ler a norma, entender, reescrever, provar — o método da
  casa. O custo escalar é medido, não estimado.
- **Sem reúso de slot — append-only.** A `.reg` nunca reaproveita slot excluído
  (ordem de digitação sagrada). Um HNSW clássico remove nó e **reescreve** o
  grafo no lugar; isso a nossa pétrea proíbe. O vetorial do PhxSql marca o
  vetor apagado e **nunca reusa o slot**; a reconstrução do índice é operação
  explícita (um `VACUUM`/`reindexar`), nunca silenciosa — a mesma divergência
  que a colmeia tem do REGF, pela mesma pétrea.
- **Durabilidade pela nossa máquina.** Nada de log próprio: usa a marca `.tx`
  write-ahead e a disciplina de `fsync` que o padrão já tem, e herda o conserto
  de atomicidade do P0 quando ele entrar.
- **Integridade: o vetor é filho da linha.** Um embedding descreve um item —
  uma linha de negócio. Pela regra primordial, **só existe o vetor se a linha
  existir primeiro**, e apagar a linha não pode deixar o vetor órfão. O
  `Restrict` da casa vale aqui por construção, se o vetor referenciar uma
  tabela padrão.
- **Cifra em repouso e direitos.** Vetor é dado: a cifra em repouso e o portão
  de permissão por tabela que o PhxSql já tem valem igual. Embedding vaza tanto
  quanto o texto que o gerou.

O desenho interno pode ser **inspirado** em `pgvector`/HNSW/IVF — são bons
desenhos para K-NN —, mas cada decisão passa pelo crivo acima. O que não passar,
não entra.

## 3. Onde ele mora

Busca semântica sobre o dado que já está no banco: o cliente guarda documentos
ou produtos no padrão, gera embeddings, e pergunta «os mais parecidos com
isto». O vetorial é o índice dessa pergunta, ao lado do relacional — não no
lugar dele. É também a peça que um RAG local pediria: recuperar por similaridade
sem sair para um serviço de fora (coerente com a pétrea de zero dependência e
com o dado ficando em casa).

## 4. O que isto NÃO é, e a ordem certa

- **Não é para agora.** O mesmo parecer que pediu os três tipos mandou **pausar
  a ampliação até o núcleo estar sólido**, e o núcleo tem um **P0 de
  atomicidade aberto** (o commit pai+filho). Construir um terceiro motor antes
  de consertar o commit do primeiro é o anti-padrão que o parecer alertou.
- **Não é cópia.** Ver §2: as divergências forçadas pelas nossas pétreas são a
  prova de que o desenho passou pela nossa cabeça.
- **Não é decidido.** É proposta. O que a destrava é **medir a premissa** (§1):
  um medidor de força bruta em Rust puro, sobre vetores reais, contra o K-NN que
  o dono precisa — com a disciplina da `bancada/comparacao` (mesmo trabalho,
  faixa min–máx, data). Se a força bruta já responde no tempo que o uso aceita,
  o tipo vetorial nasce **sem índice novo** e sem formato novo — uma vitória
  barata. Se não, aí sim se decide um índice ANN, com o dono aprovando o formato
  antes de gravar.

## 5. Recomendação

1. **Primeiro o P0** (atomicidade do commit) e os P1 de SQL — o núcleo sólido.
2. **Depois, medir a premissa**: força bruta escalar em Rust × o K-NN real, nos
   N e d do dono. A hipótese «força bruta basta» pode **morrer medida** — e a
   recusa com número é resultado tão válido quanto o ganho, e impede a ideia de
   voltar sem medição. O contrário também: se a força bruta passa, poupamos um
   formato de índice inteiro.
3. **Só se a força bruta não bastar**, desenhar o índice ANN contra as
   restrições da §2 (append-only, zero dep, `VACUUM` explícito), com o formato
   aprovado pelo dono antes de qualquer byte em disco.

A infraestrutura dos três tipos já existe (o enum `TipoDatabase`, o marcador
`_database.json`, a recusa honesta «motor em construção»): criar um database
vetorial já é válido e reserva o tipo. O **motor** é esta frente — e ela começa
por um número, não por um formato.
