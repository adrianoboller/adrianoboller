# Fonte do dossiê

`dossie-phxsql-0.18.html` é o fonte da página publicada em:

**https://claude.ai/code/artifact/5c14044e-0dc5-4832-b015-224ab1e40033**

O nome muda a cada refação: era `dossie-phxsql.html`, virou `-0.15` e agora é
`-0.18`. **Só existe um por vez** — o anterior sai do repositório no mesmo
commit, para que ninguém atualize o errado. Todos os scripts aceitam o caminho
do HTML como argumento, então trocar o nome de novo não exige editá-los.

Ele mora aqui, e não só na máquina de quem publicou, para que qualquer sessão
futura consiga atualizá-lo. Sem isto, a regra de manter o dossiê em dia seria
impossível de cumprir depois que o diretório temporário sumisse.

## Como atualizar

1. Edite o `dossie-phxsql-*.html` da pasta — **só existe um**, e é essa regra
   que faz os geradores o acharem sozinhos.
2. **Rode os geradores** (abaixo — são **quinze**, contados do `PLANO` do
   `portao-dos-geradores.py` em 23/09/2026, não script por script: a lista que
   o portão já mantém é a lista, e uma segunda contagem à mão é a que diverge).
   Nenhum número visível se digita.
3. Publique **passando a URL acima**, para cair na mesma página em vez de criar
   uma nova.

## Os quinze geradores, e o que cada um regrava

São **quinze** os scripts de `docs/dossie/*.py` que escrevem número ou texto
numa página publicada — o dossiê principal e as satélites (pedidos, testes,
gráficos, status e, desde 23/09/2026, **o console em imagens**). Os outros
**quatro** arquivos `.py` da pasta, `dossie_da_pasta.py` (só acha o arquivo, por
varredura), `embutir-fontes.py` (embute fontes numa cópia offline para o PDF),
`portao-dos-geradores.py` (a catraca) e `prova-do-leitor-de-pedidos.py` (a prova
real do leitor do `PENDENCIAS.md`, abaixo), não escrevem número nenhum e não
entram na conta. Quinze mais quatro são os dezenove `.py` da pasta — e o quinze
sai do `PLANO`, por `python3 docs/dossie/portao-dos-geradores.py --lista`.

A subpasta `migracoes/` **não entra na conta**, e é de propósito: ela guarda os
scripts de mudança de uma vez só — hoje o `411-secoes-para-a-oitava-pagina.py`,
que diz linha a linha o que saiu do dossiê e quantos bytes cada corte valeu.
Eles não escrevem número visível (escreveram uma vez, e acabou) e **param se
rodados de novo**, porque migração que roda duas vezes é migração que ninguém
sabe se já rodou. Estão versionados porque script que resolveu algo não pode
morrer com a sessão. A lista de comandos e a tabela «script → o que ele
escreve» estão em «O que conferir antes de publicar», abaixo.

Fora desta pasta há mais geradores de página, e por isso **fora desta conta**:
os dois de `docs/pmo/` (o board e o painel PMO) e os **dois** de
`docs/status/` — a **sétima página**, o status do projeto, cujo comando é
`./status-html.sh`, e o `riscos.py`, que escreve as seções de riscos e de
dívida técnica **dentro** dela (pedido 264; ele roda em segundos e o
`status-html.sh` já o chama por dentro). A receita das duas está em
`docs/status/LEIA-ME.md`. A lista **completa** é o `PLANO`
do `portao-dos-geradores.py`, que `python3 docs/dossie/portao-dos-geradores.py
--lista` imprime. E a sétima roda **depois de todas**: ela publica o tamanho
das outras páginas, então quem rodar um gerador depois dela a deixa velha.

### O degrau 1 não é formalidade — medido

Numa rodada de agosto de 2026, a página publicada tinha **33 seções e 2,4 MB**
(com 21 imagens embutidas) e este arquivo tinha **24 seções e 383 KB**. Nove
seções — multitela, as grades, telemetria e Profiler, o console em imagens, a
cifra do dado pessoal, a restauração, SQL e gatilhos, ODBC/MCP e os seis
idiomas — existiam **só na página**, publicadas de outros *worktrees* que
ainda não tinham voltado para este branch.

Quem seguisse «edite e publique» teria **apagado as nove**, com a melhor das
intenções e sem ver nada errado no diff local. É a mesma família do
`replicas_autorizadas` e do binário velho: **o que não se confere contra a
fonte de verdade envelhece calado** — e aqui a fonte de verdade, na hora de
publicar, é a página que está no ar, não o arquivo que está na sua mão.

Se a página estiver à frente: **não publique deste branch**. Diga que ela está
à frente e em quantas seções, e deixe a integração dos branches reconciliar o
fonte. Publicar um merge montado em `/tmp` é pior — vira uma página que
nenhum repositório reproduz.

### O sexto entrou em 04/09, e o motivo dele é uma cicatriz

O `tetos-da-trava.py` lê as **corridas cruas** do medidor de concorrência e
escreve os quatro tetos na §35. Ele não podia ser uma tabela escrita à mão, e a
razão custou uma hora nesta casa: naquele mesmo dia uma bancada mediu **quatro
vezes a mesma coisa** por mandar um campo que o servidor não lê, e o número
publicado ficou errado até outro medidor discordar dele.

Ele só lê arquivo com **`CERTO`** no nome. As corridas invalidadas continuam
guardadas ao lado — apagá-las perderia a série e a lição —, e é por isso que o
nome, e não a data, decide o que entra. Se não achar corrida nenhuma, ele
**reprova em vez de devolver vazio**: gerador que emite nada quando a fonte
sumiu é a mesma doença do conferidor que diz «limpo» sem ter conferido.

## O que conferir antes de publicar

**Os números do painel e do rodapé não se digitam mais.** Saem de

