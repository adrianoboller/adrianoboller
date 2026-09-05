# Utilização padrão: a bancada de quem **usa** o banco

Os dois pedidos de 05/09/2026, palavra do dono:

> *«Bateria de testes de utilização padrão criar base, incluir 20.000 registros
> tabela complexa com e sem binários e memos. Testes de paginação alfabética.»*

Nada aqui chama o motor por dentro. Tudo passa pelo soquete, contra o
`phxsqld` de pé — que é por onde passam a tela, o driver ODBC e qualquer
cliente. É essa a diferença que faz esta bancada achar o que os testes do
motor não acham.

```bash
flock /tmp/phx-cargo.lock cargo build --release --bin phxsqld
python3 bancada/utilizacao-padrao/medir.py 20000
python3 bancada/utilizacao-padrao/paginacao-alfabetica.py
python3 bancada/utilizacao-padrao/gera-leia-me.py     # reescreve este arquivo
```

| arquivo | o que é |
|---|---|
| `medir.py` | a carga: cria a base, cria a tabela complexa em três formas, grava 20.000 linhas em cada, lê tudo de volta e **confere** |
| `paginacao-alfabetica.py` | a partição por letra pela porta de dados, com controle em toda afirmação |
| `oficina.py` | o que as duas compartilham: subir o servidor, falar contando os bytes, somar o disco |
| `gera-leia-me.py` | **o gerador**: nenhum número deste arquivo se digita |
| `resultado.json` / `resultado-alfabetica.json` | a última medição, crua |

<!-- GERADO: capa -->
| | |
|---|---|
| linhas por lado | **20.000** |
| lados comparados | 3 — `sem`, `com` (Bin+Memo), `largo` (Str) |
| colunas declaradas → colunas no esquema | `sem`: 10 → **12**, `com` (Bin+Memo): 12 → **14**, `largo` (Str): 12 → **14** |
| índices em cada lado | 5 — `porFilialId`, `porCodigo`, `porNome`, `porCidade`, `porCategoria` |
| blob por linha | 256 bytes no `Bin`, 600 caracteres no `Memo` |
| linhas conferidas de volta | 60.000 de 60.000, **0 divergências** |
| afirmações da partição alfabética | **18**, 0 sem confirmar |
| tempo publicável | cargas: sim, chave_conferida: sim |
| medido contra | `phxsqld 0.18.0 (1cff41c6e3be-sujo) x86_64-unknown-linux-gnu` |
<!-- FIM: capa -->

## A tabela complexa, e por que cada peça está nela

Uma tabela complexa é decisão de quem monta a bancada, e ela se justifica
escrita. Cada peça está aqui porque exercita um caminho **diferente** do motor,
e não para engrossar a lista — uma tabela com dez colunas do mesmo tipo e um
índice mediria a mesma coisa dez vezes.

<!-- GERADO: esquema -->
| coluna | tipo | por que ela está aqui |
|---|---|---|
| `filial` **(obrigatória)** | `Int2` | metade da **chave composta**, e a que faz o índice comparar duas colunas |
| `id` **(obrigatória)** | `Int8` | a outra metade da chave composta |
| `codigo` **(obrigatória)** | `Str(24)` | **índice único próprio** — o motor lê antes de gravar para poder recusar a repetida |
| `nome` | `Str(60)` | **índice sem caixa** (`nocase`), que compara dobrando a caixa |
| `cidade` | `Str(30)` | **índice de baixa cardinalidade** (8 valores): folha longa, o caso oposto do único |
| `nascimento` | `Date` | dias inteiros no disco, texto ISO no fio |
| `criado_em` | `DateTime` | milissegundos no disco, e um **terceiro** formato de volta |
| `saldo` | `Decimal(15,2)` | i128 escalado: **recusa** número em JSON e exige texto, para não perder centavo em `f64` |
| `ativo` | `Bool` | um byte |
| `categoria_id` | `Int8` | a **chave estrangeira**, que nasce conferida |
| `observacao` — só nos lados `com` e `largo` | `Memo` / `Str(n)` | o texto longo — `Memo` num lado, `Str` de largura fixa no outro |
| `foto` — só nos lados `com` e `largo` | `Bin` / `Str(n)` | o binário — `Bin` num lado, o mesmo hexadecimal em `Str` no outro |

| índice | colunas | marca |
|---|---|---|
| `porFilialId` | `filial`, `id` | primário, único, composto |
| `porCodigo` | `codigo` | único |
| `porNome` | `nome nocase` | sem caixa |
| `porCidade` | `cidade` | comum |
| `porCategoria` | `categoria_id` | comum |

