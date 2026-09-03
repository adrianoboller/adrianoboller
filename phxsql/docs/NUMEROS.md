# Inventário dos números visíveis — de onde cada um vem

Papel H (documentação), rodada de 03/09/2026. Varredura de `docs/*.md`,
`README.md`, `MANUAL.txt`, `Dockerfile` (raiz e `bancada/replicacao/docker/`)
e os HTML de `docs/dossie/`, aplicando a lei do `CLAUDE.md`: **todo número
visível sai de um gerador, ou está errado e ninguém percebeu ainda**, com o
corolário de que a receita de um número também envelhece.

## Como esta varredura foi feita, e como não foi

Duas varreduras mais cedo nesta mesma rodada testaram um conferidor
automático baseado em "o número tem data/âncora ao lado?" e voltaram com
**1 defeito em 6** e **2 em 8** — proporção baixa demais para justificar uma
catraca nova. Repetir esse método aqui teria o mesmo problema: a maioria
esmagadora dos números "digitados sem data" neste repositório é legítima —
tabelas de bancada com a própria descrição do método ao lado, linhas
históricas do `PENDENCIAS.md`, o veredito de `GPU.md`. Marcar tudo isso como
defeito por falta de um "medido em DD/MM" literal geraria uma lista enorme de
falsos positivos.

O que esta varredura fez em vez disso: para cada número que fazia uma
afirmação no **presente** sobre o estado do sistema, sem data/âncora/til,
**procurou o mesmo número em outro lugar do repositório** — no gerador que
deveria tê-lo escrito, ou em outro documento que mede a mesma coisa. É assim
que esta casa já achou os três defeitos que o `CLAUDE.md` registra
(arredondamento, 276×280 testes, rodapé parado), e é assim que os dez
defeitos desta rodada foram achados: **por contradição medida, não por
ausência de data.**

## 1. Os geradores descobertos — lendo os scripts, não uma lista

Nove scripts escrevem blocos marcados. Descobertos varrendo os próprios
arquivos por `<!-- ...:inicio -->` / `<!-- GERADO: ... -->`, não por uma
lista digitada aqui:

| gerador | blocos que escreve | mecanismo |
|---|---|---|
| `docs/dossie/numeros-do-projeto.py` | dossiê: `projeto:`, `rodape:`, `selo:`, `idiomas:`, `<title>`; `README.md` `readme:testes:`; `docs/TESTES.md` `testes:total:`; `docs/REST.md` `rest:operacoes:`; `CAPABILITIES.json` | lê o repositório, **reescreve o arquivo** (substitui entre as marcas) |
| `docs/dossie/numeros-da-bancada.py` | dossiê: `bancada:`, `bancada:tabela:`, `bancada:diagnostico:`, `replicacao:`; `docs/PENDENCIAS.md` `pendencias:insercao:` | lê `bancada/resultados.json` e `bancada/replicacao/resultados.json`, **reescreve o arquivo** |
| `docs/dossie/pagina-dos-pedidos.py` | `docs/dossie/pedidos.html` inteiro; dossiê `pedidos:`; `docs/PENDENCIAS.md` `pedidos:contagem:` | lê `docs/PENDENCIAS.md`, **reescreve os três** |
| `docs/dossie/cobertura-por-area.py` | `docs/TESTES.md` `cobertura:`; dossiê `cobertura:` | conta `#[test]` por arquivo, **reescreve os dois** |
| `docs/dossie/capturas-no-dossie.py` | dossiê `capturas:` | embute as capturas como *data URI*, **reescreve o arquivo** |
| `bancada/guardas/tabela-no-testes.py` | `docs/TESTES.md` `guardas:` | lê o `--json` de uma corrida de `provar-guardas.py`, `len(GUARDAS)` do catálogo, **reescreve o arquivo** |
| `docs/tecnologias/extrair.py` | `docs/TECNOLOGIAS.md`, 16 blocos `<!-- GERADO: ... -->` | **só imprime** — quem atualiza o documento tem de colar o `stdout` a mão (ver achado §4) |
| `docs/qa/medir.py` | `docs/QA-PDCA.md` `catracas:` | varre `crates/*/examples/*.rs` atrás de `catraca:`, **reescreve o arquivo** |
| `bancada/sqlite/medir.py --documento` | `docs/MOBILE.md`, 8 blocos `mobile:*` | mede PhxSql × SQLite(R), **reescreve o arquivo**, reprova se um bloco marcado não existir ou sobrar sem marca |