```bash
python3 docs/dossie/pagina-do-console.py     # PRIMEIRO -- a casca da oitava página
python3 docs/dossie/numeros-do-projeto.py
python3 docs/dossie/numeros-da-bancada.py
python3 docs/dossie/pagina-dos-pedidos.py
python3 docs/dossie/cobertura-por-area.py
python3 docs/dossie/capturas-no-dossie.py
python3 docs/dossie/tetos-da-trava.py
python3 docs/dossie/comparativo-no-dossie.py
python3 docs/dossie/fluxo-do-motor.py
python3 docs/dossie/trio-de-motores.py
python3 docs/dossie/perguntas-no-dossie.py   # a seção 36, das respostas em docs/pdf/respostas/
python3 docs/dossie/numerar-figuras.py       # POR ÚLTIMO -- alcança as DUAS páginas
```

**O `pagina-do-console.py` vem primeiro, e isso não é estilo.** Cinco dos
geradores abaixo escrevem *dentro* da oitava página, entre marcas que só
existem porque ele as pôs lá. Numa árvore onde ela ainda não existisse,
qualquer um deles pararia com «a página do console não existe» — que é o certo,
e é por isso que ele abre a fila em vez de fechá-la.

**Confira o código de saída de cada um.** O `numeros-do-projeto.py` chama o
`cargo`, e numa árvore compartilhada ele pode sair diferente de zero porque
outra frente segura o `flock /tmp/phx-cargo.lock` — aconteceu na revisão de
07/09/2026. Ele **não** grava número errado nesse caso; ele simplesmente não
grava, e o painel fica com o de ontem anunciando sucesso pelo silêncio. Rodar o
laço com `|| echo FALHOU: $g` custa nada e é a diferença entre saber e supor.

### O portão dos geradores fecha o laço — rode-o por último

Conferir o código de saída de cada gerador não basta: a rodada pode **esquecer
de rodar um** por inteiro, e aí o painel dele fica com o número de ontem sem
ninguém ver falha nenhuma. O portão dos geradores é a catraca que reprova esse
estado:

```bash
python3 docs/dossie/portao-dos-geradores.py     # sai != 0 se algum ficou velho
```

Para cada gerador ele responde a uma pergunta — *re-rodar mudaria algum número
visível?* — rodando o gerador de verdade e comparando o alvo antes/depois, e é
**read-only** (devolve os bytes originais). Derivado que mudaria, ou gerador que
falha, é VERMELHO, com o nome do gerador e do arquivo. O `numeros-do-projeto.py`
sai como NOTA (chama `cargo`, e o portão não martela o build); rode-o à mão. A
lei da catraca, o defeito que a motivou e os três modos de comparação estão em
`docs/CATRACAS.md` §9. Depois de consertar um, `--so <nome>` reconfere só ele.

### E o portão prova a GUARDA, não só o resultado

Rodar a prova do leitor do `PENDENCIAS.md` entrou no portão em 18/09/2026, e o
motivo é o defeito que o obrigou:

```bash
python3 docs/dossie/prova-do-leitor-de-pedidos.py   # sai != 0 se a guarda parou de guardar
```

Naquele dia o `pagina-dos-pedidos.py` imprimiu **369 pedidos** quando o arquivo
tinha **380** linhas de pedido — e imprimiu junto três linhas de êxito. Onze
linhas não casavam a forma: seis sem o pipe de fechamento, e cinco porque o
próprio texto de fecho («**FECHADO em…**») entrou como **quinta coluna**. O
leitor as pulava em silêncio.

A guarda do pedido 150 já existia e não pegou nenhuma: ela cobre o **símbolo**
de estado, e nestas onze o símbolo estava certo. **Guarda se escreve contra o
efeito, não contra o motivo** — o efeito é um só, «pedido que existe no arquivo
e não existe na página», e há quatro caminhos até ele. A prova cataloga os
quatro, cada um com o pedido que o pagou.

E o portão passou o tempo todo **verde** enquanto isso acontecia, porque ele
conferia se o derivado estava em dia — e estava, com um leitor que contava
menos. Catraca que só olha o resultado não vê a guarda que parou de guardar.

### E o portão prova a IDADE do número da versão, não só o dos derivados

`docs/versao/portao-da-versao.py` entrou em 23/09/2026, e o defeito que o
motivou não era teórico nem pequeno: `version = "0.18.0"` no `Cargo.toml` foi
selado em 29/08/2026 e a árvore andou **887 commits** por cima sem que nada
acusasse. O `empacotar.sh` já tinha `confere_versoes()`, mas ele só confere
que `Cargo.toml`/`Cargo.lock`/`MANUAL.txt`/`CHANGELOG.md` **concordam** entre
si — quatro cópias do mesmo número velho passam por ele de braços abertos,
porque concordar não é o mesmo que **descrever** o que existe. É o padrão
«gerador certo chamado pela metade», um nível acima do de sempre: o gerador do
selo da capa estava certo, e a fonte dele — a linha do `Cargo.toml` — envelhecia
calada.

```bash
python3 docs/versao/portao-da-versao.py   # sai != 0 se a versao selada envelheceu
```

A régua é a distância, em commits, entre o commit que selou a versão atual —
achado por `git log -S 'version = "X.Y.Z"' -- Cargo.toml`, nunca digitado — e
o HEAD, contra um teto medido em 23/09/2026: o **maior intervalo** entre duas
selagens em toda a história do projeto antes desta rodada (**51**,
`c4af6d0..baff46e`). 887 é 17,4× esse máximo. É uma catraca, e catraca só
desce: subir este teto por estilo não é permitido, quem quiser um teto maior
aposenta esta constante e nasce outra, nomeada e medida no dia. Tempo de
parede e linhas alteradas foram pesados e descartados — o próprio docstring do
script diz por quê, com o mesmo formato do `TETO_TABELA_NA_MAO` do QA.

