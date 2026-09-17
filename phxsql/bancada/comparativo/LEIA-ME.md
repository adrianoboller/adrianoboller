# A bancada comparativa — o que falta aqui, contra quem tem

Três arquivos, e a ordem importa:

```bash
python3 bancada/comparativo/medir.py             # mede e grava resultados.json
python3 bancada/comparativo/documento.py         # escreve docs/COMPARATIVO.md
python3 bancada/comparativo/prova-dos-portoes.py # prova os portões do medidor
```

O `docs/COMPARATIVO.md` **não se edita**: a prosa mora no `documento.py` e a
medição no `resultados.json`. Documento gerado que alguém edita à mão volta a
ser documento digitado no commit seguinte, e ninguém percebe qual das duas
versões é a verdadeira.

## As três procedências, e por que a tabela as separa

**Motor vivo.** Quatro respondem nesta máquina — PhxSql (soquete, op `sql`),
MySQL(R), PostgreSQL(R) e SQLite(R) —, e a mesma pergunta vai para os quatro na
língua de cada um. Nenhuma célula dessas colunas é escrita à mão.

**Sonda de código**, só onde SQL não alcança. Cada uma aponta arquivo, linha
**e o trecho citado** — caminho pelado não é evidência, e foi assim que a sonda
do TLS ficou três rodadas apontando para o cliente de e-mail com o veredito
certo.

**Citado**, para HFSQL(R) e Cassandra(R), que não têm motor nem folha nesta
sessão. Citado não é mentira; é segunda mão, e a tabela marca 📄 célula a
célula para que ninguém a leia como medida.

## A evidência que cada célula carrega

Um parecer de fora, em 17/09/2026, achou em duas linhas o furo desta bancada:
a matriz dizia o veredito e **não dizia contra o quê**. O `resultados.json`
passou a gravar seis coisas que antes não gravava — e a lei que as governa é a
de sempre, *número medido, nunca digitado*: nenhuma delas se escreve à mão.

| campo | por que ele existe |
|---|---|
| `ambiente.commit` e `ambiente.branch` | de onde partir para refazer a corrida |
| `ambiente.arvore` | `limpa`, ou `SUJA` com a conta dos arquivos — a corrida de 16/09 rodou num binário que **se chamava** `6e717e6579ad-sujo`, e o arquivo não dizia |
| `ambiente.sha256_phxsqld` | qual binário respondeu, e não qual fonte existia |
| `ambiente.uname`, `cpus`, `memoria_total` | a máquina, porque número de bancada sem máquina ao lado não se compara |
| `ambiente.configuracao_do_phxsqld` | a configuração de fábrica da corrida, com `token` e `senha_hash` **tarjados** |
| `linhas[].cru` | o que cada motor respondeu: comando, código de saída e os dois canais |
| `linhas[].negativo` | o gêmeo que tem de ser **recusado**, e o que a recusa prova |

**A tarja é pétrea, e tem portão.** O `config.json` da oficina carrega `token`
e `senha_hash`; gravá-lo cru vazaria os dois num arquivo versionado. A tarja
deixa a **chave visível** com o valor omitido — apagar a chave junto esconderia
que a corrida rodou com token. E o portão `nenhum_segredo_no_json()` roda sobre
o JSON **já serializado**, não sobre o dicionário: o que vaza é o que se grava,
e uma tarja aplicada no ramo errado passaria por uma conferência feita no ramo
certo.

### O caso negativo: a metade da prova real que faltava aqui

Até esta data, uma célula virava `tem` porque a instrução passou. E «passou»
não distingue duas coisas muito diferentes: **o motor entendeu o construto**, ou
**o motor ignorou o que não entendeu e devolveu `ok`**. A segunda família não é
hipótese — o MySQL(R) 5.7 aceitava `CHECK` e o descartava em silêncio.

Cada item ganhou um gêmeo que tem de ser recusado, em dois formatos, e o
segundo é mais forte que o primeiro:

- **por EFEITO** — cria com o construto e pede o que o construto proíbe: a
  linha que viola o `CHECK`, a escrita na coluna calculada, a chave repetida
  **sem** o `ON CONFLICT`.
- **por RESOLUÇÃO** — nomeia dentro do construto algo que não existe, ou
  escreve o construto pela metade.

O gêmeo roda **só onde a positiva deu `tem`**, porque é o veredito afirmativo
que pode ser falso; onde a positiva foi recusada, a recusa já é a resposta, e o
controle global (`CREATE ZZZZ nao_existe_de_proposito`) já provou que este
cliente sabe ver recusa.

