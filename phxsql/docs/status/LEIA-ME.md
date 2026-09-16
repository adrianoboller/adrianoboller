# `docs/status/` — a sétima página: o status do projeto

O dono mandou um exemplo de «status html do projeto» de um projeto irmão
(P.O.S — Phoenix Operating System) e decidiu duas coisas, em 16/09/2026. As
duas são contrato, e é delas que sai tudo o que está nesta pasta:

1. **Página NOVA, a sétima.** As seis que já existem ficam como estão — em
   particular, `docs/pmo/status-do-projeto.html` e o gerador dele **não foram
   tocados**, apesar de o arquivo do exemplo ter exatamente o mesmo nome.
2. **Seção só entra com gerador.** Se um número da seção não sai de um gerador,
   **a seção não nasce** — vira pendência nomeada, com o gerador que falta.

## O comando

```bash
./status-html.sh              # grava docs/status/status-do-projeto.html
./status-html.sh saida.html   # grava em outro lugar
./status-html.sh --conferir   # grava e roda o portão só para este gerador
```

**Chamada sem argumento faz a coisa inteira.** É a lição do
`dossie_da_pasta.py`: alvo que só existe quando vem por argumento é alvo que
alguém esquece — e foi assim que o `pagina-dos-pedidos.py` gravou a página,
gravou a contagem, imprimiu três linhas de êxito e **pulou o painel do dossiê**
em 07/09/2026, deixando três painéis atrasados sem um dígito digitado.

O `.sh` é a **porta**, não uma segunda implementação: a montagem inteira vive
em `docs/status/pagina-do-status-do-projeto.py`. *Receita duplicada é receita
que diverge.*

E ele **diz que fez menos quando fez menos**: as seções que não nasceram saem
nomeadas na saída, com o gerador que falta, debaixo de um cabeçalho que não é
linha de êxito.

| Arquivo | O que é | Edita-se? |
|---|---|---|
| `pagina-do-status-do-projeto.py` | o gerador — uma seção por linha da tabela «de onde sai cada número», abaixo | — |
| `status-do-projeto.html` | a página gerada | **não** — mexeu numa fonte, rode o comando |
| `LEIA-ME.md` | este arquivo | sim |
| `../../status-html.sh` | o comando, na raiz do `phxsql/` | — |

Não confundir com `docs/pmo/pagina-do-status-do-projeto.py`, que tem o **mesmo
nome de arquivo** e gera outra coisa (o painel PMO, três vistas). A pasta é
outra de propósito.

## De onde sai cada número, seção por seção

Nenhum número desta página foi digitado. Cada seção diz na própria página de
qual gerador ela vem (a linha «De onde sai»); esta tabela é o índice.