Ele roda **aqui** (não só no `empacotar.sh`) para pegar o envelhecimento antes
de alguém tentar empacotar, na mesma rodada que já confere o resto do que se
publica; e roda também dentro de `confere_versoes()`, guardado pelo mesmo
`command -v python3` do `docs/versao/conferir.py`, para que o próprio
empacotador se recuse quando o número não descreve mais a árvore.

**Sem argumento nenhum**, e isso é conserto de 07/09/2026, não estilo. O nome
do dossiê some da receita porque ele muda a cada refação, e quem o acha é o
`dossie_da_pasta.py` — **um dono só**, varrendo `dossie-phxsql-*.html` na
pasta, com a regra «só existe um por vez» virando portão: zero é parada, dois
é parada, nunca um palpite sobre qual atualizar. Ainda dá para passar o caminho
por argumento; o que mudou é que **não passar deixou de significar «faça
menos»**.

O defeito que isso fechou não era teórico. Três painéis do dossiê estavam
parados, e nenhum número tinha sido digitado por ninguém:

| painel | dizia | era |
|---|---:|---:|
| pedidos, ao todo | 198 | **203** |
| testes na maior área | 428 | **451** |
| replicação, linhas/s no master | 26.762 | **37.810** |

A causa é uma só, e vale mais que os três números: o `pagina-dos-pedidos.py`
chamado **nu** gravava a página e a contagem e **pulava o dossiê calado** — o
alvo do painel só existia se viesse por argumento. O `cobertura-por-area.py`
tinha o mesmo laço e a mesma falta; o `numeros-da-bancada.py` e mais três
tinham padrão, mas era o **nome digitado**, que morre na próxima refação. E o
`tetos-da-trava.py` era o único que exigia argumento — fazer diferente dos oito
irmãos é a armadilha, porque quem repete a receita nua deixa aquele bloco para
trás.

*Gerador que faz menos do que o nome dele promete e não diz é a mesma doença do
número digitado à mão: envelhece calado.*

| script | blocos que ele escreve |
|---|---|
| `numeros-do-projeto.py` | `<title>`, `selo:`, `projeto:` (o painel da capa), `rodape:` e `idiomas:` |
| `pagina-do-console.py` | a **oitava página** inteira (casca, estilo e prosa) — e **preserva** os blocos dos outros cinco; regrava também os três ponteiros `console:a1/a2/a3` do dossiê, que são o link de volta |
| `numeros-da-bancada.py` | `bancada:`, `bancada:tabela:` e `bancada:diagnostico:` **na oitava página**, e `replicacao:` no dossiê (a §10 não se mudou) |
| `pagina-dos-pedidos.py` | `pedidos:` no dossiê, as páginas `pedidos-*.html` por faixa (número e nomes saem do corte por tamanho, não são fixos — pedido 403), e a contagem de volta no `PENDENCIAS.md` |
| `cobertura-por-area.py` | `cobertura:` **na oitava página**, e as tabelas do `docs/TESTES.md` |
| `capturas-no-dossie.py` | `capturas:` **na oitava página** — as vinte telas, como *data URI* |
| `tetos-da-trava.py` | `tetos:` **na oitava página** — os quatro tetos de concorrência, lidos das corridas cruas em `bancada/concorrencia/corridas/` |
| `comparativo-no-dossie.py` | `comparativo:` — a tabela do que ainda falta aqui (§33) e as as duas figuras do medidor, lidas de `bancada/comparativo/` e `bancada/cobertura-da-tela/`; grava também os dois `.svg` avulsos |
| `trio-de-motores.py` | `trio:` **na oitava página** — os três motores a um milhão de linhas, do `bancada/comparacao/um-milhao.json`. **Não redesenha**: o SVG é do `bancada/comparacao/grafico.py`, e ele PARA se o desenho for mais velho que a medição. Ficou **fora desta receita** até 07/09/2026, e quem a seguia nunca o rodava |
| `fluxo-do-motor.py` | `fluxo-motor:` (§9) e `workflow-motor:` (§31) — o caminho de um pedido e o ciclo de operação; as **listas saem do código** e ele PARA quando divergem |
| `perguntas-no-dossie.py` | `perguntas:` — a seção 36, a resposta curta de cada uma das 26 perguntas do dono, lida de `docs/pdf/respostas/*.md` (o MESMO material do PDF, para as duas cópias não divergirem). Reaproveita o conversor de Markdown do `docs/pdf/gerar.py` — um conversor, não dois |
| `numerar-figuras.py` | renumera **todas** as legendas `Figura N` na ordem de **cada** documento — são dois desde 23/09/2026, e chamada nua alcança os dois. Roda **por último** |

`--so-medir` mostra sem gravar; `--sem-testes` no primeiro pula o `cargo test`,
que demora. Use só quando o que mudou não foi código.

**Nenhum desses números se confere à mão, e é esse o ponto.** A receita de cada
um está no cabeçalho do script que o produz, e mexer numa exige mexer na outra.
Duas contagens da mesma coisa é o jeito clássico de a vitrine e o produto
discordarem — foi assim que o painel da replicação chegou a dizer 28.914/4.357
enquanto a seção da bancada, no mesmo documento, mostrava 34.048/17.450.

### O sétimo entrou em 07/09, e o motivo é o mesmo do sexto por outro lado

O `comparativo-no-dossie.py` escreve a tabela do **que ainda falta aqui**, na
§33. Ela era prosa inteira, e prosa é onde ausência envelhece: o
`docs/HFSQL.md` publicou **seis** vereditos de ausência errados, e o sexto foi
a *trava por linha* — existe desde que o `travas.rs` entrou, e a página dizia
que não.

