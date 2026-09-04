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

1. Edite `dossie-phxsql-0.18.html`.
2. **Rode os seis geradores** (abaixo). Nenhum número visível se digita.
3. Publique **passando a URL acima**, para cair na mesma página em vez de criar
   uma nova.

## Os seis geradores, e o que cada um regrava
1. **LEIA o artefato publicado antes de qualquer coisa** (`action: "read"` com
   a URL acima).
2. Compare com `dossie-phxsql-0.15.html` — pelo menos o número de `<h2>`.
3. Edite `dossie-phxsql-0.15.html`.
4. Publique **passando a URL acima**, para cair na mesma página em vez de criar
   uma nova.

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
python3 docs/dossie/numeros-do-projeto.py    docs/dossie/dossie-phxsql-0.18.html
python3 docs/dossie/numeros-da-bancada.py    docs/dossie/dossie-phxsql-0.18.html
python3 docs/dossie/pagina-dos-pedidos.py    docs/dossie/pedidos.html docs/dossie/dossie-phxsql-0.18.html
python3 docs/dossie/cobertura-por-area.py    docs/dossie/dossie-phxsql-0.18.html
python3 docs/dossie/capturas-no-dossie.py    docs/dossie/dossie-phxsql-0.18.html
python3 docs/dossie/tetos-da-trava.py        docs/dossie/dossie-phxsql-0.18.html
```

| script | blocos que ele escreve |
|---|---|
| `numeros-do-projeto.py` | `<title>`, `selo:`, `projeto:` (o painel da capa), `rodape:` e `idiomas:` |
| `numeros-da-bancada.py` | `bancada:`, `bancada:tabela:`, `bancada:diagnostico:` e `replicacao:` |
| `pagina-dos-pedidos.py` | `pedidos:` no dossiê, a página `pedidos.html` inteira, e a contagem de volta no `PENDENCIAS.md` |
| `cobertura-por-area.py` | `cobertura:` no dossiê, e as tabelas do `docs/TESTES.md` |
| `capturas-no-dossie.py` | `capturas:` — as vinte telas, como *data URI* |
| `tetos-da-trava.py` | `tetos:` — os quatro tetos de concorrência (§35), lidos das corridas cruas em `bancada/concorrencia/corridas/` |

`--so-medir` mostra sem gravar; `--sem-testes` no primeiro pula o `cargo test`,
que demora. Use só quando o que mudou não foi código.

**Nenhum desses números se confere à mão, e é esse o ponto.** A receita de cada
um está no cabeçalho do script que o produz, e mexer numa exige mexer na outra.
Duas contagens da mesma coisa é o jeito clássico de a vitrine e o produto
discordarem — foi assim que o painel da replicação chegou a dizer 28.914/4.357
enquanto a seção da bancada, no mesmo documento, mostrava 34.048/17.450.

### A receita da interface saiu daqui, e foi para o `http.rs`

Ela já esteve escrita aqui como «os TRÊS arquivos que o `http.rs` embute», e
envelheceu calada: o `http.rs` passou a embutir **nove**, e o rodapé publicava
780 KiB quando a interface tinha 1.032. Hoje a lista sai do próprio `http.rs`,
lendo os `include_str!("../ui/…")` fora do `#[cfg(test)]`. **Lista de arquivos é
número como qualquer outro.**

## O dossiê em PDF

```bash
python3 docs/dossie/embutir-fontes.py docs/dossie/dossie-phxsql-0.18.html /tmp/com-fontes.html
node    docs/dossie/pdf-do-dossie.mjs /tmp/com-fontes.html dossie-phxsql-0.18.pdf
```

O primeiro baixa as **26 faces** do Google Fonts e as põe como `data:` numa
cópia — a rede do contêiner engole `fonts.googleapis.com`, e sem isso o PDF
nasce em fonte de *fallback* **sem erro nenhum**. O segundo imprime pela folha
`@media print` que a própria página traz; ele não inventa estilo.

**A armadilha que custou a primeira corrida:** as 20 capturas são
`loading="lazy"` e o `page.pdf()` **não rola a página**. O PDF saiu com **uma**
imagem em 67 páginas — a marca da capa, a única sem `lazy` — com o texto todo,
as 67 páginas, e nenhum aviso. Hoje o script troca `lazy` por `eager`, espera
cada `<img>` e **conta**: 21 de 21, ou reprova.

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
ficaria com vinte quadros quebrados e nenhum erro visível.

Para refazê-las:

```bash
cargo build --release -p phxsql-server --bin phxsqld     # a página é include_str!
node    docs/dossie/capturar-dossie.mjs . /tmp/brutas
python3 docs/dossie/capturas-no-dossie.py --preparar /tmp/brutas
python3 docs/dossie/capturas-no-dossie.py docs/dossie/dossie-phxsql-0.18.html
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
números ao gravar — KiB de PNG, KiB em base64 e o tamanho final do dossiê. Na
ordem de grandeza, ~1,5 MB de PNG viram ~2 MB embutidos e o dossiê fecha perto
de 2,5 MB.

PNG quantizado (160 cores), e não JPEG: a captura é quase toda texto e linha
fina, e o JPEG põe halo em volta de cada letra — medido nas vinte, os dois
pesam praticamente o mesmo e um deles fica com o texto limpo. A largura é
1.200 px, o dobro da que a página usa, para servir a uma tela de duas vezes a
densidade; a do multitela vai a 2.000 porque ela é um panorama de quatro telas
e a 1.200 o texto de dentro vira borrão.

## A outra página: os pedidos

`pedidos.html` é a relação de tudo que o Adriano pediu, com o estado de cada
item, publicada em:

**https://claude.ai/code/artifact/d6c8f13c-e4a2-444e-9f19-0e047e230352**

Publique **passando essa URL**. Ela **não se edita** — sai do
`pagina-dos-pedidos.py`, que lê o `docs/PENDENCIAS.md` e conta os três estados
sozinho. A fonte da verdade é o `.md`; mexeu lá, rode isto.

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

## Três armadilhas de estilo da página

- **Nenhuma cor literal nos SVG.** Tudo sai dos tokens (`var(--reg)`,
  `var(--acento)`…), senão o diagrama some no tema escuro. Confira com
  `grep -c 'fill="#\|stroke="#' dossie-phxsql-0.18.html` — tem de dar zero.
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