Isso soma **43 blocos marcados** verificados nesta rodada, entre os nove
geradores. Todos os que foram reconferidos batem com a fonte (detalhe por
gerador abaixo) — a única exceção é o próprio `extrair.py`, tratado no
achado §4.

## 2. A tabela — número, onde, gerador ou digitado

Amostra representativa (a íntegra dos 43 blocos gerados mais os números
digitados lidos um a um nas seções de maior risco: capa dos documentos,
tabelas "Estado atual"/comparação, e todo lugar onde outro documento já
citava o mesmo número).

| número | onde (arquivo:linha) | sai de gerador? | gerador / classificação |
|---|---|---|---|
| 1.547 testes | `README.md:226`, `docs/TESTES.md:37`, `docs/PENDENCIAS.md` #172 | sim / sim / história datada | `numeros-do-projeto.py`; §5.5.3 de 03/09 |
| 77 guardas | `docs/TESTES.md:720` | sim | `bancada/guardas/tabela-no-testes.py`, medido em 2026-09-03 15:22 |
| 121 operações | `README.md:226`, `docs/REST.md:35` | sim | `numeros-do-projeto.py` (lê `OPERACOES` de `catalogo.rs`) |
| 175 pedidos (168/4/3) | dossiê `pedidos:`, `docs/PENDENCIAS.md` `pedidos:contagem:` | sim | `pagina-dos-pedidos.py`; conferido contra a contagem direta do `.md` — bate |
| 34.048 / 17.450 (replicação) | dossiê `replicacao:`, `docs/REPLICACAO.md:18-19`, `docs/DESEMPENHO.md:644` | sim / sim / história (antes/agora) | `numeros-da-bancada.py`; conferido contra `bancada/replicacao/resultados.json` — bate |
| `TETO_ROTULOS_E_CRASE = 1.720` | `docs/QA-PDCA.md` tabela `catracas:` | sim | `docs/qa/medir.py`, conferido contra a constante em `conferidor.rs:1239` — bate |
| 24 catracas na tabela mobile / 8 blocos `mobile:*` | `docs/MOBILE.md` | sim | `bancada/sqlite/medir.py --documento` |
| Veredito de GPU (99,4%/0,58%/63,0%/12,1%/28.234 MiB/s/3,90×/3,59×/2,51×/1,51×) | `docs/GPU.md`, repetido em `docs/TECNOLOGIAS.md` §5.2 | sim (na origem) / não (cópia manual) | reexecutei `bloco_gpu_veredito()`: saída **idêntica**, byte a byte, ao texto publicado — cópia ainda fiel |
| "Medido numa tabela de 200.000 linhas..." (7 ms / 18 ms / 72 ms / 131 ms) | `README.md:81-88` | não | história (descreve o método da própria medição) — legítimo |
| Multiplicadores da tabela "Estado atual" (2,4×, 3,1×, 1,8×, 20×, 87×) | `README.md:222-260` | não | história, todos conferidos contra `docs/DESEMPENHO.md` — nenhum contradito |
| "TETO em 1.577... baixou para 1.549" | `docs/QA-PDCA.md:417-420` | não | história com âncora de rodada explícita ("a mesma rodada") — legítimo |
| ~15 linhas amostradas do `PENDENCIAS.md` (#1, 6, 7, 17-19, 30, 113, 144, 151-154, 156, 172) | `docs/PENDENCIAS.md` | não | história/âncora de rodada em todas — o formato da tabela é inerentemente datado por pedido |
| 3,4 MB servidor + 1,2 MB cliente (musl) | `Dockerfile:12` (antes do conserto), `README.md:118`, `docs/CLUSTER.md:263` | não | **DEFEITO** — ver §3.1 |
| 113 rotas / 113 operações | `docs/REST.md:68`, `docs/REST.md:115` | não | **DEFEITO** — ver §3.2 |
| 142 guardas catalogadas | `docs/TECNOLOGIAS.md` §4.3 (antes do conserto) | "sim" (mas com bug) | **DEFEITO** — ver §3.3 |
| 28.914 linhas/s / 4.357 eventos/s (replicação, apresentado como atual) | `README.md` (antes do conserto), `docs/HFSQL.md:176` (antes do conserto) | não | **DEFEITO** — ver §3.4 |
| "REPLICACAO.md §10 e CLUSTER.md §2.2 ainda dizem 4.357/28.914" | `docs/CASSANDRA.md` (antes do conserto) | não | **DEFEITO** (nota de manutenção que ela mesma envelheceu) — ver §3.4 |

## 3. Os defeitos achados, e o conserto de cada um

### 3.1 — Tamanho do binário musl parado em "3,4 MB" (três arquivos)

`Dockerfile:12` (comentário), `README.md:118`, `docs/CLUSTER.md:263-264`
afirmavam, no presente e sem data, que o binário `static-pie` do alvo musl
tem 3,4 MB (servidor) e 1,2 MB (cliente). Já estava contraditado dentro do
próprio repositório: `docs/PENDENCIAS.md` #167 e
`docs/dossie/relatorio-conteineres.html` (selo 02/09/2026) medem o binário
do servidor, com `strip`, em **7,66 MB** — mais que o dobro.

**Conserto:** os três locais deixaram de repetir um tamanho fixo. Passaram a
citar a medição real (7,66 MB, datada, com a fonte) e a dizer explicitamente
que o número do **cliente** não foi remedido — não inventei um substituto.
`docs/CLUSTER.md` tinha um segundo número parado na mesma seção ("`docker
build` não foi executado" — verdade quando escrito, falso hoje: `PENDENCIAS`
#118 registra mais de dez corridas reais com o daemon no ar); também
corrigido, com data e referência.

### 3.2 — `docs/REST.md` repetia "113" ao lado de um "121" já corrigido

O bloco gerado (`rest:operacoes:`, linha 34-36) já diz **121 operações**, e o
próprio texto do bloco conta a história: já foi 113, depois 108, depois 121
num catálogo externo, e hoje "sai da constante `OPERACOES`". Só que duas
frases **fora** do bloco — linha 68 ("`bancada/rest/provar.py` percorre as
113 rotas...") e linha 115 ("Ele lista as 113 operações...") — continuavam
com o número antigo, contradizendo o parágrafo gerado poucas linhas acima no
mesmo documento.

**Conserto:** as duas frases pararam de repetir um dígito solto e passam a
apontar para o número gerado da §1, com uma frase explicando por que não se
repete o número aqui (é exatamente o hábito que causou o defeito). Os dois
outros "113" do arquivo (linha 349, um trecho de terminal citado
verbatim de uma corrida específica, e linha 471, narrando o histórico de
como a bancada foi escrita) são citações históricas legítimas e ficaram como
estavam.

### 3.3 — Bug no gerador: `docs/tecnologias/extrair.py` contava chaves, não guardas

**O achado mais importante desta rodada.** `bloco_guardas()` calculava o
número de defeitos catalogados contando `"{" seguido de quebra de linha` no
texto de `bancada/guardas/catalogo.py`, em vez de importar o módulo e usar
`len(GUARDAS)` — que é como o gerador irmão, `tabela-no-testes.py`, já faz.

A conta por regex **nunca teve relação estável com o número certo**: guardas
que usam o campo `trocas` (documentado no próprio `catalogo.py`, para o
defeito que mexe em mais de um ponto) trazem sub-dicionários
`{arquivo, trecho, troca}` que também batem no padrão `"{\n"` sem serem
guardas novas. Medido nesta rodada:

| método | resultado |
|---|---:|
| `len(GUARDAS)` (correto, importado) | **77** |
| regex `"{\n"` (o que o gerador fazia) | **180** |
| o que `docs/TECNOLOGIAS.md` publicava (rodada anterior, mesma regex) | 142 |
| `docs/TESTES.md:720`, gerado por `tabela-no-testes.py` (`len(GUARDAS)`, 2026-09-03 15:22) | **77** — confere |

Ou seja: mesmo se alguém tivesse rodado o extrator de novo sem consertar
nada, o número publicado teria pulado de 142 para 180 sem nenhuma guarda
nova — a regex nunca mediu o que dizia medir. É a mesma família do defeito
que o `CLAUDE.md` já registra para a receita do KiB de interface: um número
que parece gerado, mas a fórmula por trás está errada.

**Conserto:** `bloco_guardas()` em `docs/tecnologias/extrair.py:647-680`
agora importa `catalogo.py` e conta `len(GUARDAS)`. `docs/TECNOLOGIAS.md`
§4.3 foi atualizado para **77**, com o achado documentado no próprio
parágrafo. Aproveitei a mesma edição para trocar a referência a
`TETO = 1.549` — aposentado — por `TETO_ROTULOS_E_CRASE = 1.720`, o nome e o
valor certos hoje (conferido contra `conferidor.rs:1239` e contra a tabela,
já fresca, de `docs/QA-PDCA.md`).

**O que fica só reportado, não corrigido nesta rodada** (fora do escopo de
"consertar o que está errado"): as demais 15 tabelas geradas de
`docs/TECNOLOGIAS.md` também estão staleadas por crescimento natural do
código desde a última execução — por exemplo a tabela de linhas de Rust por
crate (documento publica 58.546 linhas de `src/`; medindo agora, 59.328) e
a contagem de pedidos recusados (documento publica 161/9; medindo agora,
175/12). Isso **não é o mesmo defeito**: a fórmula está certa, só o
documento não foi republicado depois que a árvore cresceu — o próprio
`docs/TECNOLOGIAS.md` já avisa que isso é esperado ("se o código mudou, os
números mudam com ele, o que é o comportamento certo, não um defeito do
documento"). Recomendo rodar `python3 docs/tecnologias/extrair.py` e colar
o resultado numa rodada dedicada — não aqui, para não misturar uma
republicação inteira com os consertos pontuais desta auditoria.

### 3.4 — A vazão da réplica ficou em "4.357 eventos/s" muito depois de virar 17.450

`docs/PENDENCIAS.md` #19 registra a primeira medição da replicação:
master 28.914 linhas/s, réplica sem número de vazão próprio nessa entrada.
`docs/PENDENCIAS.md` #144 já registra que essa dupla (28.914/4.357) foi
corrigida no **painel do dossiê**. O que ninguém tinha conferido é se ela
sobrevivia em **outros** documentos — e sobrevivia, em dois:

- `README.md` (seção "Replicação: Master e espelhos"): tabela inteira com
  os cinco números da primeira medição (28.914 linhas/s, 4.357 eventos/s,
  atraso 1,3-2,1 s, retomada "343 ms + 1,0 s"), sem data, como se fosse o
  estado atual.
- `docs/HFSQL.md:176`: "4.357 eventos/s por réplica" citado no presente,
  numa comparação direta com o HFSQL(R) — o pior lugar para um número
  desatualizado, porque é justamente onde o leitor decide se o PhxSql é
  competitivo.

E um terceiro documento, `docs/CASSANDRA.md`, já tinha uma "nota de
manutenção" apontando exatamente esse padrão — mas apontando para
`docs/REPLICACAO.md` §10 e `docs/CLUSTER.md` §2.2, que **já tinham sido
corrigidos** antes desta rodada (conferido: nenhum dos dois cita mais
4.357/28.914). A própria nota de manutenção tinha envelhecido, sem
mencionar o README, que era o terceiro lugar remanescente.

**Conserto:** `README.md` passou a citar os números atuais (34.048 / 17.450
/ 0,1-2,0 s / 323 ms + 0,3 s), datados em 29/08/2026 e citando
`bancada/replicacao/resultados.json` — a mesma fonte que
`numeros-da-bancada.py` já usa para o painel do dossiê, então não é um
número novo, é o mesmo número que já estava correto em outro lugar.
`docs/HFSQL.md` idem. A nota de `docs/CASSANDRA.md` foi atualizada para
"RESOLVIDA", registrando que o README era o terceiro local e já foi
corrigido no mesmo commit — sem apagar o histórico da nota original.

## 4. Achado de primeira ordem — lista digitada dentro de gerador

`docs/dossie/numeros-do-projeto.py:95-101` (`DOCS_AVULSOS`) é uma tupla
digitada com nove caminhos (`README.md`, `CHANGELOG.md`, `MANUAL.txt` e seis
`LEIA-ME.md`) usada por `linhas_de_doc()` para somar às linhas de
`docs/*.md`. O próprio comentário ao lado ("A receita do LEIA-ME, na letra.
Documento novo entra nos DOIS lugares", linha 94) já confessa o problema: a
MESMA lista existe **de novo**, digitada, dentro de `docs/dossie/LEIA-ME.md`
(o comando `cat docs/*.md README.md CHANGELOG.md MANUAL.txt bancada/LEIA-ME.md
...` na seção de conferência). Duas listas independentes da mesma coisa —
exatamente o padrão que já produziu o defeito do KiB de interface (três
arquivos digitados, `http.rs` passou a embutir nove, rodapé publicou 780 KiB
por 1.032).

Hoje as duas listas ainda coincidem (conferido: `linhas_de_doc()` roda sem
achar arquivo faltando, e o total medido agora — 39.390 — bate a poucas
dezenas de linhas com o publicado — 39.349 — diferença de commits normais
entre a geração e agora, não de lista errada). **Não é defeito ainda**, mas é
o mesmo formato de risco: um `LEIA-ME.md` novo em qualquer lugar do
repositório entra na conferência de `docs/dossie/LEIA-ME.md` (que é
documentação, feita para humano ler) sem entrar automaticamente em
`DOCS_AVULSOS` (que é código). Reportado, não consertado nesta rodada — trocar
por uma descoberta automática (`rglob("LEIA-ME.md")` mais os três arquivos
soltos da raiz) é mudança de comportamento do número "linhas de doc" e por
isso fica para quem decidir se topa esse número mudar.

Achado secundário, de risco bem mais baixo: `cobertura-por-area.py` tem três
mapas escritos à mão (`TESTES_DO_SERVER`, `DO_SERVIDOR`, `CRIPTO`, linhas
46-96) que decidem a QUE ÁREA cada arquivo pertence — mas o próprio
comentário do arquivo já explica por que isso é julgamento e não medição
("'isto é criptografia' é decisão de leitura de código que um script não
infere sozinho"), e o preço de errar é um arquivo caindo na categoria
"outros", não um número errado sobre uma categoria existente. Não reclassifico
isto como o mesmo defeito da §4 acima — é uma lista digitada por decisão
documentada, não por atalho não percebido.

## 5. A contagem final

| categoria | quantos |
|---|---:|
| Blocos marcados, gerador confirmado (reescreve o arquivo sozinho) | 43, de 9 geradores |
| Blocos gerados mas com transcrição manual (não reescreve sozinho) | 16 (`docs/TECNOLOGIAS.md`, via `extrair.py`) |
| Números digitados avaliados um a um pelo crivo | ~55 |
| — legítimos: história com data explícita | 6 |
| — legítimos: âncora de rodada | 4 |
| — legítimos: metodologia de medição (framing histórico sem data literal, conferidos contra a fonte) | ~35 |
| — **defeito** (presente, sem âncora, contradito por outra fonte do próprio repositório) | **10** locais, **4 causas-raiz** (§3.1-§3.4) |
| Bug de fórmula num gerador (não é "digitado", é receita errada) | 1 (`bloco_guardas()`, §3.3) |
| Lista digitada dentro de gerador, achado de primeira ordem | 1 (`DOCS_AVULSOS`, §4), não urgente hoje |

**A proporção de defeito nos números "digitados sem data" avaliados foi de
~10/55 (≈18%) — mas todos os dez foram achados por contradição medida contra
outra fonte do repositório, não pela ausência de data por si só.** Marcar
"sem data" como defeito teria produzido dezenas de falsos positivos (as ~35
linhas de metodologia/história listadas acima). **Recomendo não construir um
conferidor automático baseado em presença de data/âncora** — é a mesma
conclusão a que esta casa já chegou duas vezes hoje, e o padrão se repetiu
aqui: baixo valor por unidade de esforço, porque a maioria dos números
"digitados" neste repositório é história legítima, e o teste que separa
defeito de legítimo (contradição contra outra fonte) não é algo que um
conferidor genérico consiga fazer sem reimplementar, documento por
documento, o que cada número deveria valer hoje.

**O que vale a pena, e é bem mais barato que um conferidor:** fechar a lacuna
que deixou o bug de §3.3 sobreviver — fazer `docs/tecnologias/extrair.py`
reescrever `docs/TECNOLOGIAS.md` sozinho, como os outros oito geradores já
fazem, em vez de depender de alguém colar o `stdout` a mão. Enquanto essa
colagem for manual, um bug na fórmula (como o de `bloco_guardas()`) e um
esquecimento de colar são indistinguíveis de fora — os dois produzem o mesmo
sintoma, um número parado.

## 6. Como refazer esta varredura

```bash
# Os nove geradores, um a um (cada --so-medir mostra sem gravar):
python3 docs/dossie/numeros-do-projeto.py --so-medir docs/dossie/dossie-phxsql-0.18.html
python3 docs/dossie/numeros-da-bancada.py docs/dossie/dossie-phxsql-0.18.html
python3 docs/dossie/cobertura-por-area.py --so-medir docs/dossie/dossie-phxsql-0.18.html
python3 docs/qa/medir.py
python3 docs/tecnologias/extrair.py | diff - <(sed -n '/GERADO POR/,$p' docs/TECNOLOGIAS.md)

# A conferência cruzada que achou os defeitos de §3: procurar o mesmo número
# em mais de um lugar e ver se todos concordam com a fonte mais recente:
grep -rn "<o número suspeito>" docs/*.md README.md MANUAL.txt Dockerfile \
  docs/dossie/*.html
```