**Ausência não se reconfere sozinha.** Um número errado alguém desconfia ao
bater o olho; um «não há» ninguém revisita, porque não há o que olhar. Por isso
esta tabela sai de um **medidor** que pergunta a quatro motores vivos, e não de
uma leitura — e cada célula carrega a procedência, porque `citado` e `medido`
não valem a mesma coisa.

### As figuras do sétimo saem em DOIS destinos, e se provam nos dois

O `comparativo-no-dossie.py` desenha duas figuras — o fluxograma do medidor e
o workflow da rodada — e as entrega embutidas no dossiê **e** como
`fig-fluxo-do-medidor.svg` e `fig-workflow-da-rodada.svg`. O mesmo texto, e
**não é o mesmo arquivo**: o solto é mais estrito, e as três diferenças não dão
erro nenhum na tela.

| dentro do HTML | como arquivo `.svg` |
|---|---|
| o `xmlns` é assumido | **obrigatório** — sem ele, `naturalWidth` = 0 e nenhum aviso |
| `&middot;` e `&mdash;` valem | só as **cinco** entidades do XML; uma nomeada derruba o arquivo inteiro |
| `<style>` pode vir antes | a raiz tem de ser o `<svg>` |

A função `solto()` cuida das três. E o número da figura **não se digita**: sai
da contagem das legendas anteriores no próprio arquivo, senão apontaria para a
figura errada no dia em que alguém acrescentasse uma acima — e legenda errada
não quebra nada, então ninguém veria.

**E o portão que confere isso mede o EFEITO, não a estrutura**: «carrega como
`<img>` e tem tamanho». Duas réguas anteriores reprovaram as figuras boas
porque olhavam `documentElement` e `getBBox`, e o Chromium embrulha todo `.svg`
de `file://` num documento sintético — elas mediam o embrulho.
`docs/cognicao/cognicao_o-svg-embutido-nao-e-o-svg-solto_20260907_0345.md`.

### O oitavo lê o CÓDIGO, e o nono conserta o que inserir no meio quebrou

O `fluxo-do-motor.py` desenha o caminho de um pedido (§9) e o ciclo de operação
(§31). As duas listas que ele usa **saem do fonte**: os portões, dos
comentários que o `servidor.rs` numera (`// Portao 0`, `1`, `2`, `2a`…), e os
passos de gravação, das chamadas que o `inserir` do `table.rs` faz. Ele **para**
quando os dois divergem, nos dois sentidos — portão no código sem rótulo aqui
(o desenho mentiria por omissão) e rótulo aqui sem portão no código (a «chave
morta» da fábrica de idiomas, que é pior: quem lê acha que há proteção que não
há).

**E o portão já pagou por si na primeira corrida:** ele reprovou dois passos que
eu tinha escrito de memória. `proximo_rowid` não existe — são `numerar_linha` e
`numerar` — e eu havia **esquecido o `montar_payload`**, que é onde o `.bin` e o
`.memo` são gravados. O desenho teria publicado o caminho de gravação sem dois
dos sete arquivos.

O `numerar-figuras.py` existe por consequência. Enquanto figura só entrava no
fim, o número digitado batia por sorte; duas no **meio** viraram 16 legendas
erradas de uma vez. E ele achou uma desordem que já existia: no documento as
figuras vinham na ordem 14, 17, 18, 15, 19, 20, 21, 16. Legenda errada **não
quebra nada** — a página abre, o desenho aparece —, e por isso ninguém confere.

### A receita da interface saiu daqui, e foi para o `http.rs`

Ela já esteve escrita aqui como «os TRÊS arquivos que o `http.rs` embute», e
envelheceu calada: o `http.rs` passou a embutir **nove**, e o rodapé publicava
780 KiB quando a interface tinha 1.032. Hoje a lista sai do próprio `http.rs`,
lendo os `include_str!("../ui/…")` fora do `#[cfg(test)]`. **Lista de arquivos é
número como qualquer outro.**

## O dossiê em PDF

```bash
python3 docs/dossie/embutir-fontes.py docs/dossie/dossie-phxsql-*.html /tmp/com-fontes.html
node    docs/dossie/pdf-do-dossie.mjs /tmp/com-fontes.html dossie-phxsql.pdf
```

O primeiro baixa as **26 faces** do Google Fonts e as põe como `data:` numa
cópia — a rede do contêiner engole `fonts.googleapis.com`, e sem isso o PDF
nasce em fonte de *fallback* **sem erro nenhum**. O segundo imprime pela folha
`@media print` que a própria página traz; ele não inventa estilo.

**O PDF do dossiê não tem mais a galeria, e isso é consequência, não defeito.**
Desde 23/09/2026 as capturas e a marca da capa saíram dele: o mesmo comando
apontado para `console-em-imagens.html` é que produz o PDF com as vinte telas.

    python3 docs/dossie/embutir-fontes.py docs/dossie/console-em-imagens.html /tmp/c.html
    node    docs/dossie/pdf-do-dossie.mjs /tmp/c.html console.pdf

**A armadilha que custou a primeira corrida:** as 20 capturas são
`loading="lazy"` e o `page.pdf()` **não rola a página**. O PDF saiu com **uma**
imagem em 67 páginas — a marca da capa, a única sem `lazy` — com o texto todo,
as 67 páginas, e nenhum aviso. Hoje o script troca `lazy` por `eager`, espera
cada `<img>` e **conta**, e a conta é da própria página: no dossiê hoje dá
0 de 0 e na página do console, 21 de 21. **Atenção ao 0 de 0**: a guarda compara
`prontas` com `pedidas`, então uma página que perdeu as imagens passa por ela —
quem imprime o console olha o número impresso, não só o código de saída.

