# QA — os quatro trechos mortos de hoje (pedido 432)

Papel G. Só leitura e execução das réguas Python (baratas); nenhum `cargo
build`/`test` rodado — disco em 2,3 GB. Nada consertado, nada no
`catalogo.py` tocado, nada comitado.

## 0. O fato medido, confirmado

```
$ python3 bancada/guardas/trecho-vivo.py
trecho-morto   alter-espelho-para-tras: o trecho nao esta mais la (crates/phxsql-store/src/reg.rs)
trecho-morto   fts-reconstruir-sem-recriar: o trecho nao esta mais la (crates/phxsql-store/src/table.rs)
trecho-morto   fts-abrir-recusa-a-tabela: o trecho nao esta mais la (crates/phxsql-store/src/table.rs)
trecho-morto   fts-nasce-na-pista-de-leitura: o trecho nao esta mais la (crates/phxsql-store/src/table.rs)
EXIT=0
```

Confirmado. E confirmado também que **existe uma segunda porta**, já provada
e já vermelha para o mesmo defeito:

```
$ python3 bancada/guardas/trecho-vivo.py --catraca
   201 guardas no catalogo + 1 aposentada(s) escrita(s)
   ...
   SUBIU  TETO_TRECHO_MORTO: 4 (teto 0)
   Reprovado: guarda que nao pode nem ser tentada nao esta guardando nada -- ...
   ...
   SUBIU  TETO_NAO_JULGADA_ESCONDIDA: 2 (teto 0)
   CRESCEU -- SUBA O PISO  PISO_DAS_ENTRADAS: 202 (piso 200)
EXIT=1
```

`docs/QA-PDCA.md` (gerado por `docs/qa/medir.py`, commit `8c3e77c`, hoje
21:34 UTC) já publica `TETO_TRECHO_MORTO … 4 … **REPROVANDO** — 4 acima`. Ou
seja: **a catraca que importa já apitou** — o comando citado no FATO MEDIDO
é o modo de listagem sem flag (não documentado no `LEIA-ME.md`, chamado por
nenhum outro script), não o `--catraca`. Isso não muda o veredito de nenhuma
das quatro perguntas abaixo, mas muda a pergunta 2.

## 1. Moveu ou morreu? As quatro, uma a uma

Método: `git diff 55ab903..97dffc6 -- <arquivo>` mais `git show <commit>:<arquivo>`
em `bc495f6` (ontem, catraca 0), `805fb34` (hoje 05:58 UTC, catraca 3) e
`97dffc6` (hoje 18:13 UTC, catraca 4) para achatar exatamente QUANDO cada
trecho parou de casar. Nenhuma das quatro é código morto — as quatro
**MOVERAM**, por dois motivos de commit diferentes:

- **3 delas morreram em `805fb34`** (0.19.0, PSCH v10 + `.fts` selado):
  `FtsFile::recriar(...)` ganhou um terceiro parâmetro (`selar_o_fts`) e o
  `match` de abertura ganhou um braço novo
  (`Err(e) if matches!(e, PhxError::Autorizacao(_)) => return Err(e)`, a
  proteção do pedido 340 contra reabrir o `.fts` em claro com senha errada).
- **1 delas morreu em `97dffc6`** (pedido 422, migração fora da trava
  global): `RegFile::alargar_pra_uma_coluna` virou duas funções —
  `alargar_fase_a` e `alargar_fase_b` —, e o `for` que troca os volumes
  passou a iterar `&pendente.trocas` (uma tupla de 2, `(caminho, espelho)`)
  em vez de `&primeiros` (uma tupla de 4, `(v, _, caminho, espelho)`).

### 1.1 `alter-espelho-para-tras` — `crates/phxsql-store/src/reg.rs`