E a chave estrangeira `fk_categoria`: `categoria_id` → `categorias(id)`, com `ao_excluir: restringir` e `ao_alterar: cascata`. Ela **nasce conferida** — o `verificar` nem é mandado no pedido —, e chave conferida exige índice **dos dois lados**: `porId` na mãe e `porCategoria` na filha.

As **colunas de sistema** entram sozinhas, e é por isso que a linha «colunas declaradas → colunas no esquema» da capa tem dois números: `sem` 10 → 12, `com` (Bin+Memo) 12 → 14, `largo` (Str) 12 → 14. Coluna de sistema nova já quebrou *todo salvar e todo incluir* pela tela uma vez, e é por isso que ela está exercitada aqui — o `rownum` de toda linha é conferido na leitura de volta.
<!-- FIM: esquema -->

O que **não** está aqui, e por quê: *coluna com valor padrão*. Ela não existe
no esquema, e isso está medido mais abaixo, não suposto.

## Os três lados, e por que são três

O eixo do pedido é «com e sem binários e memos», e ele tem uma armadilha que a
`bancada/LEIA-ME.md` já descreve com dois números: **bancada compara trabalho
igual, não só pergunta igual**. Uma tabela com `Bin` e `Memo` grava em dois
arquivos que a outra nem abre — então «20.000 linhas com blob» e «20.000 sem
blob» não são o mesmo trabalho, e publicar a razão entre as duas como se fosse
o custo do motor seria o **terceiro** erro da série (depois do
`WHERE id IN (…)` contra vinte mil buscas separadas, 41× a favor do outro
motor, e do `COUNT(*)+SUM` sobre 1.250.000 linhas contra a leitura de 20.000,
5× a favor do nosso).

Por isso há um **terceiro lado**, e é ele que separa as duas metades:

<!-- GERADO: os-tres-lados -->
| lado | o que ele é | fio (B/linha) | disco (B/linha) | `.reg` | `.ndx` | `.bin` | `.memo` |
|---|---|---:|---:|---:|---:|---:|---:|
| `sem` | 10 colunas de dado, 5 índices. Nenhum arquivo externo. | 209,0 | 603,2 | 3,7 MiB | 6,9 MiB | 0,1 KiB | 0,1 KiB |
| `com` (Bin+Memo) | as mesmas 10 mais `observacao` **Memo** e `foto` **Bin**. | 1384,1 | 1529,8 | 4,3 MiB | 6,9 MiB | 5,2 MiB | 11,9 MiB |
| `largo` (Str) | as mesmas 10 mais `observacao` e `foto` com os **mesmos nomes e os mesmos valores**, declaradas `Str(n)` — o pedido no fio é byte a byte igual ao do `com`. | 1384,1 | 1763,2 | 25,9 MiB | 6,9 MiB | 0,1 KiB | 0,1 KiB |

O `.log` é igual nos três (859,4 KiB) e o `.trash`, o `.reason` e o `.pag` são só cabeçalho.
<!-- FIM: os-tres-lados -->

### O que cada diferença mede

<!-- GERADO: decomposicao -->
| a diferença | o que ela mede | fio | disco |
|---|---|---:|---:|
| `sem` → `largo` | **o peso no fio e no slot**: o mesmo JSON, guardado em coluna de largura fixa, sem nenhum arquivo externo | +1175,1 B/linha | +1160,0 B/linha |
| `largo` → `com` | **o `.bin` e o `.memo`**: o mesmo pedido no fio, outro destino no disco | 0 B/linha (idêntico) | -233,4 B/linha |

O dado que a linha carrega são **856 bytes** (256 no binário e 600 caracteres no texto). Guardado no `.bin`/`.memo` ele custa **926,6 bytes por linha** de disco — 8,2% de sobra, que são o cabeçalho do bloco, o CRC e o ponteiro que entra no slot. Guardado em `Str` de largura fixa custa **1160,0 bytes por linha**, e a conta fecha exatamente com as larguras declaradas: o `.reg` cresce o que a coluna pediu, esteja ela cheia ou vazia.

E no **fio** os dois lados custam o mesmo: +1175,1 bytes por linha, dos quais 512 são o hexadecimal do `Bin` — **um binário viaja com o dobro do tamanho** porque JSON não tem tipo binário.
<!-- FIM: decomposicao -->