**E a que ensina sobre medir:** `document.fonts.check()` responde `true` para o
*fallback* — ele diz «consigo desenhar isto», não «a fonte chegou». Nem
`document.fonts.size` serve: ele conta as regras declaradas. A medida que não
mente é abrir o PDF pronto e listar as fontes (`get_fonts` do PyMuPDF).

**Limitação conhecida, medida:** **Exo 2 e Source Serif 4 não são embutidas**
pelo Chromium — saem substituídas por DejaVu Sans e Liberation/FreeSerif. O
IBM Plex Mono, sim. Medido em caso mínimo e isolado, com as faces já embutidas
como `data:`, então **não é a rede**. Texto, cor, desenho, tabelas e as vinte
capturas saem certos; a tipografia dos títulos e do corpo não é a da marca.

## As capturas

Elas moram em `capturas/`, já reduzidas, e entram no HTML como *data URI* —
dentro, e não ao lado: a página publicada é um arquivo só, e a política de
conteúdo do visualizador bloqueia imagem de qualquer outra origem. Ao lado, ela
ficaria com vinte quadros quebrados e nenhum erro visível. **Desde 23/09/2026 o
destino é a oitava página**, `console-em-imagens.html`, e não mais o dossiê.

Para refazê-las:

```bash
cargo build --release -p phxsql-server --bin phxsqld     # a página é include_str!
node    docs/dossie/capturar-dossie.mjs . /tmp/brutas
python3 docs/dossie/capturas-no-dossie.py --preparar /tmp/brutas
python3 docs/dossie/capturas-no-dossie.py
find . -name '*.rs' -not -path './target/*' | xargs cat | wc -l    # linhas de Rust
cargo test --workspace 2>&1 | grep '^test result' \
  | awk '{s+=$4} END {print s}'                                    # testes
$(( $(grep -c '^\[\[package\]\]' Cargo.lock) - 4 ))                # dependências externas
cat docs/*.md README.md CHANGELOG.md MANUAL.txt \
    bancada/LEIA-ME.md bancada/replicacao/LEIA-ME.md \
    bancada/replicacao/docker/LEIA-ME.md \
    bancada/carga/LEIA-ME.md \
    marca/LEIA-ME.md docs/dossie/LEIA-ME.md \
  | wc -l                                                          # linhas de doc
stat -c%s crates/phxsql-server/ui/index.html \
          crates/phxsql-server/ui/grid/phx-grid.{css,js} \
  | paste -sd+ | bc                                                # bytes de interface
```

O `capturar-dossie.mjs` sobe um `phxsqld` só dele na faixa **6700/6701**,
popula três tabelas ligadas por chave estrangeira mais um segundo banco, faz
movimento para os gráficos terem o que mostrar, e derruba **pelo PID** — nunca
`pkill -f`, que mataria o servidor do vizinho.

O peso não se digita aqui: o `capturas-no-dossie.py` **imprime** os três
números ao gravar — KiB de PNG, KiB em base64 e o tamanho final da página.
Medido em 23/09/2026: **1.534 KiB de PNG → 2.046 KiB em base64**, e a oitava
página fecha em **2.257.390 bytes**. É esse número, e não uma impressão, que
tirou a galeria do dossiê.

PNG quantizado (160 cores), e não JPEG: a captura é quase toda texto e linha
fina, e o JPEG põe halo em volta de cada letra — medido nas vinte, os dois
pesam praticamente o mesmo e um deles fica com o texto limpo. A largura é
1.200 px, o dobro da que a página usa, para servir a uma tela de duas vezes a
densidade; a do multitela vai a 2.000 porque ela é um panorama de quatro telas
e a 1.200 o texto de dentro vira borrão.

## A OITAVA página: o console em imagens — pedido 411 (23/09/2026)

`console-em-imagens.html` é onde a **seção 18** do dossiê (as vinte capturas),
a **seção 32** (a bancada de dez milhões de linhas e os três motores a um
milhão) e os **dois painéis medidos da seção 35** (os quatro tetos da trava e
os testes por área) passaram a viver.

**Ela nasce SEM URL, e isso é o certo** — é um artefato novo, nunca publicado.
Enquanto a chave estiver vazia em `URLS_PUBLICADAS`, no topo do
`pagina-do-console.py`, a navegação e os três ponteiros do dossiê caem no
**nome do arquivo**, em vez de fingir que alguma URL antiga serve. Publique-a
**sem** passar URL; depois preencha a chave `console-em-imagens.html` ali —
é o **único** lugar do script onde ela entra, e é o mesmo ponto que acerta os
três `<a>` de volta dentro do dossiê, de uma vez.

### O número que a fez nascer

| o que | bytes | % do dossiê |
|---|---:|---:|
| dossiê antes | 2.703.573 | 100% |
| §18, o console em imagens | 2.106.613 | 77,9% |
| as 21 imagens `data:` do documento | 2.171.386 | 80,3% |
| **dossiê depois** | **459.395** | **17,0%** |

O teto é o mesmo do pedido 403: **~450 KiB de página publicada é uma janela de
contexto inteira só para republicar**, porque o guarda exige reler a versão
publicada antes de aceitar a nova. As **21 imagens** do documento somavam
**4,71× o teto**, e só a galeria — as vinte capturas, sem a marca da capa —
dava **4,55×**. Não havia prosa a cortar que resolvesse, porque *o peso é a
galeria*. A rota de
reduzir as capturas morreu medida antes desta: rendia **2,4%** onde precisava
render **83,0%**, erro de **35×**.

### O que saiu, e para onde — nada sumiu

| o que saiu do dossiê | foi para |
|---|---|
| as 20 capturas, com as legendas e os dois temas | oitava página, §1 |
| a bancada inteira: figura, tabela, 13 subseções, diagnóstico, os três motores | oitava página, §2 |
| os quatro tetos da trava e os testes por área, e o roteiro em três lugares | oitava página, §3 |
| o símbolo da marca da capa (440 px, 75.394 B em base64) | oitava página, cabeçalho |
| o CSS da galeria e o da placa da marca | `pagina-do-console.py` |
| a prosa digitada da §35 que dizia «as quatro parciais» com o painel gerado mostrando quatorze | **apagada** — era o número digitado contradizendo o gerador ao lado |