Morreu em `97dffc6`. O laço de troca saiu de `RegFile::acrescentar_coluna` e
entrou em `RegFile::alargar_fase_b`, com a tupla simplificada (o índice `v`
do volume e o campo mudo `_` desapareceram; o retrato ficou em outro campo da
`TrocaPendente`). A lógica que a guarda prova — o espelho `.bkp` acompanhar
a troca do volume — **continua ali, inteira**, só que na nova função.

Trecho novo (casa 1× em `reg.rs`, conferido):

```rust
        for (caminho, espelho) in &pendente.trocas {
            trocar_pelo_novo(caminho)?;
```

Troca (defeito reposto) — mesmo padrão de antes, só a tupla mudou:

```rust
        // DEFEITO REPOSTO: o espelho nao acompanha a troca.
        for (caminho, espelho) in &pendente.trocas {
            let espelho: &Option<PathBuf> = &None;
            trocar_pelo_novo(caminho)?;
```

### 1.2 `fts-reconstruir-sem-recriar` — `crates/phxsql-store/src/table.rs`

Morreu em `805fb34`. É a mesma chamada, em `Table::reconstruir_fts`, agora
com um terceiro argumento (`texto_sobre_coluna_marcada(&self.esquema)` —
a cifra da coluna marcada, pedido 340) e um comentário novo acima. O
comportamento provado — recriar o arquivo antes de varrer, para a segunda
passada não bater em chave repetida — não mudou.

Trecho novo (casa 1×):

```rust
        self.fts = Some(FtsFile::recriar(
            caminho(&self.diretorio, &self.nome, EXT_FTS),
            dobra,
            texto_sobre_coluna_marcada(&self.esquema),
        )?);
```

Troca — **não precisa mudar**: o defeito reposto do catálogo já apaga o
trecho inteiro e o troca por um comentário (não reconstrói o `.fts`, varre
por cima do que já existe), então ele é independente de quantos argumentos
a chamada tem:

```rust
        // DEFEITO REPOSTO: varre por cima do indice que ja existe, em vez de
        // recriar o arquivo. A segunda passada bate em chave repetida.
```

### 1.3 `fts-abrir-recusa-a-tabela` — `crates/phxsql-store/src/table.rs`

Morreu em `805fb34`, e **é a segunda vez que esta entrada quebra** — o
próprio comentário dela no catálogo já registrava a primeira (no dia em que
nasceu, 16/09, um braço `Err(_) if !escrever` entrou no meio do mesmo
`match`). Desta vez o `match` ganhou um comentário de sete linhas e um braço
novo, ANTES do braço que a guarda cobre — a recusa de reabrir o `.fts` em
claro com senha errada (pedido 340). É um `match` que qualquer frente de
cripto mexe, e é o terceiro trecho mais frágil dos quatro por isso mesmo
(ver recomendação na seção 3).

Trecho novo (casa 1× — precisa do bloco inteiro, comentário incluso, porque
uma janela menor não é mais um substring único do arquivo depois do braço
novo):

```rust
            match FtsFile::abrir(&caminho_fts, dobra.clone()) {
                Ok(f) => Some(f),
                // **Falta de CHAVE nao cai na vala do "refaz".** A vala existe
                // para `.fts` corrompido ou divergente, onde refazer do `.reg`
                // devolve a verdade. Aqui ela devolveria um `.fts` NOVO e em
                // claro, com os termos da coluna marcada de volta ao disco
                // legiveis -- desfazendo calado a protecao do pedido 340 por
                // causa de uma senha errada no `config.json`. Recusa
                // nomeando, que e o que o `.reg` faz na mesma situacao.
                Err(e) if matches!(e, PhxError::Autorizacao(_)) => return Err(e),
                Err(_) if !escrever => {
                    return Ok(SemEscrever::PrecisaEscrever(
                        "o indice de texto .fts nao abre e seria refeito",
                    ));
                }
                Err(_) => {
                    refazer = true;
                    Some(FtsFile::recriar(&caminho_fts, dobra, selar_o_fts)?)
                }
            }
```

