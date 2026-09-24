# Marca PhxSql

Arquivos oficiais, como recebidos:

| Arquivo | O que é |
|---|---|
| `phxsql-manual-de-marca.png` | Folha de marca: logo primário, ícone, versão horizontal, ícone de app, paleta e tipografia |
| `phxsql-logo.png` | Logotipo quadrado com assinatura *Phoenix Database Engine* |
| `phxsql-abertura.png` | Tela de abertura |

Em `derivados/`, o que a documentação usa — gerados dos originais, não
redesenhados:

| Arquivo | Onde entra |
|---|---|
| `phxsql-logo-560.png` | cabeçalho do `README.md` |
| `phxsql-simbolo-420.png` | aposentado — trazia a palavra cortada ao meio |
| `phxsql-simbolo-440.png` | capa do dossiê (embutido como data URI) |
| `phxsql-icone-128.png` | uso geral em miniatura |
| `phxsql-simbolo-224.png` | cartão de entrada do Centro de Controle |
| `phxsql-icone-64.png` | barra do Centro de Controle |
| `phxsql-icone-32.png` | ícone da aba do navegador |

Os três últimos são os que entram na interface web, embutidos no
`ui/index.html` como data URI — a página é um arquivo só, e não há de onde
buscar imagem.

**Eles têm alfa; os originais não.** O fundo `#010418` foi retirado
desfazendo a pré-multiplicação: subtrai o fundo, tira `alfa = max(r,g,b)` e
divide a cor por ele. Como o logo é brilho sobre quase preto, isso recupera
a cor real de cada pixel e a borda do brilho sai suave, em vez de recortada
com halo. É o que permite a fênix assentar em cima do painel `#0a1122` sem
deixar emenda de retângulo.

O símbolo de 440 substituiu o de 420 na capa do dossiê por dois defeitos que
apareceram juntos: o recorte antigo pegava o **topo da palavra "PhxSql"**, que
saía cortada ao meio, e a imagem **não tinha alfa**, então o `#010418` virava
um retângulo escuro solto sobre o papel claro do documento.

O recorte novo pega só a fênix e o cilindro — a palavra já aparece grande,
em texto, logo abaixo. E o alfa resolveu metade do problema, não todo: sobre
papel claro o **cilindro vira um fantasma branco**, porque o miolo escuro dele
era o fundo aparecendo. Por isso a capa põe a marca numa **placa** com o
`#010418`, com cantos arredondados e um brilho — deliberada, para ler como
apresentação da marca e não como retângulo perdido. Assim a mesma imagem
serve aos dois temas do dossiê.

O ícone da barra usa um **recorte mais fechado** que o do cartão — só a ave e
o cilindro, sem as pontas da asa nem as trilhas de circuito. Em 30 px o
desenho completo vira borrão; menos desenho é mais legível.

## Especificação

| | |
|---|---|
| Tipografia | **Exo 2** — SemiBold / Medium / Regular |
| Fundo | `#010418` (medido dos originais) |
| Assinatura | *Built to store. Engineered to scale.* |
| Subtítulo | *Phoenix Database Engine* |

```
#FFC43D   âmbar        destaque, números
#FF8A1C   laranja      acento no tema escuro
#FF4D10   vermelhão    acento (escurecido para #C63C0A sobre papel)
#D71A1A   vermelho     o .log nos diagramas
#8B0D0D   vinho
#DDE2EB   prata        cor do texto no tema escuro
```

### Onde cada cor pode ser TEXTO — medido, 24/09/2026

Contraste WCAG sobre o fundo `#010418` e sobre papel branco:

| Cor | sobre `#010418` | sobre branco | Como texto |
|---|---|---|---|
| `#FFC43D` âmbar | 12,8:1 | 1,6:1 | só no escuro |
| `#FF8A1C` laranja | 8,6:1 | 2,4:1 | só no escuro |
| `#FF4D10` vermelhão | 6,1:1 | 3,3:1 | no escuro; no claro vira `#C63C0A` |
| `#D71A1A` vermelho | **3,9:1** | 5,2:1 | **nunca no escuro** — só preenchimento/borda, ou letra grande (≥ 24 px, onde o mínimo é 3:1) |
| `#8B0D0D` vinho | **2,1:1** | 9,7:1 | **nunca no escuro** — só preenchimento/borda; no claro pode |
| `#DDE2EB` prata | 15,7:1 | 1,3:1 | só no escuro |

A regra das duas linhas em negrito é nova: o vermelho e o vinho entram na
tela escura como FUNDO de alerta ou traço, nunca como a cor da letra.

## A família Phoenix

Um símbolo só (a fênix e o cilindro) para os quatro produtos, e um **acento**
por produto, sempre da paleta — é ele que pinta o `x` da palavra e o traço de
luz sob ela:

| Produto | Acento |
|---|---|
| PhxSql | `#FF8A1C` laranja |
| PhxZip | `#FFC43D` âmbar |
| PhxMail | `#FF4D10` vermelhão |
| Phxblockchain | `#D71A1A` vermelho (letra de 150 px: passa como texto grande) |