No dossiê, as §18, §32 e §35 continuam existindo: viraram **ponteiros**, com o
link de volta gerado. O painel dos **pedidos** ficou na §35, porque ele é o
resumo de capa do estado do projeto.

### A marca saiu da capa, e não por preferência

**A marca manda** — por isso ela não foi apagada sem medida. As três saídas,
na ordem em que a regra manda avaliá-las:

| saída | número medido | veredito |
|---|---:|---|
| (a) o mesmo símbolo em resolução menor | o dossiê residual já está a **1.396 B** do teto; o menor derivado oficial (`phxsql-icone-32.png`, 40×34) custa **1.990 B** em base64, e o 224×133 recomprimido a 64 cores custa **15.576 B** | **não cabe** — e um símbolo de 40 px numa placa de 440 seria a marca mostrada mal |
| (b) o símbolo como arquivo de apoio, ao lado da página | não é o orçamento que decide: a política de conteúdo do visualizador **bloqueia imagem de qualquer outra origem** (já medido nesta casa, é o motivo de as capturas serem *data URI*) | **morre antes da conta** — a imagem não carregaria, e sem erro visível |
| (c) tirar da capa | — | **escolhida**, com a ressalva abaixo |

E a ressalva é o que salva a regra: **ela mudou de casa, como as seções.** O
símbolo vive inteiro, em 440 px, no cabeçalho da oitava página, onde é 3,3% do
arquivo. A capa do dossiê continua falando pela marca no que não custa bytes —
Exo 2, o `#010418` do tema escuro, o vermelhão do acento e a assinatura
*Built to store. Engineered to scale.*

### A folga é apertada, e o próximo degrau está medido

O dossiê fechou em **459.395 bytes** contra o teto de 460.800: **1.405 bytes,
0,30%**. Isso passa hoje e não passa para sempre — a cada rodada entra prosa.
O maior bloco restante está medido: a **§36, as perguntas do dono, com 42.525
bytes**, e ela é uma cópia do mesmo material de `docs/pdf/respostas/*.md` que o
PDF publica. Movê-la daria **30× mais folga** que a de hoje, e traria a marca de
volta à capa com sobra. É decisão do dono, não do gerador.

### E a oitava página é maior que o teto, de propósito

Ela fechou em **2.257.390 bytes** — **4,90× o teto de republicação**. Isso é
conhecido e aceito: ela publica barato **na primeira vez**, e o preço aparece na
**segunda**, quando o guarda exigir reler a versão publicada. Quem for
republicá-la vai precisar decidir entre parti-la por tamanho (como as páginas de
pedidos) ou reduzir as capturas. **Não descubra isso na hora** — está escrito
aqui porque o número já foi medido.

## A outra página: os pedidos — partida por TAMANHO, desde o pedido 403 (23/09/2026)

`pedidos.html` era a relação de tudo que o Adriano pediu, com o estado de cada
item, publicada em **https://claude.ai/code/artifact/d6c8f13c-e4a2-444e-9f19-0e047e230352**.
Essa página **parou de ser gerada** — o `pagina-dos-pedidos.py` já apaga o
arquivo do disco sozinho se sobrar — porque ela cresceu até **1.304.729
bytes** (781 linhas), e o guarda da republicação exige reler a versão
publicada **inteira** antes de aceitar a nova: ~580.000 fichas, mais de uma
janela de contexto inteira só para republicar. Decisão do dono, medida:
**partir em páginas contíguas por número de pedido**.

**A PRIMEIRA versão deste conserto cortava em blocos fixos de 100 pedidos**
(1–100, 101–200, …), e o integrador (papel A) mediu que a última nascia com
**562,5 KiB — já acima do teto de 450 KiB**, porque pedido recente pesa muito
mais que pedido antigo (60 → 287 → 423 → 562 KiB nas quatro faixas fixas; o
403 e o 404 estão entre os mais longos do arquivo). Corte por número redondo
não é corte por tamanho — é o pedido 404 de novo, por outro lado: um número
cravado no código («100») envelhece calado assim que a premissa que o
justificava muda.

A cura, em `escolher_faixas()`: **os cortes saem do tamanho ACUMULADO,
medido a cada corrida** (alvo de 300 KiB por página, bem abaixo do teto de
republicação de 450 KiB), "redondo onde der" — o corte prefere cair num
múltiplo de 10 quando isso não estoura o alvo, mas nunca ao custo de exceder
o teto. **O número de páginas deixou de ser fixo**: nesta rodada (23/09/2026)
deu cinco, não quatro.

| faixa | arquivo | pedidos | KiB medidos | folga até 450 KiB |
|---|---|---:|---:|---:|
| 1–190 | `docs/dossie/pedidos-001-190.html` | 190 | 294,8 | 155,2 |
| 191–260 | `docs/dossie/pedidos-191-260.html` | 70 | 264,4 | 185,6 |
| 261–310 | `docs/dossie/pedidos-261-310.html` | 50 | 266,1 | 183,9 |
| 311–350 | `docs/dossie/pedidos-311-350.html` | 40 | 265,9 | 184,1 |
| 351–410 | `docs/dossie/pedidos-351-410.html` | 60 | 278,6 | 171,4 |
| 411 em diante | `docs/dossie/pedidos-411-mais.html` | 12 | 54,5 | 395,5 |