Troca — **não precisa mudar**, pelo mesmo motivo do item 1.2: o defeito
reposto do catálogo substitui o bloco inteiro por uma única chamada que
sempre recusa a tabela quando o `.fts` não abre, e isso é ortogonal a
quantos braços o `match` tem:

```rust
            // DEFEITO REPOSTO: `.fts` que nao abre volta a derrubar a tabela.
            Some(FtsFile::abrir(&caminho_fts, dobra.clone())?)
```

### 1.4 `fts-nasce-na-pista-de-leitura` — `crates/phxsql-store/src/table.rs`

Morreu em `805fb34`. Mesma causa do item 1.2: só a chamada a
`FtsFile::recriar` ganhou o terceiro argumento.

Trecho novo (casa 1×):

```rust
        } else if refazer {
            if !escrever {
                return Ok(SemEscrever::PrecisaEscrever(
                    "o indice de texto .fts desta tabela ainda nao existe e seria criado",
                ));
            }
            Some(FtsFile::recriar(&caminho_fts, dobra, selar_o_fts)?)
```

Troca:

```rust
        } else if refazer {
            // DEFEITO REPOSTO: o `.fts` volta a nascer sem olhar a ficha.
            Some(FtsFile::recriar(&caminho_fts, dobra, selar_o_fts)?)
```

**Nenhuma das quatro se aposenta.** As quatro protegem um comportamento que
o código de hoje ainda tem; o que morreu foi só o ponteiro. Quem aplicar o
reapontamento ainda precisa rodar o provador (`provar-guardas.py --so
<id>`) para confirmar PROVADA — esta rodada não fez isso, por instrução
explícita (sem `cargo`).

## 2. O conferidor que avisa e passa

**Medido:** o catálogo tem hoje **201 guardas vivas + 1 aposentada** (`cifra-
do-fio-imposta`, 18/09) = 202 no piso, contra `PISO_DAS_ENTRADAS = 200`
(também vermelho, +2, mas por motivo não relacionado a este pedido).

**Histórico de `TETO_TRECHO_MORTO` acima de zero** (via `git log`/`git show`
em `docs/QA-PDCA.md` e nos documentos de rodada — a régua nasceu em
16/09/2026, pedido 263):

| quando | trechos mortos | achado por | desfecho |
|---|---:|---|---|
| 16/09/2026 (fundação) | 8 | leitura manual (a régua NÃO EXISTIA ainda) | motivou a criação do `trecho-vivo.py`; ficaram 4 dias sem ninguém ver |
| 17/09/2026 ~04:00 UTC | 1 (`trava-atras-da-rede`) | `--catraca`, na hora | reancorado na mesma madrugada |
| 17/09/2026 ~04:xx UTC | 1 (fusão da condição do pulso, `cluster.rs`) | `--catraca`, achado SEC A11 | reancorado na mesma madrugada |
| 23/09/2026 05:58 UTC (commit `805fb34`) | 3 (as três do `.fts`) | `docs/qa/medir.py --gravar` | **comitado vermelho** e deixado assim |
| 23/09/2026 18:13 UTC (commit `97dffc6`) | 4 (+ `alter-espelho-para-tras`) | idem | **ainda vermelho agora**, 12 commits depois |

A régua nunca "avisou e passou" nas duas primeiras vezes que importou: nas
duas de 17/09 alguém rodou `--catraca` na hora e consertou antes de
comitar. **Hoje é diferente**: `docs/QA-PDCA.md` já registra `REPROVANDO —
4 acima` desde as 21:34 UTC, e ninguém consertou nos ~3h20 e nos 12 commits
entre `97dffc6` e `HEAD`. Isso não é falha do `trecho-vivo.py` — é falha de
alguém rodar/ler o que ele já diz.

**Resposta à pergunta "ele deve virar catraca":**