| § | seção | de onde sai o número |
|---|---|---|
| 01 | Resumo executivo | `CAPABILITIES.json` (via `capabilities()` de `pagina-dos-testes.py`), `PENDENCIAS.md` pelo `ler()` de `pagina-dos-pedidos.py`, `bancada/guardas/catalogo.py`, as constantes `TETO_*` do fonte, a tabela `BANCADAS` de `pagina-dos-testes.py` e o `medir()` de `cobertura-por-area.py` |
| 02 | O que o PhxSql é hoje | `CAPABILITIES.json`. As duas frases sobre ACID e marca são contrato (`CLAUDE.md`, `docs/ACID.md`), não medição — e por isso não trazem número novo |
| 03 | Os crates, medidos | `contar_crate()` de `docs/tecnologias/extrair.py` (o mesmo que escreve a tabela do `TECNOLOGIAS.md`) e a `description` de cada `Cargo.toml`. A lista de crates sai da varredura de `crates/`: crate novo entra sozinho |
| 04 | O formato em disco | varredura de `crates/*/src/**/*.rs` atrás de `const MAGIC*: &[u8; N] = b"…"` e dos `const VERSAO*` do mesmo arquivo. **Leitor próprio** — ver abaixo |
| 05 | O caminho de um pedido | `docs/dossie/fig-fluxo-do-motor.svg`, escrito por `fluxo-do-motor.py` a partir dos portões do próprio fonte Rust. Entra embutido: a página é um arquivo só |
| 06 | O que o motor faz — medido | `bancada/comparativo/resultados.json` — a mesma pergunta em quatro motores vivos, com sonda de efeito |
| 07 | Os pedidos do dono | `docs/PENDENCIAS.md`, pelo `ler()` de `pagina-dos-pedidos.py` |
| 08 | O que está travado com você | `achar_gates()` de `docs/pmo/pagina-do-status-do-projeto.py` — o mesmo léxico do painel PMO, importado |
| 09 | Testes e cobertura por área | `medir()` de `cobertura-por-area.py` (conta `#[test]` no fonte) e o campo `testes` do `CAPABILITIES.json` (sai de um `cargo test` de verdade) |
| 10 | O catálogo de guardas | `guardas()` de `pagina-dos-testes.py`, que **importa** `bancada/guardas/catalogo.py`; `bancada/guardas/ultima-corrida.json`; e a varredura de `#[ignore = "VERMELHA …"]` |
| 11 | As catracas | `catracas()` de `pagina-dos-testes.py`, varrendo `const TETO*` no fonte |
| 12 | As bancadas | a tabela `BANCADAS` e o `linha_bancada()` de `pagina-dos-testes.py` — **bancada sem resultado aparece como não medida**, com o comando para rodar |
| 13 | Desempenho | `g_tres_motores()` de `graficos-dos-testes.py`, sobre `bancada/comparacao/um-milhao.json` |
| 14 | Replicação | `bancada/replicacao/resultados.json` e `bancada/cluster/resultados.json`, cada um com a data da própria corrida |
| 15 | A fábrica de idiomas | o campo `idiomas` do `CAPABILITIES.json` |
| 16 | Zero dependências | `dependencias_externas()` de `numeros-do-projeto.py`, que conta `[[package]]` no `Cargo.lock` e subtrai os crates deste projeto |
| 17 | Documentação | `arquivos_de_doc()` e `linhas_de_doc()` de `numeros-do-projeto.py`; as páginas saem dos **alvos** do `PLANO` de `portao-dos-geradores.py` |
| 18 | O que se baixa | varredura de `pacotes/*.zip` cruzada com `pacotes/SHA256SUMS`. **Leitor próprio** |
| 19 | O board, por pilar | `ler()` de `docs/pmo/rollup.py`, sobre `docs/pmo/BACKLOG.md` |
| 20 | As últimas frentes | `git log --no-merges -n 10`, só leitura. **Leitor próprio** |
| 21 | O que **não** nasceu | a lista `SECOES_SEM_GERADOR` do gerador — ver abaixo |

## Ela não tem leitor próprio de quase nada, e isso é a regra

O gerador **importa** os leitores que já existem, no molde de
`docs/planilha/planilha-das-atividades.py`:

`pagina-dos-pedidos.py` · `pmo/rollup.py` · `pmo/pagina-do-status-do-projeto.py`
· `pagina-dos-testes.py` · `graficos-dos-testes.py` · `cobertura-por-area.py` ·
`tecnologias/extrair.py` · `numeros-do-projeto.py` · `portao-dos-geradores.py`

Duas contagens do `PENDENCIAS.md` divergiriam na primeira mudança de legenda —
e já divergiram: o pedido 150 passou meses invisível numa página cujo leitor
não conhecia o símbolo dele.

Os **três leitores próprios** existem porque ninguém mais lia aquilo, e cada um
tira a lista **do código**, nunca de uma cópia:

- **o formato em disco** — a lista de arquivos do formato sai dos `const
  MAGIC_*` do Rust. Uma tabela copiada aqui perderia o arquivo novo no dia em
  que ele nascesse, que é exatamente o que a receita do KiB da interface já
  custou (780 KiB publicados onde eram 1.032);
- **os pacotes** — tamanho e data são do arquivo real em `pacotes/`;
- **as frentes** — `git log`.

### Dois consertos que esta frente precisou fazer nos vizinhos