Medido em 23/09/2026, com **422 pedidos**. Foram **cinco** páginas na corrida
da manhã e são **seis** agora: os pedidos 414–422 deslocaram um corte, a
`pedidos-351-mais.html` deixou de ser gerada (o script a apagou sozinho) e
nasceram `pedidos-351-410.html` e `pedidos-411-mais.html` — **as duas sem URL**,
como manda a regra. As quatro de cima mantiveram o nome, então a URL delas
continua valendo.

A última faixa é **aberta** de propósito — nomeá-la com um teto fixo mentiria
assim que a faixa seguinte nascesse (o maior pedido nesta rodada é o 404, e
o corte de hoje já a fecha em 351+), então o nome nunca cita um número que a
rodada seguinte furaria calado. Como ela é a única que **cresce** a cada
rodada (as faixas de baixo fecham para sempre — pedido antigo não muda de
número), o alvo de 300 KiB por corte é o que lhe dá folga: ela nasce a
~183 KiB do teto de republicação, não a alguns KiB dele.

**Os nomes dos arquivos PODEM MUDAR entre rodadas** — se o corpo dos pedidos
crescer ou encolher o bastante para deslocar um corte, o script escreve
arquivos novos e **apaga os órfãos** da rodada anterior sozinho (nunca deixa
um `pedidos-XXX-YYY.html` velho apontando para uma faixa que não existe
mais). O portão dos geradores confere isso pelo CONJUNTO de arquivos, não por
uma lista de nomes fixa — `PEDIDOS_FAIXAS` em `portao-dos-geradores.py`, que
varre `docs/dossie/pedidos-*.html` dos dois lados da corrida.

**Cada página é um artefato NOVO na primeira vez que nasce** — sem URL
anterior, sem guarda de leitura. Publique cada uma **sem** passar URL (ela
nasce nova); depois de publicadas, preencha as URLs em
`docs/dossie/pagina-dos-pedidos.py`, no dicionário `URLS_PUBLICADAS` — é o
**único** lugar do script onde elas entram, pelo NOME exato que o script
escolheu (impresso no stdout a cada corrida), e é o que liga a navegação
entre as páginas por link de verdade em vez do nome do arquivo local. Se um
corte se deslocar numa rodada futura, a chave antiga simplesmente para de
bater — a navegação cai de volta no nome do arquivo até alguém atualizar a
tabela; o script não finge que a URL antiga ainda serve.

Nenhuma se edita — saem do `pagina-dos-pedidos.py`, que lê o
`docs/PENDENCIAS.md` e conta os três estados sozinho. A fonte da verdade é o
`.md`; mexeu lá, rode isto.

## A página do status dos dez recursos

`status.html` é a tabela A–J que o Adriano pediu em 08/09/2026 — nota de 0 a
10, o que existe e o que falta em cada recurso —, publicada em:

**https://claude.ai/code/artifact/51330b6a-7c5c-4f8f-831a-93a9fb7cba9c**

Ela **não se edita**: `python3 docs/dossie/pagina-de-status.py` a gera do
`docs/STATUS.md`, que é a fonte e **se edita** — a avaliação, datada, com as
fontes nomeadas em cada linha. Rode **depois** do `numeros-do-projeto.py`,
porque o painel lê o `CAPABILITIES.json`; a corrida da rodada faz isso na
ordem certa.

Dois tipos de número, separados de propósito: a **nota** é avaliação (uma
leitura do código e dos testes, na data dita) e o **painel** é medido — do
`CAPABILITIES.json` e dos `resultados.json` de gestão, replicação e cluster,
cada um com a data em que foi medido. O cluster não grava a data no próprio
resultado, e a página diz «data do arquivo», como a dos testes; bancada sem
arquivo aparece como NÃO MEDIDA. A régua carrega o tipo na **forma** da barra
(cheia = construído, meia = parcial, hachurada = recusa medida, só contorno =
promessa), e o número fica depois do traço.

`olhar.mjs` faz uma captura local para a olhada antes de publicar. O
`playwright` entra pelo caminho absoluto, como no `capturar-dossie.mjs`: não há
`node_modules` no repositório, e o pacote global não se resolve pelo nome — o
Node cai com `ERR_MODULE_NOT_FOUND` e, cortado no `tail`, parece que só
imprimiu a versão dele.

## O que só apareceu abrindo no navegador

- **`thead` grudento dentro de `overflow-x:auto` cai por cima da primeira
  linha.** O `.rolo` vira contexto de rolagem próprio, e o `position:sticky`
  passa a se medir por ele. Quem gruda é a barra de filtro.
- **Busca em português tem de achatar acento.** Sem `normalize('NFD')`, quem
  digita «indice» não acha «índice» — e a busca falha calada.
- **`1fr` num grid é `minmax(auto,1fr)`, e `auto` não desce abaixo do
  min-content do filho.** Foi o que pôs 1.700px de galeria dentro de uma janela
  de 390 e fez a página rolar de lado. A coluna única é `minmax(0,1fr)`, e a
  galeria só vira multi-coluna a partir de 900px.
- **`<details>` fechado esconde o conteúdo, e nenhum CSS o abre.** O índice
  lateral é um `<details open>` no HTML — sem JS ele fica aberto, que é uma
  lista comprida mas nunca um estado quebrado — e um `matchMedia` o fecha só
  quando a janela é estreita demais para a coluna lateral.

## O aviso de «download morto» na publicação é FALSO POSITIVO

Publicar o dossiê devolve um aviso dizendo que a página oferece um arquivo por
link de download, e que o visualizador nunca dá essa permissão. **Não é
verdade, e não se conserta.** Medido em 07/09/2026: `<a … download>` de
verdade, **zero**; `createObjectURL`, `new Blob`, `msSaveBlob`, **zero**. O
botão «baixar» abre a **caixa de impressão do navegador**, que é do navegador
e por isso abre, com «Salvar como PDF» no destino.