## A marca em vetor — `vetor/`

Até 24/09/2026 a marca só existia em PNG (1,3 a 2,3 MB, sem alfa), e cada
tamanho novo era um recorte à mão. Agora:

| Arquivo | O que é |
|---|---|
| `vetor/phx-simbolo.svg` | o símbolo, **desenhado à mão** a partir da folha — a única fonte vetorial |
| `vetor/phx-icone.svg` | o mesmo, para 16–48 px: sem trilhas nem luzes, traço mais grosso, cilindro mais claro |
| `vetor/phx-simbolo-mono.svg` | uma cor só (`currentColor`), para impressão e carimbo; `--vazio` troca o papel |
| `vetor/<produto>-horizontal.svg` | símbolo + palavra em **curvas** (Exo 2 SemiBold) + assinatura, para os quatro produtos |
| `derivados/vetor/` | PNG de 16 a 1200 px, Android 192/512, iOS 180 e o `phx.ico` (16/32/48/256) |

### O símbolo do PhxZip — duas propostas, pedido do dono (24/09/2026)

*«O logo deve ter uma morsa de aperto e a Phoenix pousada em cima. Pode ser
um cadeado laranja com asas.»*

| Arquivo | O que é |
|---|---|
| `vetor/phxzip-simbolo-morsa.svg` | a fênix pousada na morsa que aperta o arquivo — **em uso** no PhxZip web e no horizontal |
| `vetor/phxzip-simbolo-cadeado.svg` | o cadeado laranja com asas — alternativa |
| `vetor/phxzip-icone-{morsa,cadeado}.svg` | os dois, recortados para 16–48 px |

A morsa entrou por ser o «deve»; o cadeado lê melhor em 16 px (medido nas
capturas de 32 px) — a escolha final é do dono. As asas, o pescoço e a cabeça
vêm do `phx-simbolo.svg` por leitura (`vetor/gerar-phxzip.py`): a família
continua sendo um desenho só de fênix.

Refazer: `python3 vetor/gerar-phxzip.py`, depois `python3 vetor/gerar.py Exo2[wght].ttf` (precisa de `fontTools`,
ferramenta de trabalho, não do produto) e `node vetor/exportar.mjs`.

**Estado: PROPOSTA a aprovar pelo dono.** O SVG é uma redesenho fiel à
folha, não a folha: os PNG originais continuam sendo a marca oficial até o
dono aprovar o vetor. O que já usa o vetor hoje: o PhxZip web (ícone da aba e
símbolo do cabeçalho).

## As fontes — `fontes/`

Exo 2 (variável, 400–700) e IBM Plex Mono (400/500/600), subconjunto latino,
**86.112 bytes** de woff2, licença SIL OFL 1.1 (os `OFL-*.txt` ao lado).
Entram embutidas no binário pelo `phxsql_core::fontes`, para o PhxSql e o
PhxZip — a marca aparece igual sem internet, e a tela não pede nada ao Google.

## Como a marca entra no dossiê

O acento e a tipografia vêm daqui. Duas adaptações deliberadas, para o
documento continuar sendo um documento:

- **O corpo do texto não é Exo 2.** Exo 2 é a voz da marca e leva títulos,
  rótulos e o cromo da página; parágrafos longos ficam numa serifada. É
  extensão de marca, não desvio: uma face geométrica cansa em texto corrido.
- **O vermelhão escurece no tema claro.** `#FF4D10` sobre papel dá ~3,5:1 de
  contraste, abaixo do mínimo para texto. Vira `#C63C0A`, que mantém a cor da
  marca e passa dos 4,5:1.

As cinco cores de arquivo dos diagramas (`.reg` `.ndx` `.bin` `.memo` `.log`)
precisam se distinguir **entre si e do acento**. Por isso o `.bin`, que era
âmbar, virou ciano: âmbar ao lado do laranja da marca vira ruído. O `.log`
ficou com o vermelho `#D71A1A` da paleta, que é onde ele encaixa sozinho.

## A folha de marca: o original e o corrigido

A folha original afirma *"Reliable storage — ACID compliant and durable."*
Falso: o isolamento entregue por padrão é `READ COMMITTED` (`docs/ACID.md`,
`docs/CONTRATO-1.0.md` §2.1), e «durable» sem ressalva também não — no regime
padrão uma escrita comum responde OK sem `fsync` (`ACID.md` §5.1).

`derivados/phxsql-manual-de-marca-corrigido.png` troca aquele bloco por
**«TRANSACTIONAL STORAGE — Commit, rollback and savepoints.»**, que é verdade
medida desde o pedido 162, com a Exo 2 embutida e a cor de fundo amostrada ao
lado. Refazer: `node vetor/corrigir-folha.mjs`. **É o corrigido que vai para
cliente**; o original fica como registro do que se recebeu.

*"Built-in replication — high availability and failover ready"* é verdade:
a replicação está medida com quatro servidores, e o cluster faz eleição e
promoção automática (`docs/REPLICACAO.md`).