- `docs/dossie/cobertura-por-area.py` chamava `main()` **solto no fim do
  módulo**: importá-lo para reusar o `medir()` regravava o `TESTES.md` e o
  dossiê sem ninguém pedir. Ganhou o `if __name__ == "__main__":`. Nada muda
  para quem o roda como script — muda só para quem reusa a receita, que é o
  jeito de não duplicá-la.
- `docs/dossie/numeros-do-projeto.py` tinha a lista dos documentos **dentro**
  do `linhas_de_doc()`. Virou `arquivos_de_doc()`, que o `linhas_de_doc()`
  agora chama: a §17 conta os mesmos arquivos que o `CAPABILITIES.json`, e os
  dois não podem divergir.

## As seções que NÃO nasceram

Quatro seções do exemplo não entraram, e a razão é a mesma: **o número não sai
de gerador nenhum**. Elas não sumiram — estão na §21 da página, com o gerador
que falta, e na lista `SECOES_SEM_GERADOR` do gerador, que é de onde a §21 e o
`status-html.sh` as leem (uma fonte só).

| seção do exemplo | gerador que falta | pedido |
|---|---|---|
| §11 Riscos | `docs/status/riscos.py` + um `docs/RISCOS.md` tabelado | #264 |
| §12 Dívida técnica | o mesmo, sobre uma marcação `// DIVIDA:` no fonte | #264 |
| §15/§16 Telemetria e logs | `docs/status/telemetria-medida.py` + `bancada/telemetria/resultados.json`, que a bancada ainda não grava | #265 |
| §21 Antes × Depois | `docs/status/serie-historica.py` + um `serie.jsonl` versionado — o `CAPABILITIES.json` é sobrescrito e só guarda o AGORA | #266 |

E quatro seções do exemplo **não têm correspondente** aqui, porque são do
P.O.S: ISO bootável, chamadas de sistema, utilitário de terceiros embutido e
site do projeto. A tradução não foi mecânica:

- «ISO bootável» virou a §18, **o que se baixa** — os zips de `pacotes/`,
  montados por `./empacotar.sh` e **nunca à mão**;
- «utilitário de terceiros embutido» virou o **oposto**: a §16, *zero
  dependências externas*, que é pétrea e é diferencial;
- «site do projeto» não tem análogo e não entrou — inventar uma seção vazia
  seria o mesmo defeito, com outro nome.

Quatro seções são **nossas, e o exemplo não tinha**: o formato em disco (§04),
a replicação medida com quatro servidores (§14), o catálogo de guardas (§10) e
as catracas (§11).

## O portão

O gerador está no **portão dos geradores**, em modo `sem-carimbo`:

```bash
python3 docs/dossie/portao-dos-geradores.py --so docs/status/   # só este
python3 docs/dossie/portao-dos-geradores.py                     # todos
```

`sem-carimbo` porque o rodapé carrega «gerado em DD/MM/AAAA HH:MM UTC» e
várias datas de medição vêm do `mtime` (o `git checkout` não preserva `mtime`).
Qualquer outro número que mude ao re-rodar **reprova**.

Só que o portão re-roda o gerador, e este leva **minutos** (medido: 2 min 52 s
nesta máquina, com cinco frentes ao lado — quase tudo em CPU). Ele varre
`crates/` mais de uma vez: classificação por crate, `#[test]` por área,
`TETO_*`, `MAGIC_*`, `#[ignore = "VERMELHA"]`. Use o `--so docs/status/` quando
estiver conferindo só esta página.

### Esta página roda por ÚLTIMO, e o motivo é medido

A §17 publica o **tamanho em KiB das outras páginas geradas**. Isso amarra esta
página a todas elas: rodar o `pagina-dos-pedidos.py` depois desta muda o
`pedidos.html` em 1 KiB, e o portão acusa esta aqui como VELHA — corretamente.
A ordem que funciona é a do `docs/dossie/LEIA-ME.md` com o `status-html.sh`
**no fim**, depois do `numerar-figuras.py`.