- O modo que **importa** (`--catraca`) **já é catraca** e já reprova (saída
  1) — confirmado ao vivo acima. Ele já está pendurado em
  `bancada/bateria/prova-bateria.py` (item do pedido 263), que devolve `sys.exit(1)`
  se qualquer `confere()` falhar.
- O modo que **avisa e passa** é o branch sem flag no fim de `principal()` —
  não documentado no `LEIA-ME.md` (que só ensina `--catraca`/`--numeros`/`--autoteste`),
  e sem NENHUM chamador no repositório que dependa da saída 0 dele (conferido
  por grep em `*.py` e `*.sh`). Ele existe para um humano grepar
  `trecho-morto`/`teste-morto` linha a linha, não para portão.

**Recomendação (não implementada por mim, papel B decide):** sim, vale a pena
esse modo também sair `!= 0` quando achar qualquer coisa (`trechos or
ambiguos or testes or fora`), pelo motivo que este próprio pedido acabou de
demonstrar: o comando do FATO MEDIDO usou exatamente esse modo, e uma saída
0 ao lado de quatro linhas "trecho-morto" é o tipo exato de leitura errada
que "guarda que avisa e passa" describe. O risco de regressão é baixo — zero
chamador depende do 0 hoje — e não há cliente antigo para quebrar (é modo de
depuração, não parte da API do arquivo). **Número de hoje se isso virar
catraca: 4** (mesmo número do `TETO_TRECHO_MORTO`, porque `ambiguos`,
`testes` e `fora` estão todos em 0 agora). Alternativa mais barata que não
duplica lógica: apagar o modo sem flag do `principal()` e apontar todo mundo
para `--catraca`, que já cobre isto e mais duas réguas (`TETO_NAO_JULGADA_ESCONDIDA`,
`PISO_DAS_ENTRADAS`) que o modo cru nem calcula.

## 3. O inventário guarda × pétrea

| guarda | pétrea/garantia que ela prova | pétrea nomeada em CLAUDE.md? |
|---|---|---|
| `alter-espelho-para-tras` | o espelho `.bkp` acompanha TODA alteração de esquema — `docs/FORMATO.md` linhas 43-54 e 295 ("fase A escreve TODOS os `*.novo` (principal e espelho, todos os volumes)"); é garantia de formato em disco, domínio do papel C (DBA) | não é uma das frases da lista "Regras que não se quebram"; é uma garantia documentada em `FORMATO.md` sob a mesma autoridade (mudança de formato é do DBA) |
| `fts-reconstruir-sem-recriar` | idempotência da reconstrução de um índice DERIVADO (`.fts`): rodar `reindexar` duas vezes não pode quebrar com chave duplicada | não nomeada; é invariante de confiabilidade documentado no próprio comentário do código, provado por teste (papel F/G) |
| `fts-abrir-recusa-a-tabela` | `.fts` corrompido/divergente se refaz sozinho, nunca derruba a tabela (a mesma "ordem impossível" que a lei de `ao_excluir` evita por outro caminho) — **E** faz fronteira direta com a pétrea **"senha nunca em texto puro"**: o braço novo (`Err(e) if matches!(e, PhxError::Autorizacao(_))`) é o que impede este "auto-cura" de reabrir o `.fts` em claro quando a senha do cofre está errada (pedido 340) | a segunda metade **é** a pétrea nomeada; a primeira metade não tem frase própria em CLAUDE.md |
| `fts-nasce-na-pista-de-leitura` | nenhuma leitura sob a ficha COMPARTILHADA pode escrever disco — a mesma classe de invariante de concorrência que `mapa-da-trava.py` mede (`codigo-do-dono`, `rede-ou-espera`); aqui é o caso específico do `.fts` nascendo dentro de `abrir_com(escrever=false)` | não nomeada como frase; é a mesma classe da regra de concorrência da trava de dados, sem bullet própria em CLAUDE.md |

**Pétreas que ficaram sem prova periódica hoje** (porque a guarda que as
prova está com o ponteiro morto, logo `provar-guardas.py --so <id>` nem
tenta rodá-la):

