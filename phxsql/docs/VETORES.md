# Vetores no PhxSql — o fluxo do tipo padrao e do tipo vetor

> **Status: PROPOSTA, nao construida.** Este documento e o desenho, escrito
> ANTES do codigo, no molde do `docs/CIFRA-DO-FIO.md`. O que decide o resto e
> um numero que ainda nao foi medido (ver §5). Nada aqui esta implementado.

O fluxo visual esta no artefato
<https://claude.ai/code/artifact/51e39134-fb0d-465e-8521-3bd06c9327eb>.

## 0. A lei que rege esta proposta

A ideia veio de fora (uma descricao do SAP HANA). Receita de fora **se mede
contra o nosso gargalo antes de virar plano**, e o que nao sobrevive as nossas
restricoes fica de fora — medido, nao por fe. O que passa no crivo e **so o
vetor de IA, e so como ADICAO**. O column store como segundo motor de
armazenamento esta **recusado por ora**: seriamos outro banco, e somos
row-store, decidido e provado. SIMD e tecnica, nao redesenho, e so entra contra
um gargalo analitico medido.

E a regra que o dono cobrou explicitamente: **nada do que ja foi construido se
perde.** Este desenho e aditivo por construcao — nao troca uma linha do `.reg`,
da ordem de digitacao, da integridade, da replicacao, da cifra, das transacoes
nem dos 230 pedidos.

## 1. Criar: um DDL, dois destinos, o mesmo motor

Um `CREATE TABLE` so. A unica bifurcacao e uma pergunta na MESMA rotina de
criacao: «alguma coluna e `VECTOR`?».

- **Nao** -> os arquivos de sempre: `.reg` `.ndx` `.memo` `.pag`. Byte a byte o
  PhxSql de hoje. Quem nao usa vetor nunca cria mais nada.
- **Sim** -> os mesmos arquivos MAIS dois: `.vec` (o embedding completo,
  persistente) e — so se houver `VECTOR INDEX` — `.hnsw` (o grafo de proximidade).

```sql
CREATE TABLE produtos (
    id         BIGINT PRIMARY KEY,
    codigo     VARCHAR(30),
    descricao  TEXT,
    preco      DECIMAL(12,2),
    estoque    INTEGER,
    embedding  VECTOR<F32, 1536>        -- opcional: some a linha e e a tabela de sempre
);

CREATE INDEX        idx_preco     ON produtos(preco);         -- .ndx, como sempre
CREATE VECTOR INDEX idx_embedding ON produtos(embedding)      -- .hnsw, novo e opcional
    USING HNSW METRIC COSINE;
```

O `VECTOR<F32,1536>` sao ~6 KB por linha. Inline no `.reg` incharia todo slot;
por isso ele e **externo** (a familia do `Memo`/`Bin`), e o `.reg` guarda so um
ponteiro — **zero mudanca na aritmetica do endereco**. `F16` (o `HALF_VECTOR`
do HANA) entra como compressao, decisao medida de memoria x precisao.

## 2. Gravar: uma transacao, varios arquivos — E ISSO JA EXISTE

O COMMIT do PhxSql (frente 37) ja fecha varios arquivos de uma vez, com a marca
`.tx`, e a atomicidade esta provada com `SIGKILL` no meio: a regra e **NUNCA
METADE**, com o relatorio dizendo qual dos dois lados. O `.vec` e o `.hnsw` so
entram nessa mesma carruagem — nao e maquina nova, e heranca.

O embedding e gerado **fora** do banco (o cliente, ou a integracao com a Claude,
produz o vetor antes do COMMIT). O PhxSql nao «pensa», ele grava. A parte
atomica nossa e **linha + vetor + grafo, juntos**. Gravacao sem vetor nem toca
no `.vec`.

## 3. Consultar: o filtro de sempre, e o sentido por cima

```sql
SELECT id, descricao, preco,
       VECTOR_COSINE_SIMILARITY(embedding, :consulta) AS relevancia
FROM produtos
WHERE estoque > 0 AND preco <= 3000
ORDER BY relevancia DESC LIMIT 10;
```

1. O indice tradicional (`.ndx`) **corta primeiro** por estoque/preco — barato.
2. O vetor ordena os sobreviventes: exato (`.vec`) ou aproximado (`.hnsw`->`.vec`).
3. Ordena por similaridade, LIMIT.

