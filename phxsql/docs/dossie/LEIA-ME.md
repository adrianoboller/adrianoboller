# Fonte do dossiê

`dossie-phxsql-0.15.html` é o fonte da página publicada em:

**https://claude.ai/code/artifact/5c14044e-0dc5-4832-b015-224ab1e40033**

O nome mudou na 0.15.0 — o dossiê foi refeito com o estado medido daquela
versão. Os dois scripts abaixo aceitam o caminho do HTML como argumento, para
que trocar o nome de novo não exija editá-los.

Ele mora aqui, e não só na máquina de quem publicou, para que qualquer sessão
futura consiga atualizá-lo. Sem isto, a regra de manter o dossiê em dia seria
impossível de cumprir depois que o diretório temporário sumisse.

## Como atualizar

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

## O que conferir antes de publicar

**Os números do painel e do rodapé não se digitam mais.** Saem de

```bash
python3 docs/dossie/numeros-do-projeto.py
```

que mede tudo e reescreve os três blocos entre as marcas `<!-- projeto:… -->`,
`<!-- rodape:… -->` e `<!-- selo:… -->` — o selo entrou porque a versão na capa
ficou **quatro lançamentos** dizendo 0.11.0. Ele segue **a receita abaixo, na letra** — mexeu numa,
mexa na outra, senão volta a existir número de vitrine que ninguém reproduz.
`--so-medir` mostra sem gravar; `--sem-testes` pula o `cargo test`, que demora.

A receita, para conferir à mão:

```bash
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

A interface são os **três arquivos que o `http.rs` embute com `include_str!`** —
`index.html` mais o CSS e o JS do phx-grid. Contar só o `index.html` daria um
número menor do que o publicado, e ninguém conseguiria reproduzir o rodapé.

**O conjunto de arquivos importa.** A receita de linhas de doc já esteve mais
curta do que o número publicado — e aí ninguém consegue reproduzir a capa.
Se acrescentar documento novo, acrescente aqui também.

Os números da **seção 17 (a bancada)** não se digitam: saem de

```bash
python3 docs/dossie/numeros-da-bancada.py
```

que lê `bancada/resultados.json` e reescreve a figura, a tabela e o parágrafo
do diagnóstico entre as marcas `<!-- bancada:… -->`. Número digitado envelhece
calado; foi assim que a capa passou três lançamentos dizendo 276 testes.

## A outra página: os 129 pedidos

`pedidos.html` é a relação de tudo que o Adriano pediu, com o estado de cada
item, publicada em:

**https://claude.ai/code/artifact/d6c8f13c-e4a2-444e-9f19-0e047e230352**

Publique **passando essa URL**, para cair na mesma página. Ela **não se edita** —
sai de

```bash
python3 docs/dossie/pagina-dos-pedidos.py [saida.html]
```

que lê `docs/PENDENCIAS.md` e conta os três estados sozinho. A fonte da verdade
é o `.md`; mexeu lá, rode isto. Uma lista de 129 linhas com três contadores
mantida à mão estaria errada no dia seguinte — é a mesma razão do selo.

Duas coisas que só apareceram abrindo no navegador, e ficam registradas para
quem mexer:

- **`thead` grudento dentro de `overflow-x:auto` cai por cima da primeira
  linha.** O `.rolo` vira contexto de rolagem próprio, e o `position:sticky`
  passa a se medir por ele. Quem gruda é a barra de filtro.
- **Busca em português tem de achatar acento.** Sem `normalize('NFD')`, quem
  digita «indice» não acha «índice» — e a busca falha calada.

Duas armadilhas de estilo da página:

- **Nenhuma cor literal nos SVG.** Tudo sai dos tokens (`var(--reg)`,
  `var(--acento)`…), senão o diagrama some no tema escuro. Confira com
  `grep -c 'fill="#\|stroke="#' dossie-phxsql-0.15.html` — tem de dar zero.
- **Todo token de cor nasce no `:root` base.** Cor definida só dentro de
  `@media` ou `[data-theme]` não existe para quem está no tema "sistema".
