# Fonte do dossiê

`dossie-phxsql.html` é o fonte da página publicada em:

**https://claude.ai/code/artifact/5c14044e-0dc5-4832-b015-224ab1e40033**

Ele mora aqui, e não só na máquina de quem publicou, para que qualquer sessão
futura consiga atualizá-lo. Sem isto, a regra de manter o dossiê em dia seria
impossível de cumprir depois que o diretório temporário sumisse.

## Como atualizar

1. Edite `dossie-phxsql.html`.
2. Publique **passando a URL acima**, para cair na mesma página em vez de criar
   uma nova.

## O que conferir antes de publicar

Os números do painel são medidos, nunca estimados:

```bash
find . -name '*.rs' -not -path './target/*' | xargs cat | wc -l    # linhas de Rust
cargo test --workspace 2>&1 | grep '^test result' \
  | awk '{s+=$4} END {print s}'                                    # testes
$(( $(grep -c '^\[\[package\]\]' Cargo.lock) - 4 ))                # dependências externas
cat docs/*.md README.md CHANGELOG.md MANUAL.txt \
    bancada/LEIA-ME.md marca/LEIA-ME.md docs/dossie/LEIA-ME.md \
  | wc -l                                                          # linhas de doc
```

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