O que o varredor casou foram as **próprias frases** que explicam a decisão —
os dois comentários que dizem, em letras, *«nada de `<a download>`, que o
visualizador bloqueia»*. Prosa sobre a armadilha casa com a busca pela
armadilha.

E a segunda metade da lição é minha: eu contei `grep -c '<a [^>]*download'`,
recebi **2** e escrevi que havia dois links. Eram os comentários de novo. *Um
casador de texto não sabe a diferença entre fazer e falar sobre fazer* — é a
mesma razão pela qual o conferidor genérico das mensagens de erro foi recusado
com número nesta casa.

### E em 18/09/2026 ele mordeu de novo, com ALCANCE maior

A frase acima estava escrita, e eu a repeti letra por letra: contei os mesmos
**2** e concluí os mesmos dois links. Lei escrita não impede o erro; ela só o
nomeia depois.

Mas a repetição trouxe um alcance que a lei não cobria. Ao publicar a
**página dos pedidos**, o mesmo aviso apareceu — e ali não há comentário de
fonte nenhum. O que o varredor casou foi o **texto do pedido 327**, que a
página publica **como conteúdo** e que cita `createObjectURL` e
`window.claude.downloads` ao explicar o problema.

Ou seja: a regra vale para as duas metades, e a segunda é pior de achar.
**Comentário sobre a armadilha casa com a busca pela armadilha — e conteúdo
publicado sobre a armadilha também.** Uma página que documenta um defeito
passa a ser sinalizada como tendo o defeito, e o aviso nunca vai parar de
aparecer, porque a página existe justamente para falar dele.

Medido nas cinco páginas em 18/09/2026: `window.print()` no dossiê e na página
dos pedidos; `<a download>` de verdade, `createObjectURL` de verdade,
`window.claude` de verdade — **zero em todas as cinco**. Nas páginas de
testes, gráficos e status, zero ocorrências de qualquer um dos padrões, nem
como prosa. **A oitava página foi medida em 23/09/2026**, no dia em que nasceu:
`<a download>`, `createObjectURL`, `msSaveBlob`, `window.claude` e
`<script>` — **zero de cada**. Ela não tem JavaScript nenhum, e por isso também
não tem o botão «baixar»: o PDF dela sai pelo `pdf-do-dossie.mjs`, de fora.

## Três armadilhas de estilo da página

- **Nenhuma cor literal nos SVG.** Tudo sai dos tokens (`var(--reg)`,
  `var(--acento)`…), senão o diagrama some no tema escuro. Confira com
  `grep -c 'fill="#\|stroke="#' dossie-phxsql-*.html` — tem de dar zero.
- **Todo token de cor nasce no `:root` base.** Cor definida só dentro de
  `@media` ou `[data-theme]` não existe para quem está no tema "sistema".
- **Nada centraliza, e o texto tem teto.** É a §4.1 do `docs/DESIGN.md`, medida:
  texto corrido para em `74ch`, e a largura extra vira **mais coluna**, não
  linha mais comprida. Um bloco centrado num monitor duplo cai em cima da
  emenda física entre os dois.

## O «baixar» é `window.print()`, e não um link

O visualizador do artefato **bloqueia todo download que a própria página
começa** — `<a download>` inclusive, com `data:` e `blob:`, e sem erro visível.
A caixa de impressão é do navegador, então ela abre, e «Salvar como PDF» está lá
em todos eles. A folha `@media print` é própria: fundo branco, índice e botão
fora, figura, tabela e captura sem quebra no meio, galeria em duas colunas. E a
página **diz** o que o botão faz, ao lado dele.

## O PDF de um relatório: `pdf.mjs`

    node docs/dossie/pdf.mjs <html> <claro|escuro> <saida.pdf>

Serve qualquer página deste projeto que use os tokens da marca. Duas coisas
nele não são detalhe, e as duas nasceram de erro medido:

**O CSS de impressão mora no gerador, não na página.** O que se publica é a
tela; o que se imprime tem uma restrição que a tela não tem — a largura. O
`min-width` do diagrama existe para ele não espremer no celular, e numa A4
(794 px a 96 dpi) essa mesma regra **corta** o desenho dentro do
`overflow-x: auto`. Em papel não há barra de rolagem: o que transborda **some**,
e sumir calado é o pior jeito de errar.

**Ele apaga a saída antes de gerar.** Um gerador que falha deixando o arquivo
anterior em disco faz o conferidor seguinte ler o cadáver e dizer «ok» — foi
assim que um PDF velho quase saiu daqui afirmando que os consertos estavam
nele. Falha tem de aparecer como **ausência**.

**Ele declara o charset que a página não traz.** A página publicada não tem
`<meta charset>` — o embrulho do visualizador o põe ao publicar —, e aberta por
`file://` o Chromium **adivinha** a codificação: em 08/09/2026 adivinhou Latin-1
em três das quatro páginas, e o PDF saiu com «pÃ¡gina» sem erro nenhum. A
quarta saiu certa por sorte do detector, que é o pior jeito de sair certa. O
gerador imprime uma cópia com o charset na frente; o `olhar.mjs` e o
`pdf-do-dossie.mjs` fazem o mesmo, porque são os irmãos.

**E o fundo cor de tinta é da folha, não de toda página.** A regra que pinta
`html` e `body` com `--tinta` serve ao relatório de contêineres, uma folha
clara sobre fundo escuro; nas páginas que pintam o `body` com `--papel` ela
punha texto escuro sobre fundo escuro, e as quatro saíram escuras no tema
claro, com o parágrafo de abertura apagado. Hoje ela só vale onde existe
`.folha`. E quem evita quebra de página é a **linha** da tabela, não a tabela:
uma tabela maior que a página não tem como evitar, e a dos 231 pedidos pulava
inteira para a página 2, deixando a capa dois terços vazia.