### O tempo

<!-- GERADO: tempo -->
Duas cargas por lado, em tabelas próprias, na ordem `sem` → `com` → `largo` → `r2sem` → `r2com` → `r2largo`. Duas colunas de tempo, porque elas não medem a mesma coisa: **parede** inclui montar o JSON no cliente, o fio e a análise da volta; **motor** é o `ms` que o próprio servidor carimba na resposta.

| lado | 1ª carga (parede / motor) | 2ª carga (parede / motor) | 2ª: linhas/s |
|---|---:|---:|---:|
| `sem` | 2,30 / 2,07 s | 2,26 / 2,02 s | 8.850 |
| `com` (Bin+Memo) | 4,45 / 3,43 s | 2,27 / 2,04 s | 8.809 |
| `largo` (Str) | 4,17 / 3,15 s | 2,34 / 2,12 s | 8.536 |

**Na segunda carga os três lados custam o mesmo, dentro de 0,10 s de diferença** (`sem` 2,02 s / `com` (Bin+Memo) 2,04 s / `largo` (Str) 2,12 s de motor) — ou seja, o tempo **não separa** os três lados. Quem separa são os bytes, e eles estão nas tabelas acima.

**E há um efeito que eu não sei explicar, e ele fica escrito assim.** A primeira carga dos dois lados com peso grande custa até **1,68×** a segunda do mesmo lado, e a diferença aparece dentro do motor, não só no fio. Dois controles mataram as duas explicações óbvias, e nenhum deles explicou o efeito:

1. **não é a posição na fila** — `PHX_ORDEM_INVERTIDA=1` inverte a ordem das seis cargas e o padrão não muda de dono: os mesmos dois lados saem lentos na primeira e rápidos na segunda;
2. **não é «a primeira carga de uma série»** — o controle da chave conferida, logo abaixo, faz três cargas idênticas seguidas e as três custam o mesmo, nos dois braços.

Enquanto não houver causa medida, a conclusão publicada é a que **as duas colunas concordam**: entre a diferença dos lados e a diferença entre as duas cargas do mesmo lado, a segunda é maior. *Número citado é número que não se mede* — e diagnóstico plausível não é diagnóstico medido.
<!-- FIM: tempo -->

### O que a chave estrangeira conferida custa

<!-- GERADO: chave-conferida -->
| a chave | 1ª carga | 2ª | 3ª | mediana (motor) | µs por linha |
|---|---:|---:|---:|---:|---:|
| **declarada? não** — só o índice `porCategoria` | 0,65 | 0,64 | 0,64 | **0,64 s** | 31,9 |
| **conferida** (o que a tabela desta bancada usa) | 2,09 | 2,04 | 2,05 | **2,05 s** | 102,6 |

**A chave conferida custa 3,21× a gravação.** Os dois braços diferem em uma coisa só — a declaração da chave; o índice `porCategoria` existe nos dois —, então o que está medido é a **conferência**, e não o índice. É o preço da regra primordial da casa cobrado na entrada: para cada linha gravada, o motor pergunta à mãe se o pai existe e se ele está vivo. `docs/DESEMPENHO.md` §15 mede a mesma coisa por dentro, com `--example custo-da-fk`; aqui ela é medida pela porta de dados, com a tabela inteira.

E ele é maior que o dos blobs — que no tempo é **zero**, medido no bloco acima. O que **não** está medido aqui é quanto cada um dos cinco índices custa: para dizer isso seria preciso um braço por índice, e ele não existe. Quem carregar milhões de linhas e puder conferir depois tem aqui o número para decidir; quem não puder tem aqui o que a garantia custa.

De quebra, ele é o **controle da posição**: três cargas idênticas, uma atrás da outra, com o mesmo esquema e as mesmas linhas. Nos dois braços as três custam o mesmo — logo, «ser a primeira carga» não explica sozinho o que o bloco do tempo mostra.
<!-- FIM: chave-conferida -->

### Quantos `fsync`

<!-- GERADO: fsync -->
| lado | ação | `.reg` | `.ndx` | `.bin` | `.memo` | `.log` | `.trash` | `.reason` | total |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `sem` | um lote de 500 | 1 | 2 | 1 | 1 | 1 | 1 | 1 | **8** |
| `sem` | uma linha | 1 | 2 | 1 | 1 | 1 | 1 | 1 | **8** |
| `com` (Bin+Memo) | um lote de 500 | 1 | 2 | 1 | 1 | 1 | 1 | 1 | **8** |
| `com` (Bin+Memo) | uma linha | 1 | 2 | 1 | 1 | 1 | 1 | 1 | **8** |
| `largo` (Str) | um lote de 500 | 1 | 2 | 1 | 1 | 1 | 1 | 1 | **8** |
| `largo` (Str) | uma linha | 1 | 2 | 1 | 1 | 1 | 1 | 1 | **8** |