**Na primeira corrida (17/09/2026): 43 gêmeos recusaram como devia, 32 sem
caso, e 1 aceitou o que devia recusar.** O um é `view` no SQLite(R), e ele
matou uma premissa minha: eu supus que o gêmeo por resolução valesse em todo
motor. O SQLite(R) aceitou `CREATE VIEW v_neg AS SELECT nao_existe FROM c`
porque resolve o corpo da visão na **consulta**, não na criação — enquanto
MySQL(R) e PostgreSQL(R) recusaram os dois com a coluna nomeada no erro. Não
quer dizer que o SQLite(R) não tenha visão; quer dizer que **nele a aceitação
da criação não prova o corpo**, e a prova ali é por efeito.

O achado **fica**, com o número, em vez de ser consertado no escuro: o gêmeo
«óbvio» (`SELECT * FROM v_c WHERE nao_existe = 1`) recusaria **também** se o
`CREATE VIEW` tivesse sido um nada-a-fazer, porque aí a recusa seria «no such
table» — *recusa pelo motivo errado é a forma mais barata de um controle
negativo mentir a favor*. O gêmeo mais forte exige um controle **positivo** ao
lado, e isso é desenho da catraca, não conserto de agora.

### O que ainda NÃO entrou, e por quê

A **catraca** que reprova a publicação quando a prosa do dossiê contradiz estas
células. É a metade 2 do pedido 335 e espera a escolha do dono entre fonte
única interpolada e catraca que só detecta. Gravar a evidência vale nas duas
formas, e não se perde em nenhuma.

## Os portões, e o defeito de cada um

Cada portão abaixo existe porque o defeito passou. E cada um se prova com o
defeito **reposto**, por `PHX_CMP_DEFEITO=<nome> python3 …/medir.py` — é o que
o `prova-dos-portoes.py` faz, mais o sentido contrário: sem defeito, o medidor
tem de ir até o fim. Sem essa segunda metade, um medidor que parasse sempre
passaria nos quatro.

| defeito reposto | o portão que tem de disparar |
|---|---|
| `indice-velho` | `MESA NAO POSTA` — o índice volta ao formato `[{"coluna": 0}]`, o servidor recusa, e a tabela não nasce |
| `envelope` | `LEITOR QUEBRADO` — as sondas voltam a ler o envelope em vez do `resultado` |
| `catalogo-vazio` | `SONDA QUEBRADA` — a lista de operações chega vazia, e lista vazia não é ausência de visão |
| `config-com-segredo` | `SEGREDO NO ARTEFATO` — a `configuracao` vai ao JSON sem tarja, e `token` e `senha_hash` vazariam para um arquivo versionado |

E a prova real já derrubou um diagnóstico meu aqui: o interruptor começou
sendo «pedir o `catalogo` sem `database`», porque foi assim que eu expliquei o
zero. Reposto, o medidor **passou** — sem `database` o servidor responde igual.
O zero vinha do envelope lido no nível errado, e só. *Diagnóstico plausível não
é diagnóstico medido*, e o errado sobrevive melhor quando o conserto funcionou
por outro motivo.

Há mais dois portões que não têm interruptor porque não dependem do nosso
motor: o **controle positivo** (`CREATE ZZZZ nao_existe_de_proposito` tem de
ser recusado por **todos**; se algum aceitar, quem mente é o medidor) e a
**recompilação do `phxsqld` antes de medir** — a primeira corrida de 07/09
subiu binário anterior ao `procurar_texto` e publicou 118 operações onde havia
123.

## As cinco armadilhas já pagas

Estão contadas no `docs/COMPARATIVO.md` §1, com o que cada uma virou. O resumo
para quem for mexer aqui:

1. **Perguntar ao texto do repositório** achou o que não é — três células
   erradas na primeira corrida.
2. **Um portão que exigia diversidade de resposta** reprovou o PostgreSQL(R),
   que genuinamente tem tudo o que se perguntou por SQL.
3. **Recusa sem controle que passa** não prova nada: índice que devolve zero
   pode não existir, tabela que recusa o proibido pode nunca ter nascido.
4. **Ler o envelope** em vez do `resultado` deu «nenhuma operação entre as 0» —
   um `não` certo pelo motivo errado, que é o pior tipo de certo. Estava em
   quatro sondas irmãs de uma vez.
5. **Tabelas que nunca nasceram**: o índice ia no formato errado, a recusa não
   era lida, e as leituras seguintes achavam vazio e publicavam «o campo foi
   aceito e IGNORADO».

## Ao acrescentar uma capacidade à tabela

- Se ela se pergunta por SQL, entre em `PERGUNTAS` com a instrução **na língua
  de cada motor** — a mesma pergunta, não perguntas parecidas.
- Se não, entre em `sonda_codigo()` com `citar()`, que traz o trecho junto.
- Preencha as duas colunas citadas em `CITACOES`, ou marque `CITADO` com
  «não apurado» — célula em branco vira ausência aos olhos de quem lê.
- **Se a sonda mede EFEITO, escreva o controle antes do veredito.** É a regra
  que este diretório mais pagou para aprender.