1. **Formato em disco do espelho `.bkp` em ALTER TABLE** (`alter-espelho-
   para-tras`) — garantia documentada em `FORMATO.md`, sob o papel C.
2. **Idempotência da reconstrução do índice de texto** (`fts-reconstruir-
   sem-recriar`).
3. **Auto-cura do `.fts` sem derrubar a tabela, incluindo o limite dessa
   auto-cura contra a pétrea "senha nunca em texto puro"** (`fts-abrir-
   recusa-a-tabela`) — esta é a mais grave das quatro: é a que intercepta o
   pedido 340 (senha errada não pode reabrir `.fts` em claro), e é a que já
   quebrou duas vezes no mesmo `match`.
4. **Nenhuma escrita sob a ficha compartilhada, caso do `.fts`**
   (`fts-nasce-na-pista-de-leitura`).

Nenhuma delas está sem guarda por decisão — as quatro têm entrada no
catálogo e as quatro têm o defeito nomeado. O que falta é o ponteiro casar
de novo, o que a seção 1 já entrega pronto para colar.

## 4. O irmão

A pergunta que a lei nomeia é **"o catálogo ainda aponta para código que
existe?"**. Há outro conferidor com exatamente essa forma — catálogo escrito
à mão, com um trecho de código citado literalmente (`agulha`), que precisa
casar contra o fonte de hoje:

**`bancada/concorrencia/mapa-das-threads.py`** (pedido 248). O `CATALOGO`
de cada sítio de nascimento de thread traz `"agulha"` — um trecho de código
citado ao pé da letra, como `std::thread::Builder::new().name(nome_do_so)`
ou `format!("dados-{}", endereco.port())` — que o medidor procura no
`"arquivo"` nomeado. A catraca `catalogo-envelhecido` ("entradas do catálogo
que não casam com sítio nenhum") é o `TETO_TRECHO_MORTO` deste catálogo,
palavra por palavra: o próprio cabeçalho do arquivo diz "a entrada que
envelheceu, e catálogo velho é pior que catálogo nenhum porque parece
completo" — a mesma frase, a mesma lei.

Rodado agora (Python, barato):

```
$ python3 bancada/concorrencia/mapa-das-threads.py --catraca
    20 sitios de nascimento fora dos testes, em crates/*/src

   ok       spawn-sem-teto           0 (teto 0)
   ok       catalogo-envelhecido     0 (teto 0)

As duas catracas seguram.
EXIT=0
```

**Está verde.** E, ao contrário do `trecho-vivo.py` de hoje, este já está
sendo ouvido: é o item 0c de `prova-bateria.py`, devolve `sys.exit(1)` na
função `catraca()` quando reprova, e não há registro de tê-lo pego
desprevenido — nenhuma entrada dele aparece na lista de `ENVELHECIDA` acima.

(`mapa-da-trava.py`, a outra régua da mesma pasta, **não** é irmã desta
pergunta: ela conta `travar_dados()` dinamicamente e não guarda nenhum
trecho de código citado — responde "quanto" e "o que se alcança", nunca
"este ponteiro ainda existe?".)

## Resumo da entrega

- **Moveu, não morreu — as quatro.** Trechos novos prontos para colar na
  seção 1; nenhuma aposentadoria.
- **`--catraca` já é catraca e já reprova (4).** O que avisa e passa é o
  modo sem flag, não documentado, sem chamador — recomendo que também vire
  `!= 0` (número de hoje: 4), mas quem decide implementar é o papel B.
- **Quatro pétreas/garantias sem prova periódica hoje**, nomeadas na seção 3
  — a mais grave é `fts-abrir-recusa-a-tabela`, na fronteira com "senha
  nunca em texto puro" e já na sua segunda quebra de ponteiro.
- **O irmão é `mapa-das-threads.py`** (catraca `catalogo-envelhecido`), e
  está verde (0/0) agora.