Medido em `recursos.durabilidade = "por_operacao"`, e não na configuração de fábrica — de propósito: a de fábrica (`por_lote`, 200 operações ou 200 ms) fecha a janela pelo **relógio**, e aí a contagem passa a depender de quantas vezes o relógio bateu no meio da carga. Medido assim numa primeira corrida, o mesmo lote deu 2 `fsync` no `.reg` para um lado e 1 para o outro, e a diferença era o relógio.

**O achado: o `.bin` e o `.memo` custam ZERO `fsync` a mais.** O fecho da janela sincroniza os oito arquivos da tabela **exista ou não** coluna que os use — o lado `sem`, que não tem `Bin` nem `Memo` nenhum, paga o `fsync` do `.bin` e do `.memo` igual aos outros dois. O custo do blob aparece em **bytes**, e não em chamadas ao disco.
<!-- FIM: fsync -->

## A conferência: carga que não confere o que gravou mede o soquete

Toda linha volta pelo `varrer` e é comparada **campo a campo**, e os dois blobs
são comparados **byte a byte**. Três campos voltam num formato diferente do que
foram mandados — a data, o instante e o decimal — e os três são conferidos
contra uma conta feita em Python, dois códigos sem uma linha em comum. É o
mesmo método que provou a soma da varredura na bancada de comparação.

<!-- GERADO: conferencia -->
| lado | linhas lidas | páginas | divergências | o comparador acusa o estrago? |
|---|---:|---:|---:|---|
| `sem` | 20.000 | 10 | 0 | escalar → `cidade`; decimal → `saldo` |
| `com` (Bin+Memo) | 20.000 | 10 | 0 | escalar → `cidade`; decimal → `saldo`; um byte do blob → `foto`; ultimo char do memo → `observacao` |
| `largo` (Str) | 20.000 | 10 | 0 | escalar → `cidade`; decimal → `saldo`; um byte do blob → `foto`; ultimo char do memo → `observacao` |

A última coluna é o **controle positivo**, e ele roda na mesma corrida: uma cópia do valor esperado é estragada de propósito — um caractere na cidade, um centavo no `Decimal`, **um byte** no hexadecimal do blob, o último caractere do memo — e o mesmo comparador tem de nomear o campo. Sem isso, «zero divergências» poderia ser um comparador cego, e esta casa já publicou zero com um medidor cego. Sem estrago nenhum ele cala: `sem` `[]`, `com` (Bin+Memo) `[]`, `largo` (Str) `[]`.
<!-- FIM: conferencia -->

## A integridade referencial, pela porta de dados

<!-- GERADO: integridade -->
| o que se pediu | o que o servidor respondeu |
|---|---|
| excluir (suave) a categoria que tem filhas | `[SP000008] integridade referencial: categorias: esta linha tem filhas em com pela chave "fk_categoria". Nunca se apaga o registro pai que tem filhos -- apague as filhas antes` |
| excluir de vez a mesma categoria | `[SP000008] integridade referencial: categorias: esta linha tem filhas em com pela chave "fk_categoria". Nunca se apaga o registro pai que tem filhos -- apague as filhas antes` |
| inserir linha apontando para categoria que não existe | `[SP000008] integridade referencial: fk_categoria: nao existe categorias(id) com esse valor` |
| inserir com `codigo` repetido (índice único) | `[SP000020] chave duplicada: indice unico porCodigo ja tem essa chave` |
| **controle** — excluir de vez uma categoria SEM filhas | passou, como tinha de passar |

As duas primeiras linhas são a regra primordial da casa pela porta de dados: *nunca se mata o pai que tem filhos* — e o **suave** também, porque pai logicamente morto deixa filha apontando para linha que a tela não mostra mais. O controle da última linha é o que separa «recusou por causa das filhas» de «recusa sempre».
<!-- FIM: integridade -->

## «Coluna com padrão»: o que existe, e onde termina

