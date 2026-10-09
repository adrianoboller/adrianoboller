# A lista que sai do código precisa do mesmo recorte que a promessa do gerador

**Estado:** PENDENTE

## O que aconteceu

No fecho da 0.19.0 (08/10/2026, papel H), `python3 docs/tecnologias/extrair.py`
caiu com `UnicodeDecodeError: 'utf-8' codec can't decode byte 0x9f`. A lista
dos arquivos embutidos da interface sai do `http.rs` — a correção que a casa
fez depois do rodapé de 780 KiB —, mas o padrão casava **qualquer**
`include_str!`/`include_bytes!("../…")`. No commit `a0325c80` o `http.rs`
passou a embutir `../../phxzip-web/ui/fonte/exo2-latin.woff2` por
`include_bytes!`, e o extrator tentou contar **linhas** de um binário.

## O que eu concluí primeiro, e estava errado

Que o defeito era «o extrator não sabe ler binário», e que bastava contar o
binário como zero linhas. Isso somaria a fonte ao total de KiB «da interface
em `ui/`», que a docstring do próprio extrator promete, e afastaria o número
dele do rodapé do dossiê (o `numeros-do-projeto.py` lê só `include_str!` de
`../ui/`). O defeito era o **recorte**: o padrão era mais largo que a promessa.

## O que a medição disse

Com o recorte `../ui/`, a tabela dá **10 arquivos**; o embutido de fora sai
nomeado abaixo dela (1 arquivo, 39,9 KiB), em vez de entrar calado no total
ou sumir. O `TECNOLOGIAS.md` estava parado desde `a0325c80` (17:04 UTC) sem
ninguém ver, porque o extrator só roda no fecho.

## A regra

Quando a lista de um gerador sai do código, o padrão que a extrai tem o mesmo
recorte que o nome e a docstring do gerador prometem — e o que cai fora do
recorte sai nomeado, não some.

## Como está guardado hoje

`docs/tecnologias/extrair.py`: `arquivos_da_interface_embutidos()` casa só
`../ui/`, e `embutidos_fora_da_ui()` nomeia o resto. **Buraco:** não há teste
que reponha um `include_bytes!` fora de `ui/` e veja o extrator cair; a guarda
de fato é o `portao-dos-geradores.py`, que acusa gerador que sai ≠ 0 — e só no
fecho da rodada.