Uma consulta comum para no passo 1 — e o PhxSql de hoje. O planejador e o
mesmo; o vetor entra como mais uma coluna e mais um indice.

## 4. O osso, marcado sem enfeite (papel C/F)

O `.vec` e facil — coluna externa como o `Memo`, entra e sai da transacao sem
drama. O `.hnsw` e o problema de verdade, e bate em duas petreas:

1. **HNSW e grafo MUTAVEL.** Inserir reescreve as listas de vizinhos de OUTROS
   nos. E apagar em HNSW e caro — a maioria das implementacoes nao remove,
   marca lapide e reconstroi. Isso encaixa na nossa «ordem de digitacao nunca
   reaproveita slot»: o `.hnsw` trabalha por lapide, como o soft-delete ja faz.
   Encaixa, mas e design, nao sorte.
2. **ROLLBACK num grafo e caro.** Desfazer as arestas que uma insercao mexeu,
   atomicamente, e bem mais dificil que desfazer uma linha do `.reg`. E por isso
   que a busca EXATA primeiro nao e timidez: ela nao tem indice para manter
   consistente — so varre o `.vec`, transacional como qualquer coluna.

E o custo escondido: toda edicao de texto vira um **re-embedding** (externo,
lento, pago se for API). Vale deixar isso explicito na tela.

## 5. O que decide tudo: o numero, agora MEDIDO (12/09/2026)

Foi medido. `bancada/vetorial/resultados.json`, `--example custo-do-vizinho`,
maquina parada (`esta-medindo.sh` confirmou: carga 1min 0.79, 4 nucleos, 14 GiB
livres, nenhum outro processo). Busca EXATA por cosseno, K=10, forca-bruta
escalar; mediana de 21 consultas, com a faixa min–max ao lado:

| N          | d=384        | d=768        | d=1536         |
|-----------:|-------------:|-------------:|---------------:|
| 10.000     | 3,40 ms      | 8,16 ms      | 17,32 ms       |
| 100.000    | 42,69 ms     | 82,83 ms     | 165,07 ms      |
| 1.000.000  | 410,94 ms    | 841,27 ms    | 1.656,63 ms    |

Custo cru de UMA distancia (produto interno + as duas normas), do mesmo sweep:
d=384 → 0,809 µs; d=768 → 1,639 µs; d=1536 → 3,400 µs.

**Veredito, com o corte «interativo» em ≤ 50 ms/consulta (explicito no
medidor):** a forca-bruta basta ate 100.000 × d=384 (42,7 ms) e **deixa de
bastar** ja a partir de 100.000 × d≥768 (82,8 ms), piorando de forma monotonica
ate 1,66 s em 1M × 1536. Nao ha «basta» universal: o corte depende de N×d e cai
**na primeira escala do sweep** (cem mil vetores) para as dimensoes reais de LLM
(768/1536). Corpus pequeno de dimensao baixa poderia nascer sem indice;
producao (100k+) em 768/1536 **pede ANN**.

E o que a especulacao anterior tinha de errado (ficava aqui: «para ~100 mil,
talvez poucos ms»): era otimista por ~15–30×. Cem mil × 1536 custa 165 ms, nao
«poucos ms». *Numero citado nao e numero medido* — e o `.hnsw` da §4 passa a ter
gargalo medido que o justifica em producao, nao so nos milhoes.

## 6. A ordem, entao

1. Este documento (feito).
2. Ler no fonte, como inspiracao medida e nao copia: **Cassandra 5.0 / JVector**
   (o HNSW deles) e o **pgvector**. Cada receita passa pelo nosso crivo.
3. `VECTOR<F32,N>` + busca EXATA (cosseno/euclidiana), provada contra vetor de
   referencia — como a cripto e provada contra RFC. Zero deps: a matematica e
   produto interno e norma, escrita a mao.
4. A bancada da premissa (§5), na maquina limpa. O numero.
5. `.hnsw` **so se o numero pedir**, com o design do grafo-sob-transacao na mesa.

Column store e SIMD ficam para depois de medir se ha gargalo analitico real —
provavelmente cache colunar leve + laco autovetorizado, nunca um segundo motor.