<!-- GERADO: coluna-com-padrao -->
| | |
|---|---|
| slots reescritos | 20.000 |
| índices refeitos | não — o `.ndx` aponta para rowid, e o rowid não mudou |
| nas linhas que já existiam (a primeira e a última) | `ativo`, `ativo` |
| na linha inserida **depois** | **nulo** |

**Coluna com valor padrão não existe no esquema, e isto está medido, não suposto.** `Column` guarda id, nome, rótulo, descrição, máscara, tipo, se aceita nulo e a classificação de dado pessoal — e mais nada. O único `padrao` do motor está no `acrescentar_coluna`, e ali ele é o valor de **preenchimento** das linhas que já existem: a linha inserida depois nasce **nula**, como a tabela acima mostra. Pedir «coluna com padrão» a esta bancada seria pedir uma funcionalidade que não há; o que ela faz é exercitar a que há e dizer onde termina.
<!-- FIM: coluna-com-padrao -->

## A partição alfabética pela porta de dados

O motor já está provado por dentro: `crates/phxsql-store/tests/alfanumerica.rs`
tem dezesseis testes que cobrem o arquivo da letra, o endereço por conta, o
`rownum`, o salto pelos vazios e a recusa de alterar a coluna de referência. O
que faltava era o caminho de **fora**.

E ele achou o que os dezesseis não achavam. O `varrer` monta o campo `ha_antes`
com `pagina_antes_de`, que andava **de um em um para trás** com o `ler` cru — e
na alfanumérica o slot entre o fim de um balde e o começo do próximo **não
existe**, então o `ler` responde `NaoEncontrado` em vez de `None`. Toda página
que começasse no primeiro slot de um balde voltava
`[SP000018] rowid N nao existe` no lugar das linhas, e a página 1 também,
quando o balde `_A` estava vazio.

O padrão é o da casa, escrito no `CLAUDE.md`: **conserto entra no caminho que o
motivou, e o caminho irmão fica.** `pagina_depois_de` nasceu sabendo saltar o
vazio (ela anda pelo `proximo_ativo`); a de voltar ficou com o laço cru. Hoje
há `RegFile::anterior_ativo`, que é o irmão que faltava, e a guarda
`pagina-anterior-de-um-em-um` no `bancada/guardas/catalogo.py` repõe o defeito
para provar que o teste continua caindo.

<!-- GERADO: alfabetica -->
| o que se afirma | controle da mesma corrida | confere |
|---|---|---|
| cada balde ocupado vira `porletra_<letra>.reg` no disco | o mesmo listador nao acha `_Q` nem `_X`, que nao receberam linha: [] | sim |
| letra que nunca recebeu linha nao cria arquivo vazio | na mesma listagem, `A` e `Z` aparecem: ['A', 'Z'] | sim |
| «Álvaro» e «Alvaro» vao para o MESMO arquivo (`_A`), e «Çelik» para o `_C`, e «Éder» para o `_E` | o mesmo endereco poe «Bruno» em 1001..2000 (balde B): True | sim |
| «0800» NAO cai em Outros: os dez algarismos tem balde proprio, e o arquivo se chama `porletra_0.reg` | na mesma corrida, `#etc` cai em Outros (36001..37000): True | sim |
| `#etc`, vazio, so espacos e `日本` vao todos para `_Outros` | e o mesmo criterio deixa `Mendes` fora de Outros: True | sim |
| o `rownum` de cada linha e a posicao em que ela foi DIGITADA, e nao a posicao dela no arquivo | na mesma corrida os rowids NAO sao crescentes na ordem de digitacao (o primeiro digitado tem rowid 18001 e o segundo tem 1) | sim |
| a leitura sai na ordem dos baldes (alfabetica), que e outra ordem que a de digitacao | e o `rownum` da mesma lista sai fora de ordem: [2, 5, 6, 4, 12, 13] | sim |
| com 1.000.000 de slots por balde, a varredura devolve as tres linhas pelos enderecos calculados, sem andar pelos 25 milhoes de slots vazios | terminou em 0.014 s; andar de um em um seriam 25.000.000 de leituras, que nao cabem em segundo nenhum (este e o UNICO numero de tempo desta prova, e ele e um limite, nao um desempenho) | sim |
| pedir a pagina ANTERIOR a Zeus atravessa os mesmos vazios e devolve Adriano e Mendes | e perguntar o que vem antes da primeira linha devolve lista vazia, e nao erro: 0 linha(s) | sim |
| paginar de tres em tres com `pular`/`max` devolve exatamente a mesma lista da varredura inteira, atravessando os baldes | 4 das 5 paginas comecam no PRIMEIRO slot de um balde -- que e o caso que voltava «rowid N nao existe» antes do conserto | sim |
| a posicao NAO e o `rownum` aqui, entao o `pular` anda em vez de bissetar -- e a resposta diz isso | numa tabela sem particao por letra o mesmo campo diz «bisseccao»; aqui bissetar devolveria a linha errada em silencio, porque o `rownum` nao cresce com o rowid | sim |
| o cursor `depois`/`cursor_fim` percorre a tabela inteira atravessando os baldes | sao 5 paginas de 3 para 14 linhas | sim |
| `ha_antes` responde em todas as paginas -- e e falso so na primeira | era exatamente este campo que derrubava a varredura: ele chama a pagina ANTERIOR para saber se ela existe | sim |
| `desde_rownum` acha a linha cujo numero de ordem e o pedido, e nao a que estiver naquela posicao do arquivo | a linha de rownum 5 e 'Álvaro' | sim |
| e o resto da pagina sai na ordem dos BALDES, nao na ordem de digitacao -- comportamento, e nao defeito: o `rownum` e o ponto de partida, e a leitura continua sendo a do arquivo | os numeros de ordem da pagina saem assim: [5, 6, 4, 12] | sim |
| trocar «Silva» por «Andrade» muda o arquivo em que a linha mora, e a gravacao recusa | a recusa DIZ o que fazer: '[SP000018] esquema invalido: a alteracao mudaria o balde de S para A, e o balde e o endereco fisico da linha em porletra. Exclua e insira de novo: a linha nova nasce no balde certo, com outro rowid' | sim |
| e a mensagem manda excluir e inserir, em vez de so dizer «nao» | — | sim |
| «Silva» -> «Silveira» fica no `_S` e e aceita | e o rowid nao mudou, que e o que a recusa acima protege | sim |

