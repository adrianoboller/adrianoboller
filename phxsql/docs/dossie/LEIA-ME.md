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
find crates -name '*.rs' | xargs cat | wc -l                       # linhas de Rust
cargo test --workspace --release 2>&1 | grep '^test result' \
  | awk -F'[ ;]' '{s+=$4} END {print s}'                           # testes
$(( $(grep -c '^\[\[package\]\]' Cargo.lock) - 4 ))                # dependências externas
cat docs/*.md MANUAL.txt README.md | wc -l                         # linhas de doc
```

Duas armadilhas de estilo da página:

- **Nenhuma cor literal nos SVG.** Tudo sai dos tokens (`var(--reg)`,
  `var(--acento)`…), senão o diagrama some no tema escuro. Confira com
  `grep -c 'fill="#\|stroke="#' dossie-phxsql.html` — tem de dar zero.
- **Todo token de cor nasce no `:root` base.** Cor definida só dentro de
  `@media` ou `[data-theme]` não existe para quem está no tema "sistema".