E há um número que esta página **não publica de propósito**: o tamanho dela
mesma. Escrevê-lo muda o tamanho dela, e o número seguinte já é outro — o
portão pegou exatamente isso, oscilando 103 ↔ 104 KiB a cada corrida. Número
que não chega a ponto fixo não é número medido; é número perseguindo a própria
cauda, e ele deixaria o portão vermelho para sempre. A célula diz
«— (esta página)», com o motivo ao lado.

## A tela

A marca manda sobre qualquer paleta inventada (`marca/LEIA-ME.md`): **Exo 2**,
fundo `#010418`, assinatura *Built to store. Engineered to scale.* Do exemplo
veio a **estrutura** — barra de título, `.kpi`, `.cartao`, `.etiqueta`,
`.nota`, a barra de três colunas, tema por `data-theme`, seletor de cor e botão
de imprimir. Da paleta dele não veio nada: o `--fire:#E24310` dele deu lugar ao
vermelhão da marca, que escurece para `#C63C0A` sobre papel por contraste.

Três disciplinas que a tela carrega:

- **Rótulo se estiliza, dado nunca.** O seletor de cor pinta **título**, e só.
  Nenhum `text-transform` sobre número ou nome de coisa — «Blumenau» virando
  «BLUMENAU» é mentira sobre o dado, porque quem olha não sabe se está gravado
  assim.
- **Contorno, nunca fundo cheio.** Botões e etiquetas são só contorno; o
  preenchimento acontece no `hover`, quando há intenção. É a convenção das
  cinco cores da ação desta casa (verde inclui, amarelo altera, rosa marca,
  vermelho exclui de vez, azul consulta).
- **A forma carrega o estado.** Barra cheia, hachurada ou só contorno — lê-se
  sem cor. E **o número vem depois da barra**, nunca por cima dela: o gráfico
  das bancadas já saiu com o número riscado por cima do traço, e isso só
  apareceu exercitando no navegador.

Antes de publicar, olhe:

```bash
node docs/dossie/olhar.mjs docs/status/status-do-projeto.html /tmp/status7.png
```

E **meça**, porque o defeito que a foto não mostra é o que rola de lado:

```bash
node docs/dossie/sonda-de-estouro.mjs docs/status/status-do-projeto.html 390
```

A sonda faz três perguntas separadas — a página rola? quem estoura a caixa?
quem estoura o **conteúdo**? — e **sai != 0** quando a página rola. A terceira
é a que importa: ela achou aqui uma trilha de grade de 62 px com 83 px de texto
`nowrap` dentro, três pixels de rolagem lateral que nenhuma captura mostra. A
pergunta ingênua («quem passa da janela?») devolvia doze tabelas inocentes,
todas dentro de um `.rolo` que já as rola.

Prova real nos dois sentidos, medida: com a trilha fixa reposta a sonda sai
**1** dizendo «rola 3 px»; com o conserto, **0**.

Nos dois casos o `playwright` só resolve pelo **caminho absoluto**, e rodado de
fora do repositório o Node cala e imprime só a versão dele.

## Um buraco conhecido, dito em vez de escondido

Este `LEIA-ME.md` **não entra** na contagem de linhas de documentação da §17.
A receita conta `docs/*.md` (não desce em subpasta) mais uma lista de avulsos
em `numeros-do-projeto.py`, e `docs/status/LEIA-ME.md` não está nela. Pô-lo lá
é uma linha — mas a mesma lista alimenta o `linhas_doc` do
`CAPABILITIES.json`, que só o `numeros-do-projeto.py` regrava, e ele chama
`cargo test`. Acrescentar sem poder re-rodar deixaria os dois números
divergindo, que é exatamente o que a receita única existe para impedir. Fica
para a rodada que puder rodar o `cargo`.

## Publique passando a URL

Publique sempre **passando a URL da página**, para cair na mesma em vez de
criar outra. A URL entra aqui na primeira publicação — até lá, esta linha diz
que ela **ainda não existe**, em vez de trazer um endereço inventado.

- **URL:** *(ainda não publicada)*
- **Fonte:** `docs/status/status-do-projeto.html`