**18 afirmações, 0 sem confirmar**, medidas contra `phxsqld 0.18.0 (1cff41c6e3be-sujo) x86_64-unknown-linux-gnu`. Os arquivos que a partição criou no disco: `porletra_0.reg`, `porletra_A.reg`, `porletra_B.reg`, `porletra_C.reg`, `porletra_E.reg`, `porletra_M.reg`, `porletra_Outros.reg`, `porletra_S.reg`, `porletra_Z.reg`.
<!-- FIM: alfabetica -->

### E uma coisa que só a porta de dados responde

**A página 2 de uma tabela por letra atravessa balde?** Atravessa — e o preço
está dito na própria resposta: o campo `salto` volta `"passo"` e nunca
`"bisseccao"`. Numa tabela comum o `pular` bisseta pelo `rownum`, que custa
`log2 N` leituras; aqui não pode, porque o `rownum` **não cresce com o rowid** —
a Silva digitada primeiro mora no `_S`, com rowid alto, e a Alves digitada
depois mora no `_A`, com rowid 1. Bissetar uma sequência que não está ordenada
devolveria a linha errada **em silêncio**, que é pior que devolver devagar. O
motor sabe disso e anda; a resposta diz qual dos dois pagou.

O `desde_rownum` também responde, e o que ele faz merece estar escrito: ele
**acha** a linha pelo número de ordem (varrendo, não bissetando), e daí em
diante continua na ordem dos **baldes**. O `rownum` é o ponto de partida, não a
ordem da página.

## O que esta bancada NÃO mede

**Transação.** Não há `BEGIN` aqui: o caminho medido é o de carga comum, que é
o que a maioria dos clientes faz.

**Concorrência.** Uma conexão só. A trava global de dados serializa tudo de
qualquer forma (`docs/PENDENCIAS.md`, item 4 da lista curta).

**Queda de energia.** O `fsync` é contado, não provocado — quem prova queda é
`bancada/durabilidade/` e `bancada/exclusao/`.

**O tempo, quando o portão acusa.** É a regra do gerador, e não uma decisão de
quem rodou — e ela vale **por fase**: a corrida leva um minuto e meio, e numa
máquina com vizinho ativo o veredito da corrida inteira jogaria fora também o
tempo das fases que couberam na janela limpa. Cada fase pergunta ao portão
antes e depois de si mesma, e o gerador publica só as que passaram nos dois.

**Quanto cada índice custa.** A bancada mede o custo da **chave conferida**
isolando-a (os dois braços têm os mesmos cinco índices), mas não isola índice
nenhum. Para isso seria preciso um braço por índice, e ele não existe.
