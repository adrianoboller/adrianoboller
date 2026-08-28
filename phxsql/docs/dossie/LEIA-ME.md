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

1. Edite `dossie-phxsql-0.15.html`.
2. Publique **passando a URL acima**, para cair na mesma página em vez de criar
   uma nova.

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

Duas armadilhas de estilo da página:

- **Nenhuma cor literal nos SVG.** Tudo sai dos tokens (`var(--reg)`,
  `var(--acento)`…), senão o diagrama some no tema escuro. Confira com
  `grep -c 'fill="#\|stroke="#' dossie-phxsql.html` — tem de dar zero.
- **Todo token de cor nasce no `:root` base.** Cor definida só dentro de
  `@media` ou `[data-theme]` não existe para quem está no tema "sistema".
