# Dossiê do PhxClaw

Publicado em **https://claude.ai/artifact/J5emfeTgE26AapFRTPssDk**. Publique sempre **passando
essa URL**, para cair na mesma página.

## Gerar

```bash
python3 tools/dossie/gerar_dossie.py                 # regrava o dossiê da pasta
python3 tools/dossie/gerar_dossie.py --suite ARQ     # com o placar de uma saída guardada do cargo test
```

O `dossie-phxclaw-*.html` **não se edita**: a página inteira sai do gerador. **Só existe um por
vez**. O `tools/dossie/dossie_da_pasta.py` acha o dossiê por varredura: zero arquivos para a corrida,
dois também. Para trocar de versão, apague o velho e rode com `--novo`, que dá o nome pela versão do
`Cargo.toml`.

Confira a saída. O que não tem arquivo de resultado sai na página como **NÃO MEDIDO**, com o comando
para rodar, e no terminal sob **«FEZ MENOS DO QUE O NOME PROMETE»**, que não é linha de êxito. O
gerador **não roda** o `cargo test` nem os roteiros do Chromium: lê o que eles gravaram.

Rodar duas vezes seguidas não muda nenhum byte: nenhuma data sai do relógio da corrida.

## De onde sai cada número

A tabela completa (fonte, data e como a data foi lida) está no **rodapé da própria página**. Os
leitores ficam em `tools/dossie/numeros.py`, um por fonte:

| números | fonte |
|---|---|
| versão, membros do workspace | `Cargo.toml` |
| linhas de Rust | `crates/**/*.rs` e `apps/**/*.rs` da árvore de trabalho |
| absorção, por fonte e total | `docs/absorcao/absorcao.json` (as fontes que existirem lá); nomes pelo `NOMES_PRODUTO` do `app.js` |
| sprints, kanban, % que falta | tabela «Visão geral» do `docs/absorcao/SPRINTS.md` |
| achados SEC/DBA/QA/prova F, SP000013/14 | seções do mesmo `SPRINTS.md` |
| ferramentas, capacidades | `apps/phxclaw-ui/assets/ferramentas.json` |
| equipe, macroáreas | `apps/phxclaw-ui/assets/equipe.json` |
| papéis de construção | tabelas de `.claude/agents/README.md` |
| roteiro do Integrador, conselho | `.claude/agents/integrador.md` |
| passos do portão único | o corpo do `call_tool_com` em `crates/phxclaw-agent/src/motor.rs` (a ordem sai do código) |
| exceções do bwrap, guardas | `crates/phxclaw-agent/tests/guardas.rs` |
| tokens e fontes da marca | blocos `:root` e `@font-face` do `apps/phxclaw-ui/assets/app.css` |
| qualificação da UI | `tests/desktop/out/qualificacao/qualificar.json` (fora do git) e `docs/ui/qualificacao/RELATORIO_*.md` |
| responsiva e fidelidade | `docs/ui/fidelidade/*.json` (data gravada no próprio resultado) |
| certificação | `reports/RELEASE_CERTIFICATION_v*.json` |
| roteiros da interface | `tests/desktop/**` com linha «Uso:»; o resultado sai do `writeFileSync` do roteiro |
| pétreas | títulos «Cláusula pétrea:» do `CLAUDE.md` da raiz |
| commits | `git log -- phxclaw/` |

**A data** de cada número vem da data que o próprio resultado grava. Na falta dela, vale a data do
commit, se o arquivo está limpo. Se não, vale o mtime, e a página diz «data do arquivo».

## Paradas de propósito

Algumas divergências param o gerador em vez de deixar o desenho mentir:

- portão no `call_tool_com` sem rótulo, ou rótulo sem portão;
- papel do README sem raia no organograma, ou raia citando papel que não existe;
- estado de sprint desconhecido;
- total declarado no JSON diferente da lista;
- referência a figura inexistente (`numerar_figuras.py`);
- página acima de 450 KiB.
