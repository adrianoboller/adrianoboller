# As catracas do PhxSql — inventário medido

Levantamento do papel G (QA) sobre **todas** as catracas numéricas da árvore:
o teto declarado, o valor medido hoje e a folga entre os dois. Reproduza com

```bash
flock /tmp/phx-cargo.lock cargo build --release --examples -p phxsql-server
flock /tmp/phx-cargo.lock python3 docs/qa/medir.py
```

> **A tabela viva não é esta.** Este documento é um *retrato datado* com o
> raciocínio de cada catraca — por que ela existe, que defeito a motivou, o
> que ela não cobre. Os números do dia saem do gerador
> (`python3 docs/qa/medir.py --gravar`, que escreve dentro do
> `docs/QA-PDCA.md`). Eram quatro; viraram oito em 07/09/2026 com a
> `TETO_BOTAO_SEM_PROVA` (pedido 190, §9), a `TETO_TEMP_DIR_SOLTO` (pedido 150,
> §6) e a `TETO_VERMELHA_SEM_PEDIDO` (pedido 212, §7); e são **vinte** desde
> 16/09/2026, quando o gerador passou a enxergar também as **onze** que moram
> em Python, em `bancada/` (§14) — nove em Rust, onze em `bancada/`. Número
> datado numa prosa não é número errado; número datado **sem dizer que é
> datado** é.

Medido em `5ca5326` (2026-09-03), com `cargo test -p phxsql-server --lib
conferidor` verde (24 testes) logo depois. A árvore é compartilhada — outra
frente tinha o `phxsql-server` quebrado (`dblink::Motor::Phx` sem todos os
`match` cobertos) durante boa parte desta medição, e a corrida só rodou depois
que aquela frente terminou os arms que faltavam. Nenhum dos arquivos das
quatro catracas abaixo estava na lista de modificados dela.

## O que é catraca, e o que não é

Uma **catraca** mede uma dívida de código que só deve encolher: um
conferidor conta hoje quantas ocorrências de um padrão indesejado existem, e
um teste falha se esse número passar de um teto congelado no código. **Só
desce.** Regra que passa a medir mais não sobe o teto — aposenta a catraca
velha e faz nascer uma com nome novo, no número medido do dia.

Um **limite de funcionamento** é outra coisa: um teto que protege o
*servidor em produção* — memória, tempo de resposta, tamanho de alocação —,
não uma métrica de qualidade do código-fonte. `TETO_DA_CASCATA` é o exemplo
que a tarefa já veio citando: ele existe para a cascata de `ao_alterar` nunca
recursar sem fundo, e o número (16) não mede quantas linhas de código estão
"erradas" — mede quanto trabalho a conferência aceita fazer antes de recusar.
Confundir os dois é o erro que este documento existe para não cometer: um
limite de funcionamento subindo não é catraca afrouxando.

Um terceiro caso, à parte dos dois: **portão binário**. `cargo fmt --check`,
`cargo clippy -D warnings`, `cargo test --workspace`, o conferidor de zero
dependências (`conferidor_dependencias.rs`) e o **portão dos geradores**
(`docs/dossie/portao-dos-geradores.py`, seção 10 abaixo) não têm "folga" — são
verdadeiro/falso, não contagem. Não entram na tabela abaixo pelo mesmo
motivo que `TETO_DA_CASCATA` não entra: não há número que decresça. **Não crie
um `TETO` para o portão dos geradores**: «um derivado velho» não é uma dívida
que encolhe de dez para nove — é zero ou não-zero, e um teto que aceitasse «só
três painéis velhos» seria a catraca frouxa que a própria casa proíbe.

## As cinco catracas de qualidade — medidas hoje

| Catraca | Onde mora | Teto | Medido hoje | Folga | Estado |
|---|---|---:|---:|---:|---|
| `TETO_TABELA_NA_MAO` | `crates/phxsql-server/src/conferidor_grades.rs` | 0 | **0** | **0** | **fechada** — pedido 158 |
| `TETO_ROTULOS_E_CRASE` | `crates/phxsql-server/src/conferidor.rs` | 1.707 | **1.707** | **0** | sem folga |
| `TETO_COLADO` | `crates/phxsql-server/src/conferidor.rs:1088` | 0 | **0** | **0** | sem folga |
| `TETO_FRASE_REPETIDA` | `crates/phxsql-server/src/conferidor.rs:1092` | 0 | **0** | **0** | sem folga |
| `TETO_FSYNC_POR_FECHO_V2` | `crates/phxsql-store/src/conferidor_fsync.rs` | 8 | **8** | **0** | sem folga — **substitui a V1 (7)**, aposentada |

As quatro primeiras foram medidas em 03/09/2026 (commit `5ca5326`, descrito no
resto desta seção); a quinta é de 04/09/2026, desta rodada — a medição dela
está isolada na seção 5, abaixo, para não misturar datas de medição na mesma
prosa. A quinta **já nasceu e já foi aposentada no mesmo dia**: a `V1` valia 7
e a `V2` vale 8, e a seção 5 conta por quê.

**Achado principal (03/09): nenhuma catraca frouxa entre as quatro. As quatro
estavam coladas no teto, folga zero.** Não há o que baixar naquele dia —
baixar um teto que já é igual ao medido reprovaria a suíte imediatamente, e
essa não é a lei ("catraca frouxa não segura nada"): é o oposto dela, uma
catraca que já segura no talo. O
outro lado da mesma moeda também vale nomear, porque é o que a tarefa pediu
para procurar: **as quatro estão no ponto exato em que a PRÓXIMA violação —
uma tabela nova montada à mão, um rótulo cravado, uma tradução colada — já
reprova a build.** É a postura correta de uma catraca madura: sem colchão
para esconder uma regressão pequena atrás de folga acumulada.

Isto não é acidente: as duas catracas de idiomas foram fechadas a **zero** de
propósito (comentário no código: *"So desce, e hoje e zero"*), e as outras
duas (tabelas e textos fora da fábrica) já passaram por pelo menos uma
aposentadoria-e-renascimento cada uma nesta árvore, sempre fechando no número
medido do dia, nunca deixando margem. A disciplina de "baixe no mesmo
commit" já está sendo seguida — esta rodada não achou nenhuma violação dela.

### 1. `TETO_TABELA_NA_MAO` — tabelas montadas à mão em vez de `PhxGrid`

**O defeito que motivou**: palavra do dono — *"todas as table são phxgrid
com agrupamento dinâmico"*. Sem catraca, a padronização durava até a
próxima tela: quem acrescenta UI escreve `<table>` porque é o que conhece, e
ninguém percebe — exatamente o que já tinha acontecido com os textos fora da
fábrica de idiomas (a máquina existia desde a 0.17.0 e a tela ainda tinha 16
`data-txt` cravados em 11.987 linhas).

**A régua já foi trocada uma vez, do jeito certo.** A primeira versão só
contava `<table>` cru. Quando o conferidor aprendeu a enxergar também o
ajudante `tabela(cabecas, linhas, montar)` — a mesma tabela à mão com menos
letras —, o número medido pulou de 24 para 43. A régua nova **aposentou** a
catraca antiga em vez de subir o teto: nasceu `TETO_TABELA_NA_MAO` (esta),
com o comentário no código dizendo que ela substitui a de `<table>` cru e
que a série histórica se perde de propósito — perder a comparação é mais
barato que deixar "mudei a régua" virar a porta pela qual se afrouxa uma
catraca.

**Medido hoje** (`cargo run --release --example grades-fora-do-padrao -p
phxsql-server`): **55** chamadas a `PhxGrid.criar(`; **0** na mão; **24**
isentas com motivo registrado.

**Ela fechou em 03/09**, e por CLASSIFICAÇÃO e não por conversão em massa: das
24 que restavam, quatro eram lista de dado e viraram grade (Profiler,
transações abertas, resultado de consulta da tela da Claude, e o ajudante
`tabela()`, que morreu com o último chamador); as outras vinte entraram em
`ISENTAS` com o motivo — formulário, ficha técnica, prévia, o pivot, o cartão
do ER, e uma que não é tabela. Ver `docs/GRADE.md` §8.1.

**Zero não quer dizer «acabou a tela»**: quer dizer que não há mais tabela à
mão SEM MOTIVO, e é a catraca mais dura que já houve aqui — tabela nova sem
grade e sem linha em `ISENTAS` reprova na hora.

**A guarda de piso desta catraca foi APOSENTADA**, e o motivo fica escrito
porque ele volta a valer um dia: ela dizia «sobraram muito menos que o teto,
baixe-o no mesmo commit», e em zero virou `>= 0` — sempre verdadeira, e o
clippy a reprovou. Se um dia a régua passar a medir mais e nascer uma catraca
nova num número alto, **a nova precisa do piso de volta**.

### 2. `TETO_ROTULOS_E_CRASE` — textos de tela fora da fábrica de idiomas

**O defeito que motivou**: ordem do dono sobre o agente tradutor — texto de
tela entra pela fábrica de idiomas (`phxsys.mensagens`), não cravado. A
catraca original (`TETO`, aposentada) só reconhecia rótulo entre aspas
simples ou duplas; até o pedido 165 duas formas escapavam por completo: texto
entre CRASE (`` avisar(`Tabela criada`) ``) e rótulo escondido DENTRO de uma
interpolação (`${carta("Título", ...)}`, onde a chamada inteira sumia junto
com o `${…}` que a embrulha). Medido antes do conserto: 1.549 sob a régua
velha.

**A régua trocou de novo, mesmo molde.** Quando o crivo passou a enxergar
crase e rótulo interpolado, `TETO` foi aposentado e nasceu
`TETO_ROTULOS_E_CRASE`, em 1.744 — o mesmo commit que ensinou o crivo também
traduziu o lote coerente do Painel, baixando para **1.720**, o teto de hoje.

**Medido hoje** (`cargo run --release --example textos-fora-da-fabrica -p
phxsql-server`): 1.175 textos já na fábrica, 1.720 fora, 183 isentos (nome
próprio, sigla, identificador) — cobertura de **40%** dos 2.895 textos
visíveis. É o número que sustenta chamar isto de "sem folga": a fábrica cobre
menos da metade da tela, e o teto trava exatamente no que falta hoje.

### 3. `TETO_COLADO` — chaves com os seis idiomas idênticos

**O defeito que motivou**: rótulo com aparência de traduzido que não foi.
Uma chave de i18n com o mesmo texto nos seis idiomas passa despercebida se
ninguém comparar coluna a coluna — e ela NÃO pode comparar com o português
como referência, porque 33 chaves têm o espanhol genuinamente igual ao
português (`Database`, `Profiler`, `Menu principal`) e comparar assim
reprovaria o que está certo. Nasceu **em zero** e nunca teve uma primeira
violação registrada — é catraca fechada desde o dia em que entrou.

**Medido hoje**: 0 chaves coladas nos seis idiomas. Prova real documentada no
próprio teste: trocar as seis colunas de uma chave pelo português faz o teste
reprovar nomeando a chave; devolver a tradução faz passar de novo.

### 4. `TETO_FRASE_REPETIDA` — frase longa repetida em 3+ idiomas

**O defeito que motivou**: o colar PARCIAL que a catraca 3 não pega — quem
traduz três colunas de uma chave e cola o português nas outras três. Mede a
mesma frase (>25 caracteres no MIOLO, sem os `{marcadores}`) aparecendo em
três ou mais dos seis idiomas. O corte pelo miolo evita falso positivo em
moldes como `"{id}{eu} · {nivel} · {sub} · peso {peso}"` (39 caracteres, uma
palavra só, igual em três idiomas por coincidência de pontuação).

**Medido hoje**: 0 frases longas repetidas em três ou mais idiomas. Mesma
prova real do item 3: copiar o português para dois outros idiomas de uma
chave existente faz reprovar nomeando a chave e quantos idiomas trazem a
frase.

### 5. `TETO_FSYNC_POR_FECHO_V2` — `fsync` gasto no fecho de janela

**Ela substitui a `TETO_FSYNC_POR_FECHO_V1`, que valia 7, e a substituição é a
lei sendo cumprida e não contornada.** O defeito que a V1 descreveu — o `.reg`
que não ia ao disco — foi consertado na mesma rodada (`FORMATO.md` §8,
`DESEMPENHO.md` §16), e o número real subiu para **8** por CORREÇÃO: o oitavo
`fsync` é o do dado. Subir o teto de 7 para 8 seria a mesma porta que subir
`TETO_TABELA_NA_MAO` de 24 para 43 teria aberto, então a V1 foi **aposentada**
e a V2 nasceu no número medido do dia. A série com o passado se perde de
propósito.

**E a mudança pagou por si no mesmo dia**: a V2 é cobrada nos dois sentidos —
`medido <= teto` e `medido == teto` —, e foi o segundo lado (o de catraca
frouxa) que reprovou um binário construído para uma **ablação de medição**, em
que `Table::sincronizar` mandava ao disco só quatro dos oito arquivos. Um teto
sem o lado da folga teria deixado passar: 4 é menor que 8.

**Duas correções de forma vieram junto**, e a primeira é o motivo de esta
seção existir:

* **a catraca não estava no inventário.** O `docs/qa/medir.py` acha catraca
  varrendo `crates/*/examples/*.rs` atrás de quem imprime `catraca:` e responde
  a `--numeros`, e acha teto órfão varrendo `crates/*/src/**/*.rs` atrás de
  `pub const TETO*`. A V1 morava num `tests/*.rs` e **escapava dos dois
  crivos** — nem media, nem aparecia como buraco, que pelo critério escrito
  neste próprio documento a tornava promessa. Hoje a constante mora em
  `src/conferidor_fsync.rs`, quem mede é o exemplo `fsync-por-fecho` (que se
  descreve), e o teste `tests/catraca-fsync-por-fecho.rs` **roda o exemplo** e
  cobra o que ele reportou: uma conta só, num lugar só. A régua do `medir.py`
  **não** mudou — mudar a régua obrigaria a aposentar as outras quatro
  catracas junto, e não havia motivo para pagar isso;
* **o teste recusa medir com binário velho.** `cargo test --test
  catraca-fsync-por-fecho` compila o teste e **não** compila os exemplos: o
  medidor ficaria o da rodada passada, publicando o número de ontem — é a
  armadilha que já custou a esta casa uma rodada inteira de ganhos invisível
  na bancada. O teste compara a data do binário do exemplo com a de todo
  `.rs` de `src/` e `examples/`, e reprova nomeando os arquivos mais novos.

O que segue descreve a V1 e continua valendo como história do defeito.

### 5.1 O que a V1 mediu (04/09, antes do conserto)

**O defeito que motivou**: o fecho da janela de durabilidade
(`Table::sincronizar`, chamado por `descarregar_sujas_com` num `Table`
recém-reaberto) faz `fsync` em sete arquivos e deveria fazer em oito — o
`.reg` fica de fora porque `Volumes::sincronizar` só sincroniza volumes que
estão em `abertos`, e um `Table` que só abre para fechar a janela (sem ler
nem escrever nada antes) nunca tocou o volume do `.reg`: o cabeçalho vem de
um `std::fs::File::open` direto em `RegFile::abrir`, fora do cache de
`Volumes`. A guarda irmã, `fecho-da-janela-sincroniza-o-reg.rs`, prova esse
FATO (zero `fsync` no `.reg`); esta catraca mede o CUSTO do mesmo fecho —
quantos `fsync` ele gasta no total — e trava que ele não gaste mais do que
gasta hoje.

**Medido hoje** (`cargo test -p phxsql-store --test catraca-fsync-por-fecho`,
reexecutando o próprio binário sob `strace -f -y -e trace=fsync`): **7**
`fsync` por fecho de janela, constante em três escalas de semeadura — 20,
2.000 e 200.000 linhas —, porque o custo é por ARQUIVO e não por linha. São
eles: `.trash`, `.bin`, `.memo`, `.log`, `.reason`, `.ndx` (duas vezes — o
principal e o espelho de páginas sujas). Confirmado batendo com a sonda que
motivou a tarefa (`crates/phxsql-store/examples/sonda-do-fecho.rs`, sob
`strace` manual): mesmos sete arquivos, mesma ordem, zero `.reg`.

**Quatro dos sete não mudam nada com um `inserir` comum** — `.trash`,
`.reason`, `.bin` e `.memo` só escrevem em exclusão/coluna externa —, e é
essa a dívida que a catraca cobra: sincronizar arquivo que ninguém sujou
desde o último `sincronizar`. Ela só desce à medida que um conserto aprender
a pular esses `fsync` redundantes.

**Ela NÃO cobre o defeito do `.reg`, de propósito.** Esta catraca mede
DESPERDÍCIO (fsync de arquivo que não mudou), a guarda irmã mede CORREÇÃO
(fsync que falta e devia estar lá) — são dívidas independentes e um conserto
pode mexer numa sem mexer na outra. A consequência que fica registrada para
a frente do conserto: ligar o `fsync` que falta no `.reg` SOBE o número de
verdade de 7 para 8, e subir o TETO para acomodar isso quebraria a lei
("catraca só desce, nunca sobe") do mesmo jeito que subir
`TETO_TABELA_NA_MAO` de 24 para 43 teria quebrado — a saída é a mesma que já
tem duas ocorrências nesta tabela: **aposentar `TETO_FSYNC_POR_FECHO_V1` (7)
e fazer nascer `TETO_FSYNC_POR_FECHO_V2` (8) no mesmo commit que liga o
`fsync` do `.reg`**, nunca só subir o número.

**E foi exatamente isso que aconteceu**, no mesmo dia: o conserto entrou, o
número medido virou 8, a V1 saiu e a V2 nasceu. A previsão escrita aqui em
04/09 pela frente que criou a V1 se cumpriu sem uma linha de discussão — que é
o que uma catraca bem documentada compra.

### 6. `TETO_TEMP_DIR_SOLTO` — diretório de teste criado sem guarda

**O defeito que motivou** (pedido 150, medido em 07/09/2026): uma corrida da
bateria dos três crates de servidor deixava **265 diretórios** para trás em
`/tmp`, e o `/tmp` desta máquina já tinha **27.519** entradas acumuladas. O
padrão era sempre o mesmo — um ajudante de teste que devolvia só o `PathBuf`,
com o `remove_dir_all` na **entrada** (para o próximo achar limpo) e nenhum na
saída. Quem falhava no meio, que é o caso comum de um teste de asserção,
deixava tudo.

**O conserto**: um guarda com `Drop` — `apoio_teste::DirTemp` nos crates que
têm biblioteca, `tests/comum/mod.rs` nos testes de integração, e uma cópia
curta dentro do `phxsql-cli`, que é binário e não tem de onde importar. O
`Drop` roda também durante o desenrolamento de um *panic*, que é justamente o
que um `rm` no fim do corpo do teste nunca alcança.

**Por que a catraca, e não só o conserto**: porque o defeito já tinha voltado
sozinho. O `phxsql-store` fora convertido numa rodada anterior, e a frente do
`.fts` repôs três sítios sem que nada avisasse — **21 diretórios por corrida**,
medidos nesta mesma rodada. Conserto sem catraca dura até a próxima frente.

**O que ela conta**: toda ocorrência de `std::env::temp_dir()` em
`crates/*/src` e `crates/*/tests`, fora de comentário, menos as catalogadas em
`ISENTOS` — e a isenção é por arquivo **com a quantidade esperada**, para que
uma chamada nova num arquivo já isento também reprove. São 17 isenções hoje:
os cinco guardas, os seis usos do `mensagens.rs` que só montam caminho para
**ler**, o `versao.rs` que usa o `/tmp` como diretório de trabalho de um
processo filho, e os três do `restaurar.rs`, que são código de produção.

**O que ela não cobre, e é decisão**: `examples/` fica de fora. Um exemplo é
um medidor chamado à mão ou pela bancada, não a bateria, e vários guardam o
que criaram justamente para se olhar depois. São **48 sítios em 42 arquivos**
— contados, não medidos em disco: medir exigiria rodar cada exemplo, e alguns
levam minutos. O item ficou no `PENDENCIAS.md` com esse número e com o que
falta medir.

**A prova real, nos dois sentidos**: com um `std::env::temp_dir()` reposto no
`transacao.rs`, a catraca ficou **vermelha** nomeando arquivo e linha; sem
ele, verde. E a medição fechou o laço: **265 → 0** nos três crates de
servidor, **21 → 0** no store, com os mesmos 813 e 1.669 testes passando.

### 7. `TETO_VERMELHA_SEM_PEDIDO` — guarda desligada que ninguém acha

**O defeito que motivou** (achado na revisão completa de 07/09/2026): esta casa
tem a boa prática de entregar a guarda **vermelha** quando o defeito é real e o
conserto é decisão do dono — escreve-se o teste que falha com o defeito de pé,
marca-se `#[ignore = "VERMELHA de proposito: …"]`, e o defeito fica provado
enquanto espera. Havia **duas** delas na árvore, e **nenhuma** estava no
`docs/PENDENCIAS.md`:

| Guarda | Desde | O que prova |
|---|---|---|
| `coluna_externa_marcada_sozinha_nao_pode_ir_em_claro` | 05/09 | tabela cujas únicas colunas marcadas são `Memo`/`Bin` nasce **em claro** com o cofre ligado — `.memo` 6.264 B legível |
| `tabela_que_nao_abre_nao_pode_encolher_a_posicao_em_silencio` | — | `posicao_do_diario` descarta o erro de abrir e encolhe a posição que a **eleição do cluster** compara |

**Uma prova vermelha desligada é a forma mais educada de esquecer um defeito**:
a bateria fica verde, o `cargo test` diz «0 falharam», e o defeito não aparece
em nenhum lugar que alguém leia. *Papel que não está cumprindo tem de aparecer
como não cumprindo* — e ali ele não aparecia. Abriram os pedidos 210 e 211.

**Hoje as duas estão VERDES**: o 210 fechou em 08/09 (a tabela nasce cifrada) e o
211 em 10/09 (a posição sai marcada `incompleta` e a eleição prefere a completa).
A árvore ficou **sem nenhuma prova vermelha** — que é o estado desejado, e não um
defeito. A catraca segue de pé para a **próxima** vermelha: guarda vermelha nova
entra com o pedido no mesmo commit, ou a catraca desce.

**O que ela conta**: cada `#[ignore = "VERMELHA de proposito…` em
`crates/*/src` e `crates/*/tests`, com o nome da função logo abaixo, exigindo
que esse nome apareça no `PENDENCIAS.md`. A lista de arquivos sai do disco.

**O que ela NÃO conta, e é decisão**: os `#[ignore]` comuns. O vetor de
1.000.000 de iterações do X25519 leva minutos, e o corpo traçado da sonda do
fecho só roda reexecutado por outro teste — os dois são ignorados por **custo**,
e não por defeito. Misturá-los encheria a catraca de ruído e a faria parar de
significar «há defeito conhecido aqui», que é a única coisa que ela diz.

**A prova real, nos dois sentidos**: com um nome de função inventado a catraca
fica **vermelha** nomeando arquivo, linha e função; com um nome que está mesmo no
`PENDENCIAS.md`, verde. E o casador tem controle próprio
(`o_casador_enxerga_uma_vermelha_sintetica`), porque um casador que parasse de
reconhecer a marca continuaria imprimindo «0 sem pedido» — o zero que não prova
nada. Esse controle **mudou no 211**: antes ele fazia `varrer()` e exigia achar
uma vermelha viva na árvore — e no dia em que o projeto consertou a última
(justamente a posição do diário), passou a falhar por **sucesso**. Agora prova o
casador contra um fonte sintético montado no próprio teste, e vale com a árvore
vazia ou cheia.

### 8. `TETO_INVENTARIO_DESCASADO` — o `.fts` que faltava em três lugares

**O defeito que motivou** (pergunta do dono, 07/09/2026, pedido 213): *«onde
fica os .fts? por que o dossiê não tem no gráfico: Organograma dos
arquivos?»*. Medido antes de mexer: o `.fts` faltava na **Figura 1** do dossiê
(o organograma dos arquivos de uma tabela), na **Figura 8** (o caminho de uma
inserção) e na **tabela-mestra do `docs/FORMATO.md`** — as três escritas à
mão, e nenhuma saindo do código. E o código **também** estava errado:
`EXTENSOES_TODAS`, em `phxsql-store/src/catalogo.rs`, não tinha `.fts` —
a mesma armadilha da peça nova no fim de uma lista que já tinha acontecido
duas vezes ali (seis para nove, depois nove para dez), só que desta vez em
silêncio: `excluir_tabela` e `renomear_tabela` deixavam o `.fts` órfão (sob um
nome que não existe mais, no renomear; vazando um índice de texto de uma
tabela apagada, no excluir), e `arquivos_da_tabela` mentia dizendo que a
tabela não tinha índice de texto nenhum.

**O conserto tem duas pernas.** A primeira é `EXTENSOES_TODAS` ganhar o `.fts`
(via `EXT_FTS`, de `crate::fts` — uma fonte só, não uma segunda string "fts"),
com um acessor público (`Database::extensoes_de_uma_tabela()`) para quem
precisa da lista como referência. A segunda é esta catraca: um conferidor
(`crates/phxsql-server/src/conferidor_inventario.rs`) que lê essa lista e
confere que toda extensão aparece na Figura 1, na Figura 8 (menos as que
`FORA_DA_INSERCAO` documenta como legitimamente ausentes — `.trash` e
`.reason`, que só nascem numa exclusão, nunca numa inserção) e na
tabela-mestra do `FORMATO.md`, reprovando com o nome da extensão e o lugar
que faltou — ou que sobrou, se uma cópia citar uma extensão que o código não
tem.

**Por que a Figura 8 tem uma exceção, e as outras duas não**: a Figura 1 e a
tabela do `FORMATO.md` prometem o inventário COMPLETO ("SEMPRE os sete, SÓ ÀS
VEZES quatro"); a Figura 8 desenha o caminho de UMA INSERÇÃO, e uma inserção
não toca `.trash` nem `.reason`. Exigir os dois ali forçaria a figura a
desenhar uma mentira só para agradar o conferidor. A lista de exceções é
curta, com o motivo escrito ao lado — o mesmo molde do `ISENTOS` do
`conferidor_temporarios`: isenção por nome e com motivo, para que a próxima
extensão que só nasça numa exclusão entre por decisão, não por a figura ter
esquecido dela calada.

**Medido hoje** (`cargo run --release --example inventarios-descasados -p
phxsql-server`): **11** extensões no código (`reg, ndx, bin, memo, log, bkp,
trash, reason, pag, lgpd, fts`); **11** na Figura 1; **9** na Figura 8 (as 11
menos as 2 isentas); **11** na tabela-mestra do `FORMATO.md`; **0**
descasamentos.

**A prova real, nos dois sentidos**, contra o dossiê de verdade (não só texto
sintético): trocando as cinco ocorrências de `.fts` por `.zzz` dentro do
bloco da Figura 1 do `dossie-phxsql-0.18.html`, a catraca ficou **vermelha**
nomeando os dois lados — `fts -- falta em Figura 1` e `zzz -- sobra,
desconhecida em Figura 1`; devolvido o arquivo original, verde de novo. E o
mesmo vale para o código: com `EXT_FTS` fora de `EXTENSOES_TODAS` (o estado
antes do pedido 213), os três testes que provam `excluir_tabela`,
`renomear_tabela` e `arquivos_da_tabela` ficam vermelhos, nomeando o `.fts`
órfão.

### 9. `TETO_BOTAO_SEM_PROVA` — botão de tela sem prova de clique

**O defeito que motivou** (pedido 190): *interface só se prova exercitando*. Um
botão pode quebrar sem que a leitura do código perceba — foi o que a coluna de
sistema `rownum` fez com *todo salvar e todo incluir* pela tela, e o que só um
vídeo achou em cinco minutos. Ler o `onclick` não prova que o botão funciona;
clicá-lo, sim. Sem catraca, cada tela nova traz botões que ninguém exercita, e
a regressão fica escondida até alguém clicar em produção.

**O que ela conta** (`crates/phxsql-server/src/conferidor_botoes.rs`,
`sem_prova()`): todo botão da tela (`<button>` e `role="button"`) cuja chave
estável (`#id`, `data-*` ou classe-gancho) **não** é clicada por nenhum caso da
bateria de navegador (`testes-web/casos/`) e **não** está em `DISPENSADOS` com o
motivo. Botão sem chave estável não dá para provar nem dispensar — entra na
conta como dívida até ganhar uma.

**O PISO, que obriga a descer junto da conversão**: além de `medido <= teto`, o
teste cobra `faltam.len() + 15 >= teto` — quem escreve dez casos e não baixa o
teto no mesmo commit deixa a catraca frouxa, e o piso reprova isso. É o mesmo
lado-da-folga que a §5 (fsync) tem.

**Aposenta, não sobe.** Se a régua passar a enxergar mais botões (hoje ela não
vê, por exemplo, um handler ligado por `addEventListener` sem chave no HTML),
nasce uma `..._V2` no número medido do dia, e esta é aposentada — a mesma lei
das §1 e §5, nunca subir um teto com o motivo ao lado.

**Medido hoje** (11/09/2026, `cargo run --example botoes-sem-prova -p
phxsql-server` — lê a evidência gravada em `testes-web/casos/`, não abre
navegador): **310** botões (308 `<button>` + 2 `role="button"`; 229 por `#id`,
69 por `data-*`, 11 por classe-gancho, 1 sem chave estável); **109** clicados
pela bateria; **9** dispensados com motivo; **194** sem prova — **teto 194,
folga 0**. Ela **nasceu em 211** (05/09/2026), num dia que começou em **268** de
298 botões com a bateria clicando 28; hoje clica 109. Só desceu.

**A prova real, nos dois sentidos**: `o_conferidor_acha_o_que_promete` prova que
o conferidor PEGA — um zero por engano (conferidor quebrado) é pior que um
número alto. Mais quatro guardas de crivo blindam os falsos: `a_chave_nao_e_a_frase`
(a chave é o `#id`, não o texto), `a_interpolacao_nao_fecha_a_etiqueta_cedo`,
`classe_de_estilo_nao_e_chave` e `a_lista_de_ganchos_sai_do_codigo`. E dois
laços fecham a evidência dos dois lados: `nenhuma_dispensa_morta` (dispensa que
não bate mais em botão nenhum) e `nenhuma_chave_morta_na_evidencia` (caso
gravado para um botão que não existe mais) — chave morta é pior que chave
faltando, a mesma lição da fábrica de idiomas.

## 10. O portão dos geradores — o derivado velho que anuncia sucesso pelo silêncio

**O defeito que motivou** (revisão de gaps, 11/09/2026): a pasta `docs/dossie/`
tem **catorze** geradores que escrevem TODO número visível do dossiê e das cinco
páginas satélites, e a rodada podia **esquecer de rodá-los**. Quando esquecia, o
painel publicado ficava com o número de ontem — ninguém digitou errado, e mesmo
assim a vitrine mentia. Não havia um portão único que reprovasse esse estado; a
única defesa era a disciplina de lembrar, e disciplina não é catraca. O custo já
estava medido nesta casa: em 07/09/2026, **três painéis atrasados sem um único
dígito digitado** (198 pedidos onde eram 203, 428 testes onde eram 451, 26.762
linhas/s onde eram 37.810).

**O que ele faz** (`docs/dossie/portao-dos-geradores.py`): para cada gerador,
responde a uma pergunta — *re-rodar mudaria algum número visível?* Roda o gerador
de verdade, guarda os bytes do alvo ANTES, compara com o DEPOIS, e **devolve os
bytes originais** (é read-only: confere, não conserta). Duas doenças são
VERMELHO: **derivado que mudaria** (a desatualização) e **gerador que FALHA**
(saída ≠ 0 — emitir nada quando a fonte sumiu é a mesma doença do conferidor que
diz «limpo» sem ter conferido). Reprova com saída ≠ 0, nomeando gerador, arquivo
e o primeiro trecho que difere.

**É portão binário, não catraca numérica** — não tem `TETO`, pelo motivo da
seção «O que é catraca»: um derivado velho é zero-ou-não-zero, não uma contagem
que desce.

**E é por «gerador que FALHA» que entra a guarda do pipe cru (17/09/2026).** O
`pagina-dos-pedidos.py` passa por toda linha de pedido do `PENDENCIAS.md`, e
agora **recusa** a que tenha um `|` não escapado dentro de uma célula, nomeando
a linha e o número do pedido. O defeito que a motivou é do naipe mais difícil de
achar, porque o gerador ficava **verde**: o regex dele ancora o grupo do texto
no fim da linha (`(.*?)\s*\|\s*$`), então engole o pipe a mais sem reclamar —
quem quebra é o renderizador do GitHub, que corta a célula ali e joga o resto do
texto para colunas que não existem. **Quatro linhas** estavam assim (os pedidos
175 e 311, mais duas escritas nesta rodada), e nenhuma apareceu em portão nenhum
até alguém contar os pipes. A cura é `\|`; **crase não protege**, porque em
tabela do GitHub o corte da célula acontece antes do trecho de código — foi
exatamente o caso do 311, cujo `${esc(s.versao || "")}` estava entre crases e
quebrava do mesmo jeito. Prova real nos dois sentidos, medida no dia: com o
defeito reposto na linha 335 o gerador sai com código **1** nomeando o pedido
311; desfeito, passa. E ela não precisou de máquina nova — cai no VERMELHO de
«gerador que FALHA» que este portão já tinha.

**Os três modos, e por que três e não um** — cada gerador leva o crivo mais
forte que suporta, e fingir que todos suportam byte-cru faria o portão gritar
VERMELHO sem defeito, que é o portão em que ninguém acredita:

- **`exato`** — o alvo é função pura das fontes versionadas (pedidos, cobertura,
  bancada, tetos, comparativo, fluxo, numeração de figuras, capturas). Byte a
  byte, zero máscara, zero VERDE-falso. É o núcleo forte.
- **`sem-carimbo`** — o gerador embute um CARIMBO DE PROCEDÊNCIA que muda sozinho
  a cada corrida e **não é número medido**: o relógio de parede do «gerado em», a
  data de hoje do «contados hoje», o `mtime` do arquivo lido (que o `git
  checkout` reescreve para a hora do checkout — o git não preserva mtime) e o
  commit curto do rodapé. Comparar isso por byte dá VERMELHO em toda árvore,
  sempre, sem defeito nenhum. Este modo apaga só o carimbo dos dois lados e
  compara o resto — pega mudança de VALOR medido, ignora a data. A máscara é
  cirúrgica: o `commit <code>…</code>` do rodapé some, mas o build-id
  `(41e82efa97c8)` que a página de testes mostra **fica**, porque é valor medido.
  São `sem-carimbo`: `perguntas`, `pagina-de-status`, `pagina-dos-testes`,
  `graficos`.
- **`nota-cargo`** — o `numeros-do-projeto.py` chama `cargo test` e `cargo run
  --example`, e esta worktree tem disco escasso. Ele **não é rodado** pelo
  portão; sai como NOTA, com o comando para rodá-lo à mão. *Papel que não está
  cumprindo aparece como não cumprindo*, em vez de sumir do relatório.

O `trio-de-motores` é `exato` com um ajuste: a guarda própria dele compara o
`mtime` da figura com o da medição, e essa comparação **não sobrevive a um
checkout** (as duas saem com a hora do checkout). O portão põe a figura como a
mais nova antes de rodar — reproduz a pré-condição que a receita real garante
rodando o `grafico.py` — e confere o bloco. A frescura figura-vs-medição fica a
cargo da guarda do trio, na receita real; o portão não a reproduz, e isto está
dito no fonte.

**A prova real, nos dois sentidos** (11/09/2026, gerador barato `tetos-da-trava`,
para não furar o piso de disco): com o portão VERDE, editei à mão um número
medido no bloco `tetos:` do dossiê (`RwLock` por_lote `1,54x` → `1,49x`) — o
portão ficou **VERMELHO** nomeando `tetos-da-trava.py`, o dossiê e o hunk exato,
e saiu com código 1; rodado o gerador de verdade, **VERDE** de novo, saída 0. E
o portão restaurou o defeito depois de acusá-lo, provando o read-only.

**E ele pagou por si na primeira corrida completa**, achando três derivados
velhos no próprio HEAD (`6858fa4`) que nenhuma leitura acharia: `pagina-dos-pedidos`
(226 feitos onde eram **227**, 11 planejados onde eram **10**), `cobertura-por-area`
(150 testes em «Servidor (outros)» onde eram **151**, e a linha inteira do
**Cluster** faltando) e `pagina-dos-testes` (a página ainda anunciava **1 guarda
VERMELHA** que o commit 211 já consertara — pega pelo modo `sem-carimbo`, que viu
o valor mudar por baixo do carimbo de mtime). Os três ficaram para o passo de
integração regenerar num só golpe, com os mtimes certos e o `cargo` à mão — uma
regeneração parcial aqui trocaria as datas de medição de `testes.html` pela hora
do checkout, que é o defeito que a própria página existe para não cometer.

**O que ele NÃO cobre, e é decisão**: (a) o `numeros-do-projeto.py`, por
`nota-cargo`; (b) uma DATA que envelheceu sozinha nas quatro páginas
`sem-carimbo` — mas essas datas vêm de `mtime`/hoje/agora e não se reproduzem
entre checkouts de qualquer jeito; o que o modo garante é o VALOR medido; (c) a
frescura figura-vs-medição do trio, que é guarda de mtime do próprio trio.

## 11. `TETO_PKILL_SEM_PID` — bancada matando o servidor de outra frente

**O defeito que motivou** (pedido 256, achado pelo papel F na bancada «chutar
a tomada», lendo as bancadas irmãs para reaproveitar o molde, 16/09/2026):
`bancada/carga/bulkinsert.py` chamava `pkill -x phxsqld` para subir e para
derrubar o servidor da própria bancada. `pkill` nunca aceita PID — ele mata
por NOME ou por PADRÃO na máquina **inteira** —, então a chamada derrubava o
`phxsqld` de **qualquer** outra frente ou bancada viva ao lado, contra a regra
da casa: o `zelador.sh` nem mata processo (o processo pode ser de outro
agente), e `prova-bateria.py`/`chutar-a-tomada.py` já matavam só o PID que
subiram. O irmão `bancada/carga/medir.py` (mesma pasta, mesma ordem de
chamadas) tinha o **mesmo defeito**, achado ao ler o caminho que motivou —
"conserto entra no caminho que o motivou, e o caminho IRMÃO fica" existe
justamente para não repetir esse.

**Por que aqui, e não em `bancada/guardas/catalogo.py`**: o catálogo de
guardas prova defeito reposto em código RUST — o executor
(`provar-guardas.py`) copia só `Cargo.toml`, `Cargo.lock`, `crates/`,
`exemplos/`, `docs/` e `testes-web/` para rodar `cargo test`; `bancada/` não
está nessa lista, então uma entrada cujo `arquivo` morasse lá nunca rodaria —
o executor nem a copiaria. **Alcance da pétrea "cada guarda catalogada",
medido nesta rodada**: o catálogo cobre `crates/`, não `bancada/`. Uma dívida
de script pede outro dono — a catraca, que é o que esta seção é.

**O que ela conta**: toda invocação de verdade do comando `pkill` em
`bancada/**/*.py` e `bancada/**/*.sh` — uma string entre aspas RETAS
(`"pkill"`/`'pkill'`, do jeito que um `subprocess.run([...])` python passa um
argv) ou uma linha de shell que **começa** (fora de comentário) com o
comando. O próprio arquivo do conferidor se exclui da varredura — ele precisa
escrever `"pkill"` entre aspas para IMPLEMENTAR o crivo, e sem a exclusão se
acharia, a mesma armadilha do `pgrep -f` que `esta-medindo.sh` documenta.

**O que ela NÃO conta**: a palavra `pkill` dentro de comentário ou docstring
— esta casa sempre escreve isso entre CRASE (`` `pkill -f` ``), nunca entre
aspas retas, exatamente para que "nunca pkill" no comentário não se confunda
com o uso. Um arquivo pode e deve continuar dizendo isso acima do `Popen` que
prova a promessa.

**Onde mora**: `bancada/guardas/pkill-sem-pid.py`, chamada pelo item 0b da
bateria (`bancada/bateria/prova-bateria.py`) — estática, sem servidor. Desde
16/09/2026 ela também responde a `--numeros` e entra na tabela gerada do
`docs/QA-PDCA.md`: ela era a **oitava** catraca de `bancada/` que o inventário
não contava, e a única que o próprio parágrafo da §13 esquecia de nomear
(§14).

**Medido hoje** (16/09/2026, depois do conserto de `bulkinsert.py` e
`medir.py`): **0** invocações reais de `pkill` em toda `bancada/`. Teto 0,
folga 0 — nasce colada, como as quatro do dia 03/09.

**A prova real, nos dois sentidos**: com as versões de `bulkinsert.py` e
`medir.py` de antes do conserto repostas (as duas, do commit anterior a este),
a catraca acusa **SUBIU 4 (teto 0)**, nomeando as quatro linhas; com o
conserto, `ok 0 (teto 0)`.

## 12. As sete réguas do catálogo de guardas — seis tetos e um piso

**O defeito que a motivou** (pedido 263, 16/09/2026): a corrida inteira do
`provar-guardas.py` devolveu **11 guardas QUEBRADAS** — nem provadas nem
reprovadas: o `trecho` que a entrada manda substituir para repor o defeito
**não existe mais** no arquivo, então a guarda não pode nem ser tentada.
Guarda que existe e não guarda é pior que guarda faltando, porque o catálogo
a conta como cobertura.

Medido commit a commit depois: **um único commit aposentou cinco delas de uma
vez** — `2fe8658` (12/09, «a conferência de FK dentro da transação vê o pai
empilhado»), que mexeu em `table.rs` e `transacao.rs`. Ninguém percebeu por
**quatro dias**, e o motivo é o custo: o provador leva cerca de uma hora,
porque repõe o defeito e roda `cargo test` para cada uma das 169 entradas.
**Guarda que só se confere em uma hora é guarda que não se confere.**

**Onde mora**: `bancada/guardas/trecho-vivo.py`, chamada pelo
`bancada/catracas/todas.py` (§18), que a bateria roda no item 0 e o
`comunicacao.sh` a cada batimento — estática, sem servidor e sem
compilar nada.

### 12.1 As seis formas de QUEBRADA, e quais entraram na régua

A régua nasceu vendo **uma** das cinco, e o número que isso custou está
medido: no mesmo dia 16/09 ela dizia `ok 0` enquanto o provador dizia **1
QUEBRADA** (a `trava-sem-guarda-de-reentrancia`, que estoura o prazo de
420 s). Ela estava certa no que prometia, e o `LEIA-ME` dizia isso — mas
**quem lesse o `ok 0` como inventário concluiria que o catálogo estava
inteiro**. É a lei da casa outra vez: lei que lista menos casos do que existem
protege igual hoje e menos no dia em que alguém usar a lista como inventário.

O critério de quem entra é um só: **dá para ver sem compilar e sem rodar?**

| forma de QUEBRADA | onde nasce no provador | na régua? |
|---|---|---|
| o arquivo/o trecho não está mais lá | `Arvore.repor`, `quantas == 0` | **sim** — `TETO_TRECHO_MORTO` |
| o trecho aparece **duas** vezes | `Arvore.repor`, `quantas > 1` | **sim** — `TETO_TRECHO_AMBIGUO` (nova) |
| o teste nomeado não existe mais | `julgar`, `sumidos` | **sim** — `TETO_TESTE_MORTO` |
| o teste existe, mas **não no binário** | laço principal, `faltando` | **sim** — `TETO_TESTE_FORA_DO_BINARIO` (nova) |
| o nome vem **sem o módulo**, num `--lib` | `julgar`, `sumidos` — o nome curto não está na saída do cargo | **sim** — `TETO_TESTE_SEM_MODULO` (17/09, pedido 273, §12.7) |
| o código trocado **não compila** | `julgar`, `desfecho == "nao compilou"` | **não** |
| a rodada **estourou o prazo** | `julgar`, `desfecho == "prazo"` | **não** |
| o binário **abortou** sem ser esperado | `julgar`, `desfecho == "aborta"` | **não** |

As três de baixo ficam de fora **por definição**: só existem depois de o
`cargo test` compilar o `troca` e RODAR o binário. Ver o `troca` compilar
custa uma compilação por entrada — que é exatamente a hora do provador que
esta régua existe para não esperar — e ver o prazo e o aborto custa a rodada
inteira.

**E a régua diz que ficam de fora, em toda corrida.** O `--catraca` fecha com
a lista «o provador continua dono de: …» e a frase *«um `ok` aqui NÃO diz que
o catálogo está inteiro»*. Não é prosa de rodapé: é o inventário do buraco
impresso **junto do número**, para que ninguém precise abrir o fonte para
saber o que o zero não cobre.

### 12.2 O piso — a catraca não distinguia conserto de APAGAMENTO

Buraco medido em 16/09/2026, depois de as oito entradas velhas serem
consertadas: `TETO_TRECHO_MORTO` conta **trecho morto**, não **guarda viva**.
**Apagar as oito entradas do `catalogo.py` teria medido exatamente o mesmo
`0` que consertá-las** — e apagar é o caminho barato. Uma catraca que premia
o apagamento igual ao conserto não segura o catálogo; segura a aparência dele.

`PISO_DAS_ENTRADAS` fecha esse lado, e **conta vivas + aposentadas
escritas**, não só as vivas. A diferença é a armadilha que um piso rígido
teria: ele impediria **aposentar** uma guarda cuja lógica deixou de existir, e
guarda impossível de aposentar vira entrada remendada no chute — que esta casa
trata como pior que a quebrada.

A saída é a mesma lei da catraca que muda de régua: **a aposentadoria se
escreve.** Quem tira uma entrada põe uma linha em `APOSENTADAS` com o id, a
data e o motivo, e a soma não se mexe; quem apaga em silêncio faz a soma cair,
e o piso reprova dizendo quantas sumiram. **O piso muda o preço relativo dos
dois caminhos**: consertar continua custando ler o código, e apagar passa a
custar escrever por quê.

E ele **sobe junto**, pelo mesmo motivo que o teto desce junto: catálogo que
cresceu e piso parado é piso frouxo — voltaria a aceitar o apagamento das
entradas novas. Crescer reprova pedindo o número novo no mesmo commit, que é o
espelho exato do «DESCEU — BAIXE O TETO». Uma entrada de `APOSENTADAS` que
**volte** ao catálogo também reprova: ela contaria dos dois lados e inflaria o
piso em silêncio.

### 12.3 Os sete números, medidos em 16/09/2026 (a sétima em 17/09)

| Régua | Lado | Valor | Medido | Nasceu |
|---|---|---:|---:|---|
| `TETO_TRECHO_MORTO` | teto | 0 | **0** | 16/09, em 8; desceu para 0 no mesmo dia |
| `TETO_TRECHO_AMBIGUO` | teto | 0 | **0** | 16/09, nesta frente |
| `TETO_TESTE_MORTO` | teto | 0 | **0** | 16/09 |
| `TETO_TESTE_FORA_DO_BINARIO` | teto | 0 | **0** | 16/09, nesta frente |
| `TETO_TESTE_SEM_MODULO` | teto | 0 | **0** | 17/09, pedido 273 — depois do conserto dos três nomes da §15.7.7; §12.7 |
| `TETO_NAO_JULGADA_ESCONDIDA` | teto | 0 | **0** | 16/09, pedido 269: nasceu medido em **26** e desceu para **0** no mesmo passo, republicando a corrida de 15:25 |
| `PISO_DAS_ENTRADAS` | **piso** | 177 | **177** | nasceu 16/09 em 143; **subiu para 145** (frente vizinha, no mesmo dia), para **151** na frente 245/O2–O6, para **160** na frente G-CRIPTO (§15), para **169** na frente G-SENHA (§15.7), para **170** com o `Debug` do DbLink (17/09 — a constante subiu e esta linha ficou em 169 até a frente seguinte) e para **177** na segunda leva da pétrea da senha (§15.7.7) — 177 entradas vivas + 0 aposentadas. Piso só sobe, e sobe no mesmo passo em que o catálogo cresce |

**Nenhum teto subiu e nenhuma catraca se aposentou, e isso é decisão.** A
régua do `TETO_TRECHO_MORTO` **não mudou**: ela continua respondendo
exatamente «o trecho está lá?», e as duas perguntas novas nasceram em
**catracas ao lado**, cada uma no número medido do dia. Fosse o contrário —
alargar a régua do `TETO_TRECHO_MORTO` para contar também o trecho ambíguo —,
a lei mandaria aposentá-la e fazer nascer uma `_V2`, perdendo a série com o
8 → 0 de hoje de manhã. Catraca nova ao lado custa um nome; alargar a velha
custa a série. **Quando as duas saídas existem, a que não mexe na régua é a
certa.**

E as duas listas de teste são **disjuntas de propósito**: um nome que não
existe em lugar nenhum entra só no `TETO_TESTE_MORTO`; o
`TETO_TESTE_FORA_DO_BINARIO` conta só o que existe em `crates/` e **não** está
no binário que a entrada nomeia. Sem isso, um renomear subiria dois números e
pareceria dois defeitos.

### 12.4 O custo, medido a cada passo

**Régua cara é régua que não se roda**, e esta roda em toda bateria. As duas
perguntas novas, escritas do jeito óbvio, quase a tiraram de lá:

| versão | parede (5 corridas, load ~1,0) |
|---|---|
| a régua de hoje de manhã, uma pergunta | 0,171–0,183 s |
| as quatro perguntas, relendo `src/` por pacote | 0,47 s |
| uma passagem só, guardada por arquivo | 0,32 s |
| o `catalogo.py` guardado e o `achados()` uma vez, não duas | 0,29 s |
| o crivo de `mod x;` só nos 57 arquivos de `tests/`, dos 276 | **0,200–0,206 s** |

Duas perguntas a mais por **0,03 s**. Os três consertos foram medidos um a um
e não no fim — a conta que diz *qual* deles pagou.

### 12.5 A prova real, nos dois sentidos, régua por régua

Todas em 16/09/2026, com o defeito reposto em `crates/phxsql-server/src/`
**acima** do `#[cfg(test)]` — a lição que a frente do mapa das threads pagou
no mesmo dia: o fim de um arquivo Rust quase sempre é território de teste, e
defeito reposto ali não é defeito reposto.

| régua | defeito reposto | o que ela disse |
|---|---|---|
| `TETO_TRECHO_MORTO` | a fórmula do `deve_girar` quebrada em três linhas | `SUBIU 1 (teto 0)`, nomeando `rodizio-do-profiler-ignora-o-zero` |
| `TETO_TRECHO_AMBIGUO` | a mesma fórmula **duplicada** numa segunda `pub fn` de produção | `SUBIU 1 (teto 0)`, «o trecho aparece 2 vezes» |
| `TETO_TESTE_MORTO` | `teto_zero_nunca_manda_girar` renomeado | `SUBIU 1 (teto 0)`, nomeando o teste |
| `TETO_TESTE_FORA_DO_BINARIO` | o mesmo teste **movido** de `src/` para `tests/` | `SUBIU 1 (teto 0)`, «não está em phxsql-server --lib» — e o `TETO_TESTE_MORTO` ficou em **0**, que é a disjunção provada |
| `TETO_TESTE_SEM_MODULO` (17/09) | o `dblink::testes::` tirado de um `caem` de `debug-da-ligacao-mostra-a-senha` no catálogo | `SUBIU 1 (teto 0)`, nomeando a entrada e o nome curto, saída 1 — e os outros quatro tetos em `ok 0`, que é a forma que só ela vê; restaurado, saída 0 |
| `PISO_DAS_ENTRADAS` | uma entrada apagada do `catalogo.py` | `ENCOLHEU 142 (piso 143)` — **e os quatro tetos continuaram `ok 0`**, que é o buraco de §12.2 visto acontecer |
| idem, a saída legítima | a aposentadoria escrita em `APOSENTADAS` | `ok 143`, com «142 guardas no catálogo + 1 aposentada escrita» |
| idem, a entrada de volta | a aposentada devolvida ao catálogo | `CRESCEU — SUBA O PISO 144` **e** `APOSENTADA QUE VOLTOU` |

Limpa, os cinco `ok` e código de saída 0.

**E uma prova que não precisou de compilação nenhuma**: para confirmar que o
trecho ambíguo é mesmo `QUEBRADA` no provador — e não uma classe que esta
régua inventou —, a própria função `Arvore.repor` do `provar-guardas.py` foi
chamada sobre o arquivo mutado, numa cópia de um arquivo só. Ela devolveu
*«o trecho aparece 2 vezes em `crates/phxsql-server/src/rodizio.rs`: trocar a
errada provaria outra coisa»*, que é literalmente o motivo que o laço
principal transforma em `QUEBRADA`. A correspondência entre as duas réguas
está **exercitada**, não afirmada.

**O que não foi exercitado de ponta a ponta, e fica dito**: a forma
`faltando` (teste fora do binário) não foi vista sair do provador com uma
corrida de verdade. A cópia dele (`~/.cache/phx-guardas`) estava vazia e uma
corrida `--so` custaria uma compilação fria de ~1,2 GB numa árvore com 7,9 GB
livres. A correspondência ali é **lida do fonte** (`provar-guardas.py`, o
`faltando` do laço principal), não medida — e a diferença entre as duas coisas
fica escrita em vez de sumir.

**E a armadilha que a medição já pagou**, porque é a lei da casa por outro
caminho: a primeira versão varria só `crates/<pacote>/src/` atrás dos testes e
acusou **154** nomes mortos. Eram 154 falsos — o teste de integração mora em
`crates/<pacote>/tests/`. Régua que mede um terço da caixa e anuncia o número
inteiro é o mesmo defeito do KiB da interface.

### 12.6 A quinta forma de envelhecer: a tabela publicada menor que o catálogo

**O defeito que a motivou** (pedido 269, 16/09/2026): as quatro réguas de cima
olham o **código** contra a entrada. Nenhuma olha a **entrada** contra a
**última corrida** — e uma entrada pode ter trecho vivo, teste vivo, teste no
binário certo e **nunca ter sido julgada**, bastando ter entrado depois da
última corrida do provador.

Medido: o catálogo tinha **160** entradas e a tabela publicada em
`docs/TESTES.md` dizia **«143 guardas»**. A página era honesta sobre a **data**
(traz o `medido em`) e **muda sobre o tamanho** — quem a lesse como inventário
a leria **17 entradas curta**. E nada no caminho recusava o pior caso: rodar o
provador com `--so` e publicar trocava 143 linhas por 9, escondendo 151, com
código 0 e uma linha de êxito. **Medido contra o código de então**, não
deduzido — e o estrago não era só de inventário: a linha de resumo passava a
publicar **332 s de mutação** onde a bateria custa **3.374 s**.

**Por que o teto conta o ESCONDIDO e não o buraco — e o motivo foi medido duas
vezes no mesmo serão.** O buraco cru era **17 às 21h** e **26 às 23h**, porque
uma frente vizinha escreveu nove guardas novas nesse intervalo. Não houve
defeito entre as duas medições: houve trabalho certo. Um teto sobre o buraco
cru ficaria vermelho toda vez que alguém escrevesse uma guarda, e os dois
caminhos para reverdecê-lo seriam rodar o provador inteiro (~3.374 s de
mutação mais a compilação) ou **subir** o teto — que esta casa proíbe. Catraca
cujo único caminho verde custa uma hora é catraca que se pula, e ela cobraria o
preço de quem **escreve** a guarda: o espelho exato da doença de §12.2.

Então o buraco é **inventário** e a dívida é **catraca**:

| o quê | o que é | onde aparece |
|---|---|---|
| **não julgadas** | entradas sem veredito na corrida publicada | **impresso** sempre, na linha «a última corrida julgou N de M» — nunca travado |
| **escondidas** | as não julgadas que a página **nem nomeia** | `TETO_NAO_JULGADA_ESCONDIDA`, que **só desce** |

A dívida se paga em **0,2 s**, sem prova nova e sem data nova, republicando a
**mesma** corrida: `python3 bancada/guardas/tabela-no-testes.py
bancada/guardas/ultima-corrida.json`.

**E ela vira parada com o motivo quando a fonte some.** Sem a
`ultima-corrida.json` não há corrida com que comparar; sem as marcas
`guardas:inicio`/`guardas:fim` não dá para saber o que a página nomeia. Nos
dois casos a régua reprova **com o motivo escrito e contando o pior caso** —
corrida ausente julgou zero, página ilegível nomeia zero. Régua que não sabe
tem de dizer que não sabe; o que ela não pode é devolver `0` calada e parecer
um catálogo inteiro.

**A outra metade, no gerador** (`bancada/guardas/tabela-no-testes.py`): ele
passou a **nomear** o que a rodada não julgou, sob um aviso que não é linha de
êxito, e a **recusar** encolher a tabela sem um `--parcial` escrito. A recusa
olha a **cobertura**, não o tamanho: *esta rodada julgou o catálogo inteiro?*
Se julgou, publica livre — e é assim que a **aposentadoria** de uma guarda
(§12.2) nunca vira parada permanente, sem precisar de um segundo interruptor.

**Custo**: +10 ms medidos (mediana 197 → 207 ms, sete corridas de cada lado,
mesma máquina, mesmo minuto, load 2,5).

**Prova real, nos dois sentidos**: `trecho-vivo.py --autoteste` (8 casos) e
`tabela-no-testes.py --autoteste` (16 casos), sem `cargo` e sem provador — e
oito mutações do código conferidas uma a uma, cada uma acusada pelo caso certo.
**Um dos casos passou com o defeito reposto na primeira escrita** (testava o
substring no sentido errado) e quem disse isso foi a mutação, não a leitura.

### 12.7 A sexta forma: o nome sem o módulo — e a régua que o pedido propunha estava errada

**O defeito** (17/09/2026, §15.7.7 item 2): `debug-da-ligacao-mostra-a-senha`
entrou como «provada 1/1» e estava QUEBRADA no provador — os três nomes de
`caem`/`seguem` vinham sem o `dblink::testes::`. O `julgar` compara com o que
o `cargo test` **imprime** (`test dblink::testes::nome ... ok`), e o nome curto
não está lá. `TETO_TESTE_MORTO` e `TETO_TESTE_FORA_DO_BINARIO` disseram `ok 0`,
porque procuram a `fn` no fonte, e a `fn` existe.

**A premissa do pedido morreu medida.** O pedido 273 mandava `"::" in nome`
para toda entrada de `caem`/`seguem`, «nascendo em 0». Contado no catálogo
antes de escrever: **164 nomes sem `::`**, todos em alvos `--test`, todos
certos — o cargo imprime o teste do topo de `tests/x.rs` **sem caminho
nenhum**, e é assim que o provador os casa. A régua crua nasceria em 164 e
mandaria consertar o que está certo. Num `--lib` é o contrário: todo teste
mora num `mod`, o cargo sempre imprime o caminho, e um nome sem `::` **nunca**
casa. Então a régua é **alvo `--lib` exige `::`**, e nasceu em **0**.

**O que ela não vê, e diz no próprio código:** o caminho *errado*
(`outro::testes::nome`) tem `::` e passa — continua com o provador, no mesmo
`sumidos`; e um teste de integração que more num `mod comum` e seja nomeado
sem o `comum::` também só o provador vê.

**Prova real nos dois sentidos**, além do `--autoteste` (cinco casos, com o
`--test` de nome curto como controle): o `dblink::testes::` tirado de um
`caem` no `catalogo.py` faz o `--catraca` sair `SUBIU 1 (teto 0)` nomeando
entrada e nome, código 1, com os outros quatro tetos em `ok 0`; restaurado,
código 0. Custo remedido com a sétima régua: **0,203–0,216 s** por corrida
(três corridas, load 0,08) — contra 0,200–0,206 s da versão de seis.

## 13. As cinco catracas dos dois mapas de concorrência — e onde cada uma passou a rodar

**O defeito que a motivou** (pendência #252, metade (2), 16/09/2026): as
catracas dos dois mapas de concorrência só rodavam no **item 0 da bateria de
ponta a ponta**, e a bateria é um comando que alguém tem de lembrar de dar. A
última corrida versionada era de **29/08**; a seguinte, de **16/09**. Dezoito
dias — e nos últimos **oito** a `alcancam-fsync` esteve furada (23 com teto
22, desde o merge `6245491` do PITR em 08/09 17:17), com **três rodadas de
integração de `fmt`, `clippy` e suíte verdes** sem que nada acusasse. É a mesma
doença do pedido 263, paga no mesmo dia por outro caminho: **guarda que só se
confere quando alguém lembra é guarda que não se confere.** Lá foram quatro
dias e onze guardas quebradas; aqui, dezoito dias.

**As cinco, e o que cada uma conta** (os tetos moram no `CATRACAS` de cada
medidor, e não numa cópia em Rust — duas contas divergem na primeira correção
feita numa só):

| Catraca | Medidor | Teto | Medido em 16/09/2026 | Estado |
|---|---|---:|---:|---|
| `codigo-do-dono` | `bancada/concorrencia/mapa-da-trava.py` | 5 | **5** | sem folga |
| `alcancam-fsync-2` | `bancada/concorrencia/mapa-da-trava.py` | *sai da tupla `alcancam-fsync-2` em `CATRACAS`, `mapa-da-trava.py:717-718`* | **23** (medido em 24/09/2026 por `python3 bancada/concorrencia/mapa-da-trava.py --numeros`) | **VERDE em 22/09/2026** — a `alcancam-fsync` (teto 22) foi **APOSENTADA** em 18/09/2026 por decisão do dono, e esta nasceu no número medido daquele dia. Régua que passa a medir outra coisa **aposenta** a catraca antiga e faz nascer uma nova; não se sobe teto com motivo escrito ao lado. Esta tabela publicou a aposentada como viva e **VERMELHA** por quatro dias — número digitado à mão envelhecendo dentro do documento que existe para dizer quais catracas seguram. O `2e51ac6` dizia ter consertado isso e alcançou o `QA-PDCA.md`: era o **irmão** que faltava. **E esta linha envelheceu de novo, na mesma hora do mesmo jeito**: escrevia «24 \| 24» quando o teto já tinha descido para 23 em 23/09/2026 (pedido 421) — `op_migrar_esquema` e `op_acrescentar_coluna` saíram da trava global (`mapa-da-trava.py:703-716`), e ninguém trocou o dígito. A frente do pedido 422 viu e **não tocou**, e acertou: esta tabela é retrato DATADO do papel G, não coluna derivada — o defeito nunca foi o número errado, foi o número **digitado em prosa** quando `mapa-da-trava.py --numeros` já o imprime (pedido 423, 24/09/2026). Corrigido trocando o dígito pela fonte — a tupla e o arquivo — em vez de por outro número que envelheceria do mesmo jeito. |
| `rede-ou-espera` | `bancada/concorrencia/mapa-da-trava.py` | 0 | **0** | sem folga |
| `spawn-sem-teto` | `bancada/concorrencia/mapa-das-threads.py` | 0 | **0** | sem folga |
| `catalogo-envelhecido` | `bancada/concorrencia/mapa-das-threads.py` | 0 | **0** | sem folga |

**O custo, medido antes de escolher o lugar — com a CARGA anotada ao lado**,
que é o que transforma este número em medida. Três corridas de cada, em
16/09/2026, numa máquina de **4 núcleos** com outras frentes compilando ao
lado. Tempo de **parede** medido sob carga é **teto superior**, não o custo da
régua: ele diz quanto a corrida demorou nesta máquina naquele minuto. O custo
da régua é o tempo de **CPU** (user+sys do processo filho), que não cresce
porque o vizinho compila:

| Medidor | parede @ load ~4,9 | parede @ load ~9,4 | parede @ load ~14,8 | **cpu** @ load ~9,4 / ~14,8 |
|---|---:|---:|---:|---:|
| `mapa-das-threads.py --catraca` | 1,122 / 0,885 / 0,811 s | 0,740 / 0,791 / 0,845 s | 0,726 / 0,680 / 0,708 s | **0,67 s** / **0,67 s** |
| `mapa-da-trava.py --catraca` | 3,113 / 3,056 / 3,187 s | 3,877 / 3,864 / 4,369 s | 4,469 / 3,457 / 3,077 s | **3,10–3,81 s** / **2,94–3,03 s** |

Reproduza com `python3 bancada/concorrencia/custo-das-catracas.py`, que imprime
parede, CPU e a carga antes e depois de cada corrida. Ele é versionado de
propósito: **roteiro que resolveu algo não pode morrer com a sessão**, e sem
ele a próxima pessoa que precisar decidir onde uma catraca mora vai medir de
novo do jeito que der naquele dia.

E o teste da suíte: **0,68 / 0,67 / 0,69 s** a load ~4,9 e **0,66 / 0,72 /
0,72 s** a load ~9 — a carga não o moveu.

Três leituras que uma corrida só não daria. A primeira: o `1,122 s` da corrida
inicial **não era carga, era cache frio** — a rodada com o triplo de carga saiu
mais rápida. A segunda: o tempo de **CPU do mapa das threads não se mexeu**
entre load 9,4 e 14,8 (0,67 s nas duas), que é o que se espera de uma régua que
só lê arquivo — a parede é que balança. A terceira, e a que decide: a **~3,7×
de sobrescrita** (4 núcleos a load 14,8) a pior corrida do mapa da trava foi
**4,469 s**, menos da metade dos dez segundos que separariam a suíte do
gerador. **A escolha do lugar é a mesma nas três cargas** — e é essa
invariância, não um número solto, que prova que não foi o custo que a
decidiu.

**Pelo custo as duas caberiam na suíte**: um segundo e três segundos somem
dentro de um `cargo test --workspace` que leva minutos. **O que separou as
duas não foi o número — foi o estado**, e dizer isso importa mais que o
número: uma catraca **vermelha por decisão do dono** não pode virar portão.
Posta na suíte, ela deixaria a suíte de **todas** as frentes vermelha até ele
decidir, e a saída mais barata dessa pressão seria subir o teto de 22 para 23
— exatamente o que a pétrea proíbe. **Catraca só desce.**

**Onde cada uma passou a rodar:**

- **Mapa das threads** (verde) → `cargo test`, em
  `crates/phxsql-server/tests/catraca-do-mapa-das-threads.rs`. O teste **roda
  o medidor** e cobra o veredito dele: uma medição só, num lugar só — é o
  molde do `catraca-fsync-por-fecho.rs` do `phxsql-store`. Medido dentro da
  suíte: **0,68 / 0,67 / 0,69 s** (load ~4,9) e **0,66 / 0,72 / 0,72 s**
  (load ~9). Sem `python3` ele **falha dizendo que não conferiu**, nunca passa
  calado: guarda que não roda tem de dizer que não rodou.
- **Mapa da trava** (vermelha) → `docs/dossie/numeros-do-projeto.py`, que roda
  a cada rodada. Ali ela é **relato com data, não portão**: a reprovação sai
  antes dos minutos de `cargo test` e **de novo na última linha**, que é a que
  se lê. O gerador **não muda de código de saída** por causa dela — fazê-lo
  poria o `portao-dos-geradores.py` vermelho a cada rodada com a mensagem
  errada («gerador que não emite quando a fonte sumiu»), e sinal falso é o que
  esta casa pune. No dia em que a #252 (1) for decidida, ela entra na suíte do
  mesmo jeito que a das threads.
- As duas continuam no **item 0 da bateria** e no aviso de hora em hora
  (`comunicacao.sh`, desde `560c63c`, 16/09 07:09 — uma frente anterior desta
  mesma rodada já as tinha posto lá). Nenhum desses dois é portão: a bateria é
  um comando que alguém dá, e o `comunicacao.sh` imprime `⚠️` sem código de
  saída, além de depender da corrente do batimento fino, que o próprio arquivo
  registra ter ficado ~3 h parada.

**O que continua vermelho, e continua vermelho**: `alcancam-fsync` **25, teto
22** (23 em 16/09; remedida em 17/09/2026 pelo pedido 164 — ver §13-bis: a
`abrir_travada_sem_sobrepor` nova tem só 2 chamadores, fica abaixo do corte de
«porta comum», e o mesmo `fsync` que antes vinha herdado passa a contar como
próprio de cada um — nenhum `fsync` novo entrou na trava). Nada aqui mexeu no
TETO, nem na seção crítica que a furou. A decisão é do dono (#252, metade (1)):
ou a catraca ganha a exceção nomeada «operação administrativa que troca o
banco inteiro» — o que a **aposenta** e faz nascer outra no número medido do
dia, como manda a lei da régua —, ou a reaplicação do diário sai da seção
crítica.

**A prova real, nos dois sentidos** (16/09/2026, com o binário do teste já
compilado e a árvore devolvida em seguida):

- Repondo um `thread::spawn` de produção sem entrada no catálogo
  (`crates/phxsql-core/src/paralelo.rs`), o teste da suíte **falha** nomeando:
  `SUBIU spawn-sem-teto 1 (teto 0)` e `SEM TETO
  crates/phxsql-core/src/paralelo.rs:120 thread::spawn`.
- Envelhecendo uma entrada do catálogo (agulha que não casa com sítio nenhum),
  **falha** nomeando `SUBIU catalogo-envelhecido 1 (teto 0)` e `ENVELHECIDA
  crates/phxsql-server/src/telemetria.rs`.
- Escondendo o medidor, o gerador **acusa** `mapa-das-threads.py SUMIU … a
  catraca dele NÃO foi conferida` e o nomeia na linha final.
- Com a árvore limpa: `ok` nos dois, e a linha final do gerador nomeia só a
  `mapa-da-trava.py`, que é a que está mesmo vermelha.

**E a armadilha que esta medição pagou, porque ela é a lei da casa por outro
caminho**: a **primeira** reposição do defeito **passou** — teste verde com o
defeito na árvore, que é o pior estado possível. A causa não era o teste: eu
tinha acrescentado o `spawn` no **fim** do `paralelo.rs`, e o fim do arquivo
fica **depois** do `#[cfg(test)] mod` da linha 118 — território que o medidor
ignora de propósito, e com razão. **Prova real que não falha com o defeito
reposto não provou nada**, e a segunda tentativa (o `spawn` acima da linha
118) é que mostrou a régua funcionando. A lição tem alcance: repor defeito em
arquivo Rust exige saber **onde acaba a produção**, e não só qual arquivo.

**O que este inventário não via, e passou a ver em 16/09/2026**: o
`docs/qa/medir.py` varria só `crates/*/examples/*.rs` atrás de quem imprime
`catraca:` e `crates/*/src/**` atrás de `pub const TETO*`. Nenhuma das cinco
catracas acima aparecia nele, porque os tetos moram em Python, em `bancada/`.
Está consertado na **§14**, e a frase que estava aqui rendeu um achado sobre
ela mesma: ela dizia «**sete** catracas vivas que a tabela gerada não conta» e
esquecia a `TETO_PKILL_SEM_PID` da §11, que é da mesma família e mora na mesma
pasta. **Eram oito.** O parágrafo que denunciava uma lista curta era, ele
próprio, uma lista curta — e é a prova mais barata de que lei que lista menos
casos do que existem protege menos no dia em que alguém usar a lista como
inventário, inclusive quando a lista é a dos buracos.

### 13-bis. A régua conta ATRIBUIÇÃO, não só ocorrência — o salto de 23 para 25 sem `fsync` novo (17/09/2026)

**O defeito que este parágrafo existe para impedir**: alguém lendo só o número
concluiria que o pedido 164 (`20d2c59`) fez a trava segurar mais `fsync`.
Mediu-se de novo com o mesmo `bancada/concorrencia/mapa-da-trava.py`, e
**nenhum `fsync` novo entrou na trava** — o que mudou foi a **atribuição**.

O mapa chama de **porta comum** qualquer caminho que apareça em pelo menos
**20% das seções** (hoje, 17 de 86) e **herda** o custo dela em vez de contar
como próprio de quem chama — é a melhor prova disponível em tanta seção que
não distingue nenhuma, e por isso sai da classificação por seção e vira fato à
parte. Antes do 164, o `empilhar` e o `empilhar_atualizar_com_cascata` abriam
a tabela pela porta `abrir_travada` — que **é** comum (23/86 pelo lado do
disco, 18/86 pelo lado da durabilidade) — e o `fsync` que ela alcança vinha
**herdado**, não contado por seção.

O pedido 164 trocou essa chamada por uma porta nova,
`abrir_travada_sem_sobrepor` (a dispensa registrada da sobreposição, ver
`docs/PENDENCIAS.md` #164). Ela tem **2** chamadores — os dois de sempre —,
fica bem abaixo do corte de 20%, e por isso **não é comum**: o mesmo `fsync`
que antes vinha herdado de `abrir_travada` passa a contar como **próprio** de
`empilhar` e de `empilhar_atualizar_com_cascata`. Duas seções que já
alcançavam `fsync` (por herança) passam a alcançá-lo **por conta própria** —
e é exatamente isso, e só isso, que move `alcancam-fsync` de 23 para **25**.

**Por que isto fica escrito aqui, e não só no commit**: é o tipo de coisa que
faz alguém, daqui a um mês, ler «o 164 fez a trava segurar mais escrita
durável» — e a régua nunca disse isso. Ela mede o que cada seção alcança pelo
caminho que o mapa consegue resolver por nome; trocar o NOME de uma porta sem
trocar o que ela faz pode empurrar seções para dentro ou para fora do corte de
«comum», e isso é da régua, não do motor. Nenhuma catraca foi tocada por este
achado — nem o teto, nem a régua: o número medido é que subiu, e a catraca
`alcancam-fsync` **continua vermelha** por decisão do dono (#252 (1)), como já
estava. Ver `docs/CONCORRENCIA.md` §1.2/§1.3 e `docs/PENDENCIAS.md` #164.

## 14. O inventário gerado passou a contar as catracas de Python

**O defeito que motivou** (16/09/2026): `docs/qa/medir.py` é o gerador da
tabela das catracas — o arquivo que existe para que **nenhum número desta
casa se digite**. Ele achava catraca varrendo `crates/*/examples/*.rs` atrás
de quem imprime `catraca:` e responde a `--numeros`, e teto órfão varrendo
`crates/*/src/**` atrás de `pub const TETO*`. Os dois crivos são de Rust, e
**oito catracas vivas moram em Python**, em `bancada/`: as três do
`mapa-da-trava.py`, as duas do `mapa-das-threads.py` (§13), a
`TETO_PKILL_SEM_PID` (§11) e as **seis** do `trecho-vivo.py` (§12). **A tabela que
existe para dizer quantas catracas há contava menos do que existe.**

**O conserto, e por que ele não traz lista nenhuma**: o crivo é o **mesmo** —
`bancada/**/*.py` atrás de quem escreve `catraca:nome=`, perguntado com
`--numeros`. Um critério diferente aqui (um nome de pasta, uma lista de
scripts) divergiria do de cima na primeira pasta nova; **o conferidor se
descreve**, dos dois lados, e uma catraca nova em Python entra na tabela sem
ninguém editar o gerador. Os quatro conferidores de `bancada/` ganharam o
modo `--numeros` no mesmo commit.

**`tipo=teto` e `tipo=piso`, e a regra de compatibilidade que isso exigiu**:
o `PISO_DAS_ENTRADAS` (§12.2) reprova quando **desce**, e uma tabela que o
mostrasse como «FROUXA — baixe-a» estaria mentindo sobre o dado. A linha de
auto-descrição passou a carregar `tipo=`, e **quem não o diz continua sendo
teto**: os nove conferidores em Rust não mudaram uma linha. *Guarda nova entra
pedida, não imposta* — um campo obrigatório teria quebrado todo emissor
antigo, que é o mesmo estrago da janela de conflito recusando gravação sem
versão, em miniatura.

**A varredura de órfãs também alcança `bancada/`, e com uma diferença
medida.** Em Rust, `pub const TETO*` dentro de `src/` é sinal forte. Em
`bancada/`, um `TETO_*` no topo de um script é sinal **fraco**: das oito que
existiam no dia, **quatro eram limite de funcionamento** e não dívida de
código — o prazo de uma sonda (`TETO_VARRER_MS`, `TETO_PING_MS`), o espelho
de um teto de produção (`TETO_DO_REGISTRO` em `bancada/seguranca/porta.py`),
um parâmetro de bancada (`TETO` em `enxurrada-web.py`) e o corte de um
classificador (`PISO_DE_CONFIANCA`). Acusá-las encheria a tabela do ruído que
a §7 já ensinou a evitar. **A isenção existe e mora na LINHA da constante**,
não numa lista dentro do gerador: `# nao-e-catraca: <motivo>`. É o molde do
`ISENTOS` do conferidor de temporários, com a receita saindo do código —
quando um gerador depende de uma lista, a lista tem de sair do código.

**O número, medido em 16/09/2026**: a tabela publicada no `docs/QA-PDCA.md`
trazia **8** catracas; o gerador consertado mede **20** (nove em Rust, onze em
`bancada/`).

**E o derivado estava velho, o que é um segundo achado**: a mesma corrida
mostrou que a tabela publicada não tinha a `TETO_INVENTARIO_DESCASADO` (§8,
de 07/09) e dizia `TETO_ROTULOS_E_CRASE` **1.050** quando já era **1.049** —
número velho anunciando sucesso pelo silêncio, exatamente a doença da §10. O
motivo: **o gerador da tabela das catracas não estava no `PLANO` do
`portao-dos-geradores.py`**, então nenhum portão perguntava «re-rodar mudaria
algum número visível?». Ele entrou, no modo `nota-cargo` — o mesmo do
`numeros-do-projeto.py`, porque chama `cargo run --release --example` seis
vezes e martelar o build nesta worktree fura o piso de disco. Ele **não** é
rodado pelo portão; sai como NOTA com o comando, em vez de sumir do relatório.
E o `PLANO` passou a aceitar **o comando**, não só o nome do arquivo:
`docs/qa/medir.py` só escreve com `--gravar`, e um plano que guardasse só o
nome publicaria uma receita que não regenera nada — a doença do gerador
chamado pela metade.

**A prova real, nos dois sentidos** (16/09/2026):

- com o `--numeros` **arrancado** do `pkill-sem-pid.py` (o estado em que a
  catraca sumiria calada), o gerador colhe **10** em vez de 11 e imprime
  *«`bancada/guardas/pkill-sem-pid.py`: escreve `catraca:nome=` e não
  respondeu ao `--numeros`»* — e a `TETO_PKILL_SEM_PID` aparece **também** na
  lista de órfãs, que é o segundo sinal independente. Devolvido, 11 e nenhum
  problema;
- com um `TETO_SINTETICO = 7` acrescentado a um script de `bancada/` sem
  conferidor e sem marca, ele aparece como órfão nomeando arquivo e linha;
  com o `# nao-e-catraca:` na mesma linha, some. Removido, some do mesmo jeito.

**O que ele continua NÃO vendo, e fica dito**: o crivo de órfãs em Rust
continua sendo só `pub const TETO*` — um `const` privado ou um `MAX_*`/
`LIMITE_*` não aparece, e são 26 deles na tabela de limites abaixo. A régua
não mudou nesta frente e por isso nenhuma catraca de lá se aposentou; o buraco
está medido e nomeado na seção dos limites, como estava.

## 15. O catálogo de guardas contra as PÉTREAS — o inventário, e o que continua descoberto

**Por que esta seção existe.** A lei da casa diz que *lei que lista menos
casos do que existem não protege menos hoje — protege menos no dia em que
alguém usar a lista como inventário*. O `catalogo.py` **é** um inventário: ele
diz o que esta casa já pagou e ainda pega. Então ele mesmo tem de ser conferido
contra a lista de pétreas, e não contra a memória de quem o escreveu.

Levantamento do papel G em **16/09/2026** (frente G-CRIPTO), contra as regras
da seção «Regras que não se quebram» do `CLAUDE.md`.

### 15.1 Os dois buracos que a varredura achou, medidos

**«Criptografia se confere contra vetor oficial» — ZERO entradas.** Medido:
nenhuma das 151 entradas do catálogo repunha defeito em SHA-256, HMAC ou
PBKDF2. Havia **teste** — e teste não é guarda. Os números da cobertura que
existia:

| onde | o que foi contado | medido |
|---|---|---:|
| `crates/phxsql-core/src/hash.rs`, `mod tests` | funções de teste | **9** |
| idem | funções que afirmam contra vetor **publicado** | **5** |
| idem | vetores publicados afirmados (4 FIPS 180-4 + 4 RFC 4231 + 3 PBKDF2, mais o de um milhão de letras e o de 40 bytes) | **13** |
| `crates/phxsql-core` inteiro | funções de teste que afirmam contra vetor publicado, em 9 normas | **28** |
| catálogo de guardas | entradas que repunham defeito em qualquer uma delas | **0** |

A última linha é o buraco. **Nenhum daqueles 28 testes tinha sido provado
contra o defeito que ele existe para pegar**, e quem lesse o catálogo como
inventário concluiria que a pétrea mais citada da casa estava coberta.

**«Portão de permissão é UM só — e o campo que ele lê é o furo» — 13 provas,
2 catalogadas.** Medido: a árvore tem **13** funções
`*_nao_e_a_porta_dos_fundos*`, e o catálogo repunha o defeito de **duas** —
`pivotar-sem-portao` e a metade do `operacao-sem-poder-declarado` que carrega
o `procurar_texto`. O `juntar` e o `unir`, **que a própria pétrea escreve**
(«sem conferência própria, bastaria pedir a tabela negada como o lado B de
uma junção»), estavam de fora.

### 15.2 O que entrou, e o veredito de cada uma

**Corrigido em 24/09/2026 (pedido 383, achado do papel G em 22/09/2026).**
Esta seção afirmava «todas PROVADAS pelo provador oficial em 16/09/2026» — mas
nomeada e provada não são a mesma palavra, e é a segunda que o texto usava. O
`bancada/guardas/ultima-corrida.json` é o registro da máquina, e as nove
entradas nasceram **DEPOIS** da corrida que ele carrega (16/09 15:25): nenhuma
delas jamais teve veredito gravado ali. Remedido agora, e não de memória —
`python3 -c` lendo `bancada/guardas/catalogo.py` e
`bancada/guardas/ultima-corrida.json` e comparando os nove `id` contra as
chaves do registro:

| guarda | defeito reposto | caem | veredito |
|---|---|---:|---|
| `sha256-sem-somar-o-estado` | a realimentação de Davies-Meyer vira atribuição | 4/4 | sem veredito no registro da máquina |
| `sha256-com-o-tamanho-em-little-endian` | o tamanho da mensagem no padding em little-endian | 4/4 | sem veredito no registro da máquina |
| `hmac-com-a-chave-longa-truncada` | chave > 64 bytes truncada em vez de pré-hasheada | 2/2 | sem veredito no registro da máquina |
| `pbkdf2-com-o-contador-de-bloco-parado` | `bloco += 1` vira `bloco = 1` | 1/1 | sem veredito no registro da máquina |
| `pbkdf2-sem-o-xor-acumulado` | o XOR acumulado vira atribuição | 2/2 | sem veredito no registro da máquina |
| `juntar-sem-portao` | a conferência própria do `op_juntar` sai | 1/1 | sem veredito no registro da máquina |
| `unir-sem-portao` | a conferência da LISTA do `op_unir` sai | 1/1 | sem veredito no registro da máquina |
| `diferencas-sem-portao` | a conferência dos campos `a`/`b` do `op_diferencas` sai | 1/1 | sem veredito no registro da máquina |
| `derivado-sem-portao` | `portoes_do_pedido` sai do irmão `executar_derivado` | 8/8 | sem veredito no registro da máquina |

O "caem" (quantos testes o executor derruba com o defeito reposto) é o que a
entrada do catálogo **declara**, não o que o provador confirmou — a coluna
ficou porque descrever o defeito continua útil, mas ela não é prova. **Nenhuma
das nove está QUEBRADA** — não há evidência de que falhem, só ausência de
evidência de que passem. Isto não é a mesma coisa que "provada", e é
exatamente a confusão que este achado corrige.

Custo medido **na época em que a corrida original desta seção foi escrita**:
11,7 s para as cinco de criptografia (2,3–2,4 s cada) e 136 s para as quatro do
portão (31–36 s cada, com a cópia quente) — números de tempo de execução, não
de veredito, e continuam válidos como estimativa de custo.

**Para provar as nove e gravar o veredito no registro da máquina** (NÃO
rodado nesta rodada — disco curto, outra frente compilando):

```
python3 bancada/guardas/provar-guardas.py \
  --so sha256-sem-somar-o-estado \
  --so sha256-com-o-tamanho-em-little-endian \
  --so hmac-com-a-chave-longa-truncada \
  --so pbkdf2-com-o-contador-de-bloco-parado \
  --so pbkdf2-sem-o-xor-acumulado \
  --so juntar-sem-portao \
  --so unir-sem-portao \
  --so diferencas-sem-portao \
  --so derivado-sem-portao \
  --json bancada/guardas/ultima-corrida.json
```

**Achado à parte, fora do escopo das nove:** o catálogo cresceu de **199**
entradas (22/09/2026) para **235** hoje, e o `ultima-corrida.json` de **154**
para **183** vereditos — medido agora, não repetindo o número do pedido. A
`trava-sem-guarda-de-reentrancia` continua a única com veredito **QUEBRADA**
no registro, parada desde 16/09 15:25 — decidi-la não é parte deste pedido.

### 15.3 A escolha do defeito é o trabalho, e ela se justifica entrada por entrada

Trocar uma constante de `K`, inverter um deslocamento ou cortar uma rodada
derrubariam o vetor do mesmo jeito — e provariam pouco, porque **ninguém comete
esses**: as 64 constantes são um bloco copiado da norma e não se «arrumam». O
critério usado nas cinco foi um só: *um refatorador distraído cometeria este de
verdade?* Daí o `zip` com `wrapping_add` reduzido a uma atribuição, o
`if`/`else` de dois `copy_from_slice` «generalizado» num `min`, o segundo
contador de um `while` reiniciado no lugar errado, o laço de XOR aninhado
trocado por uma atribuição, e o `to_be_bytes` uniformizado para a ordem da
máquina.

E as cinco foram escolhidas para provarem **famílias de vetor diferentes**, que
é o que faz o conjunto valer mais que a soma:

| defeito | quem o pega | quem NÃO o pega — e é o ponto |
|---|---|---|
| realimentação do estado | todos os vetores | `sha256_alimentado_em_pedacos_da_o_mesmo` (auto-consistência) |
| tamanho em little-endian | `"abc"` e os dois longos | **o vetor da mensagem vazia**: zero é zero nas duas ordens |
| chave longa truncada | **só** o caso 6 da RFC 4231 e o anexo A.2 da RFC 5869 | PBKDF2, `senha.rs`, os anexos A.1 e A.3 |
| contador de bloco parado | **só** o vetor PBKDF2 de 40 bytes | toda derivação da árvore, que pede 32 bytes |
| XOR acumulado | os vetores PBKDF2 iterados | ida-e-volta do `senha.rs`, o sal, o custo, o formato |

A coluna da direita é a pétrea escrita como tabela: **auto-consistência,
ida-e-volta e propriedade sobrevivem a um motor de criptografia quebrado**,
porque as três perguntam ao próprio motor. Só a norma responde de fora.

### 15.4 As 13 portas dos fundos, uma a uma — e por que 9 entradas bastam

O provador foi usado como **instrumento de medição** antes de virar veredito:
uma sonda temporária repôs o defeito no `portoes_do_pedido` de
`executar_derivado` com dez nomes no `caem`, e a corrida disse exatamente
quais caem por ali e quais não. Oito caíram; `juntar` e `procurar_texto`
ficaram verdes — porque entram pelo `despachar`, que tem a chamada dele.

| # | prova | quem a repõe |
|---|---|---|
| 1 | `procurar_texto_nao_e_a_porta_dos_fundos_para_a_tabela_negada` | `operacao-sem-poder-declarado` (já existia) |
| 2 | `pivotar_nao_e_a_porta_dos_fundos` | `pivotar-sem-portao` (já existia) |
| 3 | `juntar_nao_e_a_porta_dos_fundos` | **`juntar-sem-portao`** (nova) |
| 4 | `unir_nao_e_a_porta_dos_fundos` | **`unir-sem-portao`** (nova) |
| 5 | `diferencas_nao_e_a_porta_dos_fundos` | **`diferencas-sem-portao`** (nova) |
| 6 | `o_sql_nao_e_a_porta_dos_fundos_para_a_tabela_negada` | **`derivado-sem-portao`** (nova) |
| 7 | `o_dml_pelo_sql_nao_e_a_porta_dos_fundos_para_a_tabela_negada` | idem |
| 8 | `o_join_pelo_sql_nao_e_a_porta_dos_fundos` | idem |
| 9 | `o_consultar_nao_e_a_porta_dos_fundos_para_a_tabela_negada` | idem |
| 10 | `o_consultar_nao_e_a_porta_dos_fundos_pela_juncao_nem_pelo_escalar` | idem |
| 11 | `o_existe_nao_e_a_porta_dos_fundos` | idem |
| 12 | `a_visao_nao_e_a_porta_dos_fundos_para_a_tabela_negada` | idem |
| 13 | `call_nao_e_a_porta_dos_fundos_para_a_tabela_negada` | idem |

**Nenhuma ficou sem guarda, e nenhuma dispensa foi preciso registrar.** O que
o número 8 ensina é outra coisa, e vale mais que a cobertura: os oito caminhos
que parecem oito operações diferentes têm **um** ponto de conferência, e ele
mora numa função de três linhas que parece um embrulho fino do `executar`.
Inliná-la compila, passa no `clippy` e não derruba prova por soquete nenhuma —
e abre o SQL inteiro.

### 15.5 As pétreas que continuam SEM guarda, nomeadas

Dispensa registrada é decisão; dispensa silenciosa é esquecimento. Estas
ficaram de fora desta frente, **medidas e nomeadas**:

| pétrea | entradas no catálogo | o que existe hoje |
|---|---|---|
| ~~**«Senha nunca em texto puro»**~~ | **9** | **FECHADA em 16/09/2026** pela frente G-SENHA — ver **§15.7**. Esta linha fica riscada em vez de sair, porque os dois números que ela trazia estavam **errados**, e o erro ensina mais que a linha: eram **2** entradas e não 0 (`profiler-recorta` e `profiler-recorta-largo` já repunham defeito desta pétrea), e eram **40** provas medidas e não 11. Contagem por memória de nome de teste mede a lembrança de quem escreve; a varredura pela **asserção** mede a árvore |
| **«Bancada compara trabalho igual, não só pergunta igual»** | **0** | nem entrada nem catraca. As quatro regras vivem em `bancada/LEIA-ME.md` como prosa, e os dois erros que as fundaram (o `WHERE id IN (…)` contra vinte mil buscas, o `COUNT(*)+SUM` contra a leitura de 20.000) não têm defeito reposto nenhum. Repor este defeito exige mexer no roteiro de uma bancada e medir — não é troca de trecho em Rust, e por isso não cabe neste catálogo sem antes decidir a forma |
| **«Merge de conflito marca quem MEXEU, não quem perguntou por último»** | **0** | o comportamento é de interface (coluna a coluna), e a prova dele é de tela. Cai na mesma classe da linha abaixo |
| **«O CSS global morde todo componente novo» / «Interface só se prova exercitando»** | **0** | quem cobre é o `TETO_BOTAO_SEM_PROVA` (§9), que é **outra coisa**: ele conta botão que a bateria não clica, não repõe defeito. As duas lições (o rádio do tamanho da célula, «BLUMENAU») não aparecem lendo código — e também não aparecem repondo trecho |
| **«Medidor com binário velho mede o passado»** | **0** | a regra tem receita (`cargo build --release --examples` antes de medir) e nenhuma guarda. Repor o defeito aqui é apagar uma linha de um script de bancada, não de `crates/` |

As três últimas têm a mesma causa e ela se diz: **o catálogo só sabe repor
defeito em arquivo compilado e conferido por `cargo test`.** Pétrea cuja prova
mora no navegador, num soquete ou num roteiro de bancada não cabe nele como
ele é hoje. Isso não as deixa protegidas — deixa-as **nomeadas**, que é o que
esta seção existe para fazer.

### 15.6 Nenhum teto subiu — e o único número que subiu foi piso

`PISO_DAS_ENTRADAS` foi de **151** para **160** no mesmo passo em que as nove
entradas nasceram. Ele é a exceção declarada e continua sendo a única:
**conta entradas, não defeitos**, e piso parado com catálogo que cresceu volta
a aceitar o apagamento das entradas novas. Os quatro tetos do §12 ficaram
todos em **0** (medidos com o `--catraca` depois de cada escrita), e **nenhuma
outra catraca da árvore foi tocada.**

E fica dito qual é a que **pediria** para subir, porque é a que um leitor
apressado «consertaria» primeiro: a `alcancam-fsync` do mapa da trava mede
**25** hoje (23 quando esta seção foi escrita; remedida em 17/09/2026 pelo
pedido 164 — atribuição, não `fsync` novo, ver §13-bis) com teto **22** —
vermelha por decisão registrada (pendência #252), e o caminho certo ali é
baixar o número medido, nunca levantar o teto.

**E ele subiu de novo no mesmo dia**, de **160** para **169**, na frente
G-SENHA da §15.7. Duas frentes mexeram nesta mesma catraca em horas
diferentes do dia 16/09/2026 — e é exatamente o caso que o `CLAUDE.md`
nomeia: *nesta base, um número digitado envelheceu em noventa minutos porque
duas frentes mexeram na mesma catraca sem se verem*. Aqui as duas se viram,
porque a segunda leu esta seção antes de escrever.


### 15.7 A pétrea «Senha nunca em texto puro» — nove entradas, e um ponto cego de 1.103 provas

Frente **G-SENHA**, 16/09/2026, no molde da §15. A lei do dono: *«Senha nunca
em texto puro. Nem em arquivo, nem em log, nem em resposta do protocolo. Há
teste que falha se a ficha de usuário vazar o hash.»*

#### 15.7.1 O retrato de antes, medido — e os dois números que a §15.5 errou

A §15.5 disse **0 entradas** e **11 testes**. Os dois estavam errados, e o
jeito de errar é o mesmo nos dois: **contaram nomes de teste lembrados**, não
a árvore.

A varredura desta frente não pergunta pelo nome. Ela pergunta pela
**asserção**: uma função `#[test]` defende esta pétrea quando afirma que um
segredo **não aparece** numa saída — um `!…contains(…)`/`!…windows(…)` em que
o literal negado é senha, segredo, token, pino ou chave. O `!` da negação
nunca vem colado num identificador, senão o `assert!` seria contado como
negação (foi o primeiro erro desta medição, e ele inflava a conta com
asserções de **presença**).

| o que foi contado | medido |
|---|---:|
| funções `#[test]` que afirmam a AUSÊNCIA de um segredo numa saída, em `crates/**` | **40** |
| destas, as que a §15.5 nomeava | 11 |
| destas, as que já apareciam no `caem` de alguma entrada | **6** |
| entradas do catálogo que repunham defeito desta pétrea | **2** (`profiler-recorta`, `profiler-recorta-largo`) |
| **provas sem defeito reposto nenhum** | **34** |
| *(depois desta frente)* provas com defeito reposto | **14** — e **26** continuam sem |

**O 40 é remedível, e tem de ser.** Ele saiu de uma varredura escrita para
esta frente e não de um gerador versionado — o que faz dele exatamente o tipo
de número que esta casa sabe que envelhece calado. Enquanto não houver régua,
a receita fica aqui, e ela é curta o bastante para se refazer: varrer
`crates/**/*.rs`, recortar cada `fn` precedida de `#[test]`, e contar as que
têm uma linha com negação (`!` **não** colado num identificador — senão o
`assert!` conta como negação) junto de `contains`/`windows`/`find` e de um
literal que case
`senha|segredo|s3nh|secret|passwd|password|token|pino`. **O 40 é um piso, não
um total**: provas que afirmam ausência sem citar o segredo — como
`o_que_nao_se_analisa_vira_o_tamanho`, que nega a palavra `aberta` — não
entram no crivo e existem. Transformar isto numa régua do `trecho-vivo.py` é
o próximo passo do papel G nesta pétrea.

**Refeita em 17/09/2026 — e o 40 não se refez pela receita escrita.** A
receita acima, aplicada ao pé da letra — negação `(?<![A-Za-z0-9_])!`,
chamada `\.(contains|windows|find)\(` na mesma linha, léxico
`senha|segredo|s3nh|secret|passwd|password|token|pino` em qualquer lugar da
linha —, dá **31** no commit em que o 40 foi escrito (`1e3e7e1`, medido
sobre um `git archive` dele) e **35** hoje. O que a varredura original
contou e a receita não dizia é o ajudante `contem(` das provas de
integração do `phxsql-store` — `a_senha_nao_vai_para_o_disco` afirma
`!contem(SENHA.as_bytes(), &bruto)`, sem ponto e sem `.contains`. Com
`(contains|contem|windows|find)\(` a conta dá **36** naquele commit e
**40** hoje. Ou seja: **os dois 40 não são o mesmo 40** — o de hoje carrega
as quatro provas de `{:?}` do commit `74de67e` que casam o léxico
(`nenhum_segredo_do_config_sai_no_debug`,
`o_debug_da_ligacao_nunca_mostra_a_senha_nem_o_token`,
`o_debug_do_comando_nunca_mostra_a_senha`,
`o_debug_da_receita_nunca_mostra_o_token_nem_a_senha`; a quinta,
`o_debug_do_usuario_nunca_mostra_o_hash`, nega `hash`, que o léxico não
tem), e o de ontem tinha quatro que a receita não descrevia. E a leitura
**estrita** — o léxico só dentro de um literal `"…"` — dá **27** hoje: oito
provas negam um identificador (`segredo`, `SENHA`), não um literal, e
`nenhuma_credencial_do_config_sai_pela_op_config`, a mais larga do
`config.rs`, fica fora de **todas** as leituras porque nega `marca`, a
variável do laço. **O 40 continua sendo piso, e agora é remedível**: a
receita é o regex de cima, com `contem`, na leitura larga — e a régua que
o versiona continua sendo o próximo passo do papel G, porque um número
que só se refaz a partir de um parágrafo é um número que se refaz errado
na primeira vez (foi o que aconteceu aqui: 40 dito, 31 medido, pela mesma
frase).

As seis cobertas eram todas do Profiler — e **nenhuma das quatro saídas que a
pétrea NOMEIA** (o arquivo, o log, a resposta do protocolo e a ficha) tinha
entrada. A frase «0 entradas» protegia menos do que parecia por baixo e mais
do que parecia por cima: havia guarda, e ela cobria o vizinho.

#### 15.7.2 As nove que entraram, e o veredito de cada uma

Todas **PROVADAS pelo provador oficial**, com a árvore limpa verde antes de
cada bloco: **1.103** testes no `phxsql-server --lib`, **253** no
`phxsql-sql --lib`, **187** no `phxsql-store --lib` e **5** no
`phxsql-server --test cifra-pelo-config`.

| guarda | defeito reposto | caem | veredito |
|---|---|---:|---|
| `ficha-do-usuario-devolve-o-hash` | a `ficha()` passa a levar o `senha_hash` junto | 2/2 | ✅ provada |
| `senha-em-claro-no-cadastro` | `senha::cifrar(clara)` vira `clara` em `objeto_do_usuario` | 2/2 | ✅ provada |
| `senha-velha-fica-no-arquivo` | o `retain` que tira a `senha` em claro do usuário antigo sai | 1/1 | ✅ provada |
| `cifra-reserializa-a-senha` | o `para_json` da cifra devolve a senha real em vez de `(oculta)` | 2/2 | ✅ provada |
| `debug-da-cifra-mostra-a-senha` | `.field("senha", &"(oculta)")` vira `&self.senha` | 1/1 | ✅ provada |
| `profiler-sem-a-senha-dentro-do-sql` | o ramo `sql_sem_senha` sai do `limpar` | 1/1 | ✅ provada |
| `comando-invalido-vira-texto-cru` | o SQL que o léxico recusa volta inteiro em vez de virar tamanho | 1/1 | ✅ provada |
| `trilha-sem-o-nome-de-segredo` | a conferência pelo NOME da coluna sai do `valor_para_trilha` | 1/1 | ✅ provada |
| `trilha-so-olha-o-nome-da-coluna` | a conferência que ANALISA o valor sai do `valor_para_trilha` | 1/1 | ✅ provada |

Custo medido: 12,6–28,1 s cada no `phxsql-server --lib` (com a cópia quente),
4,5 s na de integração e 1,5–1,6 s nas de `phxsql-store` e `phxsql-sql`. A
árvore limpa custa 26,6–46,2 s no servidor e 1,6–1,7 s nos outros dois.

#### 15.7.3 O raio de cada defeito, MEDIDO — e não argumentado

`PROVADA 1/1` diz que o teste nomeado caiu. **Não diz que só ele caiu** — um
defeito que derrubasse quarenta provas também sairia `1/1`, e a coluna «quem
NÃO o pega» seria escrita de memória. Então o provador foi usado como
**instrumento de medição** antes de virar veredito: cada defeito correu uma
segunda vez numa sonda temporária com `espera: "nada muda"`, que lê o veredito
de **todos** os testes do binário e não só dos nomeados. As sondas foram
apagadas depois; o número ficou.

| defeito reposto | quem o pega | quem NÃO o pega — e é o ponto |
|---|---|---|
| o `Debug` da cifra imprime a senha | **1 dos 5** testes do `--test cifra-pelo-config` | **ZERO dos 1.103** do `--lib`. Nenhum veredito mudou. Os três testes de credencial do `config.rs` — inclusive o genérico, que existe justamente para pegar o campo que alguém acrescentar amanhã — olham o `para_json`, e o `Debug` é outro caminho |
| o `retain` da senha velha sai | **1 dos 1.103**: `trocar_a_senha_leva_junto_a_que_estava_em_texto_puro` | `a_senha_nunca_aparece_no_arquivo_nem_na_resposta`, que é a prova do MESMO arquivo. O usuário **novo** não sente: `pares` nasce vazio sem anterior, e `senha` não está em `CAMPOS_DO_USUARIO`. Só a ALTERAÇÃO acusa |
| o ramo `sql_sem_senha` sai do Profiler | **1 dos 1.103**: `a_senha_dentro_do_texto_sql_tambem_sai` | `a_senha_nunca_aparece`, a prova mais completa do arquivo — **oito** pedidos. Os oito nomeiam a senha num CAMPO; nenhum a esconde dentro de uma frase. Ser a mais completa do arquivo não é alcançar o ramo do vizinho |
| a conferência pelo NOME sai da trilha | **1 dos 187**: `coluna_de_senha_nao_entrega_o_valor` | `hash_em_coluna_de_nome_inocente_e_redigido`. A análise pega o que **já está protegido** (o hash destrincha em `pbkdf2-sha256$…`); a senha em texto puro numa coluna `senha` não é hash, e passa inteira |
| a ANÁLISE do valor sai da trilha | **1 dos 187**: `hash_em_coluna_de_nome_inocente_e_redigido` | `coluna_de_senha_nao_entrega_o_valor`. É a metade contrária, e o par é a prova de que as duas conferências existem — coisa que nenhuma das duas provas diz sozinha |
| o comando inválido volta cru | **1 dos 253**: `o_que_nao_se_analisa_vira_o_tamanho` | `a_senha_sai_do_texto_do_comando`, que só passa comandos VÁLIDOS. O caso que mais pede o texto cru no log — `CREATE USER c PASSWORD 'aberta`, recusado **por causa da aspas da senha** — é o que mais o proíbe |
| a ficha leva o `senha_hash` | **2 dos 1.103**, em dois módulos: `usuarios` e `servidor` | as outras 1.101. Duas provas para uma função por onde passam **três** operações (`usuarios`, `usuario`, e a resposta do login) |
| o `para_json` da cifra devolve a senha | **3 dos 1.103** | — o terceiro não estava previsto: `a_senha_da_cifra_pode_vir_do_ambiente` cai por tabela, porque a marca `(do ambiente)` some junto. Fica escrito para não ser lido como guarda nova numa corrida futura |
| `senha::cifrar(clara)` vira `clara` | **5 ou mais dos 1.103** | — é a única larga da leva, porque quebra o LOGIN junto. Ensina menos sobre alcance, e entrou assim mesmo: o defeito estava **escrito no comentário do teste** desde que ele nasceu («repor o defeito é trocar `senha::cifrar(clara)` por `clara` em `objeto_do_usuario`»), e ninguém o executava. Instrução de prova real escrita e nunca corrida é o retrato exato do teste não provado |

A linha de cima é o achado da frente, e ela vale como aviso de método: **mil e
cento e três provas, e um ponto cego**. E o ponto cego não é «o `Debug` não se
testa nesta casa» — testa-se, e a §15.7.5 mostra o vizinho `CifraFio` sendo
conferido em `--lib`, `{:?}` a `{:?}`. O cego é mais fino e por isso mais
perigoso: **a prova existe, e mora no binário errado**. Quem rodar
`cargo test -p phxsql-server --lib` — que é o que uma frente com pressa roda —
vê 1.103 verdes com a senha do cofre saindo em todo `{:?}`.

O `Debug` também não está na frase da pétrea: ela diz «arquivo, log, resposta
do protocolo». Uma lista de saídas é um inventário, e **lei que lista menos
casos do que existem protege igual hoje e menos no dia em que alguém usar a
lista como inventário** — foi exatamente o que aconteceu. O comentário acima
do `impl` já avisava, com todas as letras, que *«segredo que aparece em
`Debug` vaza no dia em que alguém acrescentar um `dbg!`»*. **Comentário que se
declara resolvido é o motivo de ninguém olhar de novo** — a mesma lição que o
`conferir_a_arvore` custou em 03/09, e a §15.7.5 mostra a terceira estrutura,
a do DbLink, em que ninguém olhou mesmo.

#### 15.7.4 A escolha do defeito, entrada por entrada

O critério foi o da §15.3: *um refatorador distraído cometeria este de
verdade?* Aqui houve um segundo crivo, e ele é o que separa sabotagem de
defeito: **cada um dos nove tem um pedido legítimo por trás**, e é isso que os
faz sobreviver ao `git diff`.

- a ficha ganha o `senha_hash` porque **a tela de edição precisa devolver o
  usuário inteiro para salvar de volta** — e o campo da senha é o que falta;
- o `para_json` da cifra devolve a senha porque a tela lê `(oculta)` e, ao
  salvar, **mandaria `(oculta)` de volta como senha**. A quebra é real; o
  conserto certo é a tela não reenviar o campo, e o errado cabe numa linha;
- o `Debug` mostra a senha porque quem depura *«por que o cofre não abre com a
  senha certa»* troca essa linha **de propósito** e esquece de desfazer. O que
  sobra continua sendo um `Debug` escrito à mão, com um campo a mais;
- o comando inválido volta cru porque `<comando inválido, 31 bytes>` **não
  ajuda ninguém** a achar a aspas que faltou;
- a trilha deixa de olhar o nome da coluna porque *«a análise abaixo já
  destrincha o hash, então a lista de nomes é redundante»* — e deixa de
  analisar o valor porque `senha::e_hash` roda em **todo** valor de **toda**
  coluna marcada de **toda** alteração registrada, que é a primeira linha em
  que se olha ao caçar tempo na trilha.

Nenhum dos nove é uma constante trocada nem um `assert` apagado. Os nove
compilam, passam no `clippy`, e oito deles deixam **1.100 ou mais** das provas
do binário verdes.

#### 15.7.5 O caminho IRMÃO — e ele está ABERTO hoje, medido

Registrado como achado, não como dispensa: **esta frente não o cobriu, e ele
não é hipotético.**

**Primeiro, a hipótese que morreu medida.** Escrevi que o `CifraFio` tinha
`Debug` escrito à mão e nenhum teste de `{:?}` — e estava errado. O
`a_privada_do_fio_nunca_sai` (`config.rs`, `--lib`) confere **duas** coisas
que o teste da `Cifra` não confere: `format!("{:?}", c.cifra_fio)` **e**
`format!("{c:?}")`, o `Config` **inteiro**. O irmão estava mais protegido que
o caminho que motivou a entrada, e não menos. Hipótese que morre medida é
resultado tão válido quanto ganho, e é o que impede a mesma suspeita de
voltar sem medição.

**Segundo, o que a mesma varredura achou ao lado — e este está aberto.** Há
**três** estruturas do servidor que guardam segredo em campo privado:

| estrutura | `Debug` | prova de `{:?}` |
|---|---|---|
| `Cifra` (`config.rs`) | **escrito à mão**, senha `(oculta)` | `a_resposta_do_protocolo_nao_leva_a_senha` (integração) |
| `CifraFio` (`config.rs`) | **escrito à mão**, privada `(oculta)` | `a_privada_do_fio_nunca_sai` (`--lib`), e ele cobre o `Config` inteiro |
| `Definicao` (`dblink/mod.rs`) | **`#[derive(Debug)]`** | **nenhuma** |

Medido em 16/09/2026 com uma sonda temporária (`format!("{d:?}")` sobre uma
ligação carregada por `Definicao::de_json`, removida em seguida — o
`crates/` voltou limpo):

```
Definicao { nome: "x", motor: MySql, host: "h", porta: 3306, usuario: "u",
            senha: "segredo-do-outro-banco", senha_env: "",
            token: "token-do-outro", token_env: "", database: "d", … }
```

**Vazam os dois**: a senha do outro banco e o token de serviço. E o `Registro`
— `#[derive(Debug, Default)]`, com `pub ligacoes: Vec<Definicao>` — também
vaza, medido: um `{:?}` no cadastro inteiro despeja a senha e o token de
**todas** as ligações de uma vez.

O agravante é o de sempre nesta casa, e ele está escrito no próprio arquivo:
os comentários dos dois campos **declaram o problema resolvido** —
*«ela nunca sai em JSON nem em log»* na senha, e
*«ele nunca sai em JSON, em log nem na tela»* no token, este último com o
motivo mais forte (*«quem o tem alcança a porta de dados do outro servidor sem
usuário nenhum»*). As duas frases são verdade sobre o `para_json`, que tem
prova (`a_senha_da_ligacao_nunca_aparece_no_json`), e **mentira sobre o
`Debug`**, que não tem. É a lei de 03/09 por outro caminho: **comentário que
se declara resolvido é o motivo de ninguém olhar de novo** — e aqui o
comentário chega a nomear a saída («em log») que ninguém foi conferir.

Por que ficou sem guarda nesta frente: fechar isto é **mudar produção** —
trocar o `derive` por um `Debug` escrito à mão, como os dois vizinhos já
têm —, e esta frente escreve prova, não código. O caminho está pronto para
quem o pegar, e é curto: o `Debug` da `Cifra` (`config.rs:1224`) é o molde
literal, a prova nova é um `{:?}` ao lado do
`a_senha_da_ligacao_nunca_aparece_no_json`, e a entrada de catálogo que nasce
depois dela é a décima desta pétrea — `debug-da-ligacao-mostra-a-senha`, com
o `derive` de volta como defeito reposto.

E a ordem importa: **a prova entra com o conserto, não antes**. Prova escrita
hoje nasceria vermelha, e portão vermelho na árvore limpa é o que faz a
próxima frente aprender a ignorar o portão.

#### 15.7.6 O que ficou de fora, e por quê

- **`a_senha_nao_vai_para_o_disco`** (`phxsql-store --test
  cifra-dos-diarios`) — a senha do cofre contra os bytes do `.log`. É uma das
  quatro saídas da pétrea («nem em arquivo») e ficou sem entrada porque o
  defeito plausível ali é na derivação da chave, e isso é território da §15.2,
  que já repõe defeito em PBKDF2. Cobrir por cima seria contar a mesma
  cobertura duas vezes;
- **26 das 40 continuam sem defeito reposto** *(em 16/09; a §15.7.7 levou
  a conta, refeita pela receita da §15.7.1, de **9/40 para 16/40** em
  17/09)*. Contado depois, e não
  estimado antes: a cobertura foi de **6/40 para 14/40** — nove entradas
  compram oito provas novas porque duas delas (`ficha-do-usuario-devolve-o-hash`
  e `senha-em-claro-no-cadastro`) dividem a mesma prova do arquivo, e é assim
  que tem de ser: a prova é a mesma, os defeitos é que são dois. A frente
  escolheu nove que cobrem as quatro saídas nomeadas mais o `Debug`, e não as
  nove mais baratas — as três de `phxsql-store`/`phxsql-sql` custam 1,6 s e as
  cinco do servidor custam até 29,6 s cada;
- **nenhuma prova por soquete entrou.** A `bancada/usuarios/provar.py` exercita
  o servidor vivo e é quem cobre o `acessos.log`, que nenhum teste de módulo
  percorre. O catálogo não sabe repor defeito contra ela — é a mesma fronteira
  que a §15.5 nomeia para a bancada e para a tela.

#### 15.7.7 A segunda leva (17/09/2026): sete saídas que a frase não nomeia, e quatro achados de método

Frente F com o chapéu de G, 17/09/2026, no molde da §15.7. A primeira leva
cobriu as quatro saídas que a frase da pétrea nomeia mais o `Debug`. Esta
cobre o que a árvore **já provava sem guarda** — e a regra de escolha foi
**uma entrada por SAÍDA, não uma por struct**
(`docs/cognicao/cognicao_guarda-trava-a-struct-nao-a-lei_20260917_0010.md`):
as cinco structs que ganharam `impl Debug` em 17/09 sem entrada própria
continuam sem, de propósito — quando o mesmo defeito cabe em N lugares, o que
protege a lei é a régua que conta os lugares, e ela é de outra frente.

**O raio, medido antes do veredito.** Como na §15.7.3, cada defeito correu
primeiro numa sonda que lê o veredito de **todos** os testes do binário
(reusando `Arvore`, `rodar` e `trocas_de` do próprio `provar-guardas.py`, e
não uma reimplementação — as notas do provador cortam em cinco nomes, e é por
isso que a §15.7.3 diz «5 ou mais»). Árvore limpa verde antes: **1.107** no
`phxsql-server --lib`, **348** no `phxsql-core --lib`, **59** no
`phxsql-odbc --lib`.

| guarda | a SAÍDA | defeito reposto (a linha que um apressado escreve) | quem o pega | quem NÃO o pega — e é o ponto | raio |
|---|---|---|---|---|---:|
| `fio-cifrado-manda-o-claro-junto` | o fio — o protocolo em si, cifrado | a linha em claro vai **depois** do registro selado, «para o `tcpdump` do suporte» | `o_texto_claro_nao_aparece_no_fio` | **hipótese que morreu medida**: eu escrevi que o ida-e-volta ficaria verde «lendo um registro e deixando o claro no buffer». `canal_leva_e_traz` **cai junto** — lê a despedida depois do pedido e acha o claro no meio. As outras 346 ficam verdes, inclusive `fim_e_corte_sao_vereditos_diferentes`, que sela por fora. O defeito é barulhento no formato e silencioso na pétrea: um leitor ensinado a pular a linha que não é Base64 reverdeceria o ida-e-volta com o vazamento de pé | **2/348** |
| `diario-das-diretivas-guarda-o-segredo-anterior` | o diário das diretivas — log que é arquivo | `valor_anterior` sai inteiro, «para reverter um token durante o incidente» | `o_valor_do_campo_sigiloso_nao_vai_para_o_diario` | `o_show_server_settings_nao_vaza_segredo`, que usa a **mesma** lista `campo_sigiloso` e fica verde: olha o valor vivo, que nunca passa por este `para_json` | **1/1.107** |
| `cluster-devolve-a-credencial-na-tela` | a op `config`, seção do cluster — hash **e** token | `usuario`/`senha_hash`/`token` no mesmo `vec!`, «hash não é senha» | `a_credencial_do_cluster_nao_sai_em_json` **e** o genérico `nenhuma_credencial_do_config_sai_pela_op_config` | as outras 1.105. Aqui o genérico **alcança** — tem o token e o hash do cluster na lista dele. Compare com as duas linhas abaixo | **2/1.107** |
| `token-do-rest-entra-pela-tela` | a tela no sentido de **ENTRADA** | `("rest.token", Texto, false)` em `CAMPOS_EDITAVEIS`, «para não editar o arquivo» | `o_token_do_rest_nao_sai_nem_entra_pela_tela` | o genérico e **toda** prova de `para_json`: guarda de saída não vê porta de entrada. É a única das 40 que olha esse lado | **1/1.107** |
| `receita-odbc-devolve-a-senha` | a *connection string* devolvida ao aplicativo — outro pacote, outro processo | `;PWD=<senha>` inteiro, «para reconectar» | `mascarada_nao_vaza_segredo` | `mascarada_diz_o_modo_e_nao_o_pino` e **todas** as do servidor: o vazamento acontece num processo que o servidor nem vê | **1/59** |
| `cifra-do-fio-reserializa-a-privada` | a op `config`, a IRMÃ da cifra | `chave_privada` inteira, o mesmo `find`/`replace` que a §15.7 repôs na `Cifra` | `a_privada_do_fio_nunca_sai` | **o genérico fica VERDE** com a privada X25519 do servidor saindo inteira: ela não está na lista dele | **1/1.107** |
| `especificacao-openapi-leva-o-token` | `GET /openapi.json`, servido **antes de qualquer portão** | `("x-token", rest.token)` no topo do documento, «para o explorador vir pré-autenticado» | `a_especificacao_nao_carrega_o_token` | todas as do `config.rs`: é outra serialização do mesmo `Rest`, num módulo que a lista genérica não conhece | **1/1.107** |

Os sete compilam, passam no `clippy` e deixam **1.105 ou mais** das provas
do servidor verdes; nenhum é constante trocada nem `assert` apagado, e cada
um tem um pedido legítimo por trás (a coluna do defeito o cita).

**Os quatro achados de método, na ordem em que doeram:**

1. **O 40 não se refez pela receita escrita** — §15.7.1, parágrafo «Refeita
   em 17/09». A frase dava 31 no commit em que o 40 foi escrito; faltava o
   ajudante `contem(`. Hoje o regex está no documento e a conta é **40**
   (leitura larga) e **27** (estrita), e o 40 de hoje não é o 40 de ontem.

2. **A décima entrada nasceu QUEBRADA, e a régua barata não viu.**
   `debug-da-ligacao-mostra-a-senha` nomeava os testes **sem o módulo**
   (`o_debug_da_ligacao_nunca_mostra…` em vez de
   `dblink::testes::o_debug_da_ligacao_nunca_mostra…`). O `julgar` do
   provador compara com o nome que o `cargo test` imprime, e devolveu
   «teste que o catálogo nomeia e o binário não tem» — medido pelo `--so`,
   1m30s. `TETO_TESTE_MORTO` e `TETO_TESTE_FORA_DO_BINARIO` ficaram em `ok 0`
   porque casam pelo nome da `fn`, não pelo caminho. É uma **sexta forma de
   QUEBRADA** para a tabela da §12, do lado que a régua não vê — e é vista sem
   compilar: basta exigir `::` no nome. Consertada no catálogo (os três
   nomes), re-provada: **1/1.107**, só o `caem`. **A régua nasceu em 17/09**
   (`TETO_TESTE_SEM_MODULO`, §12.7) — e só para `--lib`, porque o `::`
   exigido a todos acusaria 164 nomes certos.

3. **A lista genérica de segredos envelheceu.**
   `nenhuma_credencial_do_config_sai_pela_op_config` existe «para pegar o
   campo que alguém acrescentar amanhã», e tem **dez** marcas. A privada do
   fio e o token do REST **não estão nela** — medido: verde nos dois defeitos.
   E ela fica **fora das 40** em todas as leituras da receita, porque nega
   `marca`, a variável do laço. Duas linhas no teste (`crates/`, não desta
   frente) fecham o buraco; fica reportado ao papel B.

4. **A árvore limpa não estava verde — por trabalho de outra frente.** A
   cópia do provador sincroniza da árvore de **trabalho**, e a frente dos
   idiomas tinha `idiomas.rs` e `ui/index.html` sujos no meio de uma troca
   (`tela.sv_sub_no_ar` na fábrica e nenhuma tela a pedir). O portão da
   árvore limpa reprovou por motivo alheio, e a saída foi provar contra um
   `git archive HEAD` com o catálogo desta frente por cima. Com frentes
   paralelas, «medidor com binário velho mede o passado» ganha um irmão:
   **provador com árvore suja mede a frente vizinha**.

**O veredito, pelo provador oficial** (`provar-guardas.py --so …`, contra o
`git archive HEAD` do achado 4, cópia quente; árvore limpa verde antes:
**1.107** no `phxsql-server --lib`, **348** no `phxsql-core --lib`, **59** no
`phxsql-odbc --lib`). A `cifra-do-fio-imposta` veio de carona — o `--so`
casa por substring e `cifra-do-fio` é prefixo das duas — e foi re-provada
junto:

| guarda | caem | veredito | custo |
|---|---:|---|---:|
| `fio-cifrado-manda-o-claro-junto` | 1/1 | ✅ provada | 2,3 s |
| `diario-das-diretivas-guarda-o-segredo-anterior` | 1/1 | ✅ provada | 28,8 s |
| `cluster-devolve-a-credencial-na-tela` | 2/2 | ✅ provada | 31,9 s |
| `token-do-rest-entra-pela-tela` | 1/1 | ✅ provada | 32,1 s |
| `receita-odbc-devolve-a-senha` | 1/1 | ✅ provada | 1,4 s |
| `cifra-do-fio-reserializa-a-privada` | 1/1 | ✅ provada | 29,0 s |
| `especificacao-openapi-leva-o-token` | 1/1 | ✅ provada | 28,6 s |
| `debug-da-ligacao-mostra-a-senha` (nomes consertados, achado 2) | 1/1 | ✅ provada | 32,9 s |
| `cifra-do-fio-imposta` (de carona) | 1/1 | ✅ provada | 4,5 s |

`9 guardas: 9 provadas, 0 redundantes, 0 nao pegaram, 0 estragaram, 0
quebradas`. As entradas de `phxsql-server --lib` custam 28,6–32,9 s cada
com a cópia quente; as de `phxsql-core` e `phxsql-odbc`, 1,4–2,3 s. Os
`seguem` de cada uma ficaram verdes — inclusive o genérico do `config.rs` nas
duas entradas em que a lista dele não alcança, que é o achado 3 visto pelo
provador e não só pela sonda.

**Cobertura, pela receita refeita (leitura larga com `contem`, 40 provas):**
no `caem` de alguma entrada antes desta leva **9** (oito provadas e a décima
quebrada); depois **16**, e a décima provada. As **24** que continuam sem
guarda estão listadas pela própria receita (`--lista`); as mais próximas de
uma entrada são `a_senha_do_rele_nunca_aparece_no_json` (mesma saída da
`cifra-reserializa-a-senha`, e por isso não entrou — seria a segunda por
struct), `o_show_server_settings_nao_vaza_segredo` (SQL, custa 30 s por
prova no `servidor.rs`) e `a_senha_nao_vai_para_o_disco` (§15.7.6: é
território da §15.2).

## 16. `TETO_DEBUG_COM_SEGREDO` — a régua da lei que a guarda só exemplificava

**O defeito que motivou** (16/09/2026): nove structs em três crates
(`phxsql-server` ×7, `phxsql-sql`, `phxsql-odbc`) derivavam `Debug` carregando
14 campos de senha, token e hash — um `{:?}` no `Config` despejava oito de uma
vez. Consertadas em `74de67e`. E a guarda `debug-da-cifra-mostra-a-senha` **já
existia**, com a lei inteira escrita no `porque`: ela travou UMA struct, não a
lei (`docs/cognicao/cognicao_guarda-trava-a-struct-nao-a-lei_20260917_0010.md`).
Com a `debug-da-ligacao-mostra-a-senha` eram 2 structs com catraca, de 9 — e
nada impedia a décima de nascer derivando.

**A régua**: `bancada/guardas/debug-com-segredo.py` varre `crates/*/src/**/*.rs`
e conta **campos** que passam pelo crivo de três partes da §16 do
`SEGURANCA.md` — nome no léxico, tipo portador de valor, e a leitura declarada
numa lista visível de isenções (7 entradas, cada uma com o motivo; entrada
morta ou ambígua reprova) — e cujo `Debug` os imprime: por `derive`, **ou por
`impl` à mão que lê o campo** (a troca literal da guarda velha). Enxerga
`derive` em várias linhas, `cfg_attr`, enums e structs de tupla, e passa por um
lexer que apaga comentário e literal — sem ele, `.field("senha", &"(oculta)")`
contaria como vazamento.

**Medido em 17/09/2026**: **0** (teto 0). 36 campos passam por nome e tipo: 0
contam, 10 isentos, 26 não vazam por esta saída. Nasceu em zero e só desce.

**A prova, nos dois sentidos**: com o `conexao.rs` de antes do conserto, byte a
byte, acusa `Receita.token` e `Receita.senha` (SUBIU 2); com a troca da guarda
da ligação aplicada, acusa `Definicao.senha` e `Definicao.token` pelo caminho
do `impl` (SUBIU 2); limpa, 0. Os sete isentos não contam. Reproduzida pelo
integrador antes do commit `db18c87`.

**E a mutação, que é o que esta régua ensinou**: seis cópias, cada uma com uma
parte do crivo desligada. Quatro mudam o número medido (370, 17, 9, 5); **duas
não mudam nada** — sem detectar o `derive` ou sem ler o `impl`, a régua mede
**0**, igual à árvore sã. O autoteste (26 casos) roda por isso **dentro do
`--catraca` e do `--numeros`**: régua morta responde «não rodou» ao inventário,
nunca «0, em cima, sem folga». Ver
`docs/cognicao/cognicao_catraca-que-nasce-em-zero-nao-distingue-regua-morta_20260917_0038.md`.

**O que ela NÃO vê, declarado**: segredo em campo cujo nome não casa o léxico
(`Direcao { k }` no `fio.rs`); `impl Display`; caminho indireto no `impl`;
`examples/` e `tests/`. Quem a roda: o `bancada/catracas/todas.py` (§18) — na bateria e no
`comunicacao.sh` — e o inventário do
`docs/qa/medir.py`, que a achou sozinho pelo `catraca:nome=`. Não roda no
`cargo test`.

## 17. `TETO_TXT_CRU_EM_HTML` — o texto de tela chegando cru ao `innerHTML`

**Nasceu em 1, e o 1 tem endereço.** Medida em 18/09/2026, no fecho do pedido
**347**.

| | |
|---|---|
| onde | `crates/phxsql-server/src/conferidor_texto_cru.rs` |
| teto | **1** |
| medido hoje | **1** |
| folga | **0** |

### O defeito que a motivou

O texto de tela **não é constante do programa**: ele vem de
`phxsys.mensagens`, que é tabela **comum** do motor. Não existe conceito de
database de sistema — só `e_coluna_de_sistema`, que é de coluna —, então
gravar ali pede `alterar` naquele database, e **não** `administrar`. Quem tem
`alterar` executa script no navegador de quem abrir a tela, **inclusive de
quem tem mais poder que ele**. Não é auto-XSS.

A CSP não é mitigação e este é o ponto: `<script>` injetado por `innerHTML`
não roda, mas `script-src 'unsafe-inline'` deixa um manipulador de evento
rodar, e `connect-src 'self'` permite o `fetch` para o próprio servidor — a
exfiltração é 100% conforme a CSP.

**Antes → depois, medido no arquivo:** `${txt(` sem escape caiu de **9 para
1**; os `${esc(txt(` subiram de **443 para 451**. O 443 é o número que mais
diz: a casa **já escapava 443** e deixou 8 atrás. Não era prática ausente —
era o irmão que ficou.

### Por que 1 e não 0, com o motivo escrito

O 1 é `ui/index.html:9726`, a interpolação de `tela.st_ms_ou_mais` dentro do
`rot` do gráfico de distribuição. Ali o `txt()` entra cru num pedaço que é
escapado **um nível acima**, no `esc(rot)` que vai para o SVG — conferido
linha a linha. **Não é furo.**

Zero foi **recusado com motivo**: exigiria escapar dentro do `rot` e tirar o
`esc(rot)` de fora, criando uma string «já escapada» que o próximo a
concatenar não saberia que é. Trocar um furo real por uma armadilha futura não
é conserto.

### O que ela NÃO cobre, e quem cobre

Ela acha a forma `${txt(...)}` — oito dos nove sítios do 347. Ela **não** acha
o nono: no `phx-grid.js` o que vazava era `cA.titulo`, um título que
**viajou** — saiu de `txt()` noutro lugar, foi guardado num objeto de
configuração e só depois virou HTML. Nenhuma varredura de texto liga as duas
pontas, e fingir que liga seria pior que não ter guarda.

Esse lado é segurado por `testes-web/prova-xss-do-texto-de-tela.mjs`, que
carrega o arquivo real do componente num Chromium, repõe o defeito num clone e
**exige que o veneno dispare nele** — declarando-se INVÁLIDA, e não verde, se
não disparar.

**Dizer o que a guarda não cobre é parte da guarda**, e foi a omissão disso
que deixou o 347 nascer: o comentário do `index.html` já dizia que aquele
texto é entrada de usuário, e a docstring de
`nenhum_texto_da_fabrica_traz_etiqueta_crua` afirmava que «os dois caminhos
escapam antes de escrever». Existiam **quatro**. E a guarda que existia varre
a `FABRICA_TELA` — o que o programador escreveu —, enquanto o que vaza é o que
o **banco** devolve.

## Os limites de funcionamento encontrados (não são catracas)

Achados varrendo `TETO`, `MAX` e `LIMITE` em `crates/*/src/**/*.rs` e em
`bancada/`. Cada um trava um comportamento do motor em produção — memória,
tamanho de alocação, profundidade de recursão — e não mede dívida de código.
Nenhum tem "folga" porque nenhum é contado contra o código-fonte.

| Constante | Onde | Valor | O que protege |
|---|---|---:|---|
| `TETO_DA_CASCATA` | `phxsql-store/src/table.rs:81` | 16 níveis | recursão sem fundo em `ao_alterar` cascateado |
| `TETO_DO_REGISTRO` | `phxsql-core/src/fio.rs:494` | 128 MiB | tamanho corrompido de registro não aloca a memória toda da máquina |
| `TETO_PIVOT` | `phxsql-server/src/servidor.rs:15690` | 5.000.000 | teto do `max` de linhas pedido num pivot |
| `TETO_JUNCAO` | `phxsql-server/src/servidor.rs:15692` | 500.000 | linhas do lado que entra inteiro na memória numa junção |
| `TETO_DO_LOTE_SERVIDO` | `phxsql-server/src/servidor.rs:436` | 16 MiB | tamanho do lote de eventos servido de uma vez à réplica |
| `TETO_DO_CAMPO` | `phxsql-server/src/profiler.rs:118` | 120 bytes | truncamento de campo (`op`, `database`, `tabela`, `usuario`) na linha do profiler |
| `TETO_DO_ERRO` | `phxsql-server/src/profiler.rs:122` | 500 bytes | truncamento do texto de erro no profiler |
| `TETO_DO_CABECALHO` | `phxsql-server/src/profiler.rs:128` | 400 bytes | truncamento da descrição do filtro no cabeçalho/rodapé do profiler |
| `MAX_ARQUIVOS_ANTIGOS` | `phxsql-server/src/profiler.rs:134` | 32 arquivos | teto de rodízio do profiler (32 × 64 MiB = 2 GiB) |
| `MAX_CABECALHO` | `phxsql-server/src/http.rs:121` | 16 KiB | pedido HTTP malformado não consome memória |
| `MAX_CORPO` | `phxsql-server/src/http.rs:123` | 4 MiB | corpo do pedido HTTP |
| `CADEIA_MAXIMA` | `phxsql-server/src/servidor.rs:15585` | 8 | corrente de gatilhos (`AFTER INSERT ON t` gravando em `t`) sem fim |
| `LIMITE_ABERTOS_PADRAO` | `phxsql-store/src/volume.rs:24` | 64 volumes | descritores de arquivo abertos ao mesmo tempo |
| `VALOR_MAX` | `phxsql-store/src/trilha.rs:107` | 1.024 bytes | tamanho do valor antes/depois gravado na trilha LGPD |
| `COLUNA_MAX` | `phxsql-store/src/trilha.rs:109` | 2.000 bytes | nome (ou lista) de coluna na trilha LGPD |
| `IDENTIDADE_MAX` | `phxsql-store/src/trilha.rs:111` | 512 bytes | identidade da linha/critério na trilha LGPD |
| `IP_MAX` | `phxsql-store/src/trilha.rs:113` | 64 bytes | endereço de origem na trilha LGPD |
| `IMAGEM_MAX` | `phxsql-store/src/log.rs:90` | 64 MiB | tamanho corrompido de `tam_imagem` não aloca a memória toda |
| `MOTIVO_MAX` | `phxsql-store/src/motivo.rs:68` | 2.000 bytes | texto do motivo no `.reason` |
| `IDENTIDADE_MAX` | `phxsql-store/src/motivo.rs:70` | 512 bytes | identidade no `.reason` (constante distinta da de `trilha.rs`, mesmo nome) |
| `OFFSET_MAXIMO` | `phxsql-core/src/value.rs:87` | 2⁴⁸−1 | maior offset representável num volume externo (formato) |
| `MAX_SAIDA` | `phxsql-core/src/hkdf.rs:29` | 255 × SHA-256 | maior saída que o HKDF produz (limite do RFC 5869) |
| `CASAMENTO_MAX` | `phxsql-core/src/zip.rs:102` | 258 bytes | comprimento máximo de casamento no DEFLATE (limite do formato) |
| `RABO_MAXIMO` | `phxsql-core/src/zip.rs:715` | 65.557 bytes | quanto reler do fim do ZIP para achar o fim central (limite do formato) |
| `LARGURA_MAX` | `phxsql-cmd/src/lib.rs:55` | 40 caracteres | largura de coluna no `phxsql-cli`, para não estourar a tela |
| `PASSOS_MAX` | `phxsql-sql/src/rotina.rs:50` | 1.000.000 | passos que um gatilho/rotina PL roda antes de ser interrompido |
| `TEXTO_MAX` | `phxsql-sql/src/rotina.rs:69` | 64 MiB | alocação de texto de UM passo do avaliador PL |
| `TETO` (paralelismo) | `phxsql-core/src/paralelo.rs:39` | dinâmico (`recursos.threads`) | núcleos que o trabalho dividido pode usar; 0 = sem teto |
| `Ritmo::TETO` | `phxsql-server/src/replica.rs:519` | 60 s | topo do recuo exponencial da réplica entre reconexões |

28 constantes — a última achada em 16/09/2026 pela própria varredura de órfãs
do `docs/qa/medir.py`, que a acusa como `TETO` sem medidor. Ela é `pub const
TETO*` dentro de `src/`, então cai no crivo; é **limite de funcionamento**, e
está aqui em vez de virar catraca.

Nenhuma delas tem um conferidor que meça uma contagem no código-fonte contra
ela. `docs/qa/medir.py` já sinaliza um pedaço disto sozinho — varre `pub const
TETO*` e hoje acusa **dois**, o `TETO_DO_REGISTRO` e o `Ritmo::TETO` —, mas o
alcance dele é mais estreito que esta varredura: das outras
26, sete usam o prefixo `TETO_` mas são `const` privado (`TETO_DA_CASCATA`,
`TETO_PIVOT`, `TETO_JUNCAO`, `TETO_DO_LOTE_SERVIDO`, `TETO_DO_CAMPO`,
`TETO_DO_ERRO`, `TETO_DO_CABECALHO`) — invisíveis ao regex por causa da
visibilidade —, e as outras dezenove nem tentam começar com `TETO` (`MAX_*`,
`LIMITE_*`, `VALOR_MAX`, `OFFSET_MAXIMO`...) — invisíveis por causa do NOME,
independente de serem `pub`. A tarefa pedia varrer `TETO`, `MAX` e `LIMITE`
justamente por isso: uma régua que só olha um prefixo mede menos do que
existe, do mesmo jeito que a catraca de tabelas subcontava antes de aprender
a ver o ajudante `tabela(`.

### Achado auxiliar: dois tetos que a mesma rodada de hoje ainda estava escrevendo

O `git log` mostra dois commits de hoje (`5765228`, `63def9e`) acrescentando
justamente este tipo de limite — o prazo de parede do `Contexto::com_prazo`
(500 ms, reaproveita `transacao_lock_timeout_ms`) e o `TEXTO_MAX` do
avaliador PL — para o gatilho `BEFORE` sem fundo não derrubar o servidor
inteiro. Confirma a classificação: são limites de funcionamento novos,
motivados por um `SET s = CONCAT(s, s)` que dobra o texto a cada volta e
estoura o alocador antes do teto de passos alcançar; nenhum dos dois é
contado contra o código-fonte, e nenhum entra na tabela de catracas.

## O que ficou de fora desta varredura, e por quê

- **`conferidor_dependencias.rs`** (zero dependências externas) — portão
  binário, não catraca: não há contagem, é passa/não passa. Documentado na
  seção acima.
- **`bancada/guardas/catalogo.py`** (o catálogo de defeitos repostos, **177
  entradas** medidas em 17/09/2026) — é a OUTRA metade do papel G, as guardas
  de regressão provadas por mutação. Não é catraca: cada entrada prova um
  defeito específico voltando e sendo pego, não uma contagem que sobe e desce.
  Tem seu próprio inventário em `docs/TESTES.md` §12 e não se repete aqui.
  **O tamanho dele, esse sim, virou número travado** em 16/09/2026: é o
  `PISO_DAS_ENTRADAS` da §12.2, e o 77 que esta linha trazia até hoje é a
  demonstração de que um número digitado em prosa envelhece calado. **E ele
  envelheceu de novo no mesmo dia:** a catraca foi de 143 a 145 numa frente
  vizinha e este documento continuou dizendo 143 — travar o número no código
  não o publica. Enquanto estes três não saírem de um gerador, quem mexe no
  `PISO_DAS_ENTRADAS` atualiza esta seção no mesmo passo.
- **Os três portões** (`cargo fmt --check`, `clippy -D warnings`, `cargo
  test --workspace`, `docs/PORTOES.md`) — estruturais, sem folga numérica.
- **As catracas de CONTAGEM NO FONTE dentro de `#[test]`** — hoje
  `so_um_lugar_toma_a_trava` (uma `write()` e uma `read()` da trava de dados,
  cada uma dentro da função que a batiza) e
  `so_uma_operacao_usa_a_ficha_compartilhada` (teto **1**: só o `varrer` toma
  a ficha de leitura; a segunda leva entra medida), ambas em
  `phxsql-server/src/servidor.rs`. Contam ocorrências no próprio fonte pelo
  `include_str!`, como as catracas acima, mas não moram numa constante `TETO*`
  e por isso o `grep` da metodologia não as acha. **Ficam nomeadas aqui para
  que este inventário não seja lido como completo** — lei que lista menos
  casos do que existem protege igual hoje e menos no dia em que alguém usar a
  lista como inventário.
- **`bancada/concorrencia/escolher-o-desenho.py`,
  `bancada/profiler/sonda-log.py`, `bancada/cifra-do-fio/prova.py`** — usam
  "teto"/"limite" em prosa de bancada de desempenho (teto teórico de
  paralelismo, teto de latência do profiler, overhead do Base64), não são
  conferidores de qualidade de código. **O `mapa-da-trava.py` saiu desta
  lista**: ele estava aqui com razão em 03/09, e deixou de estar em `9fe9cc4`,
  quando ganhou três catracas de verdade — hoje são elas e as duas do
  `mapa-das-threads.py`, catalogadas na §13. Entrada de catálogo que envelheceu
  é o defeito da própria §12, visto de dentro.
- **`crates/phxsql-store/tests/tabela.rs:16`** (`const LIMITE: usize = 3`) —
  falso positivo do grep: é o índice da coluna "limite" (limite de crédito)
  num schema de teste, sem relação nenhuma com catraca.
- **`crates/phxsql-server/src/catalogo.rs:71`** (`const MAX: Parametro =
  ...`) — falso positivo: é a descrição do parâmetro `"max"` do protocolo
  (documentação do catálogo de operações), não um teto numérico.

## 18. `bancada/catracas/todas.py` — UM comando que roda todas as catracas em Python

**O defeito que a motivou** (pedido 476, cognição de 24/09/2026,
`docs/cognicao/cognicao_catraca-que-so-roda-dentro-da-bateria-nao-segura-a-integracao_20260924_0455.md`):
as cinco réguas em Python (§13 e §16 — os dois mapas de concorrência, o
`Debug` com segredo, o `pkill` sem PID e o catálogo envelhecido) só eram
chamadas de dentro do **item 0 da `bancada/bateria/prova-bateria.py`**, cada
uma com a própria linha de `subprocess.run` escrita à mão — e algumas
também de dentro do `comunicacao.sh`, com uma **segunda** lista, mais curta
(só os dois mapas). O commit `de4ca0a` subiu a `debug-com-segredo.py` de 0
para 1 e passou por `trecho-vivo.py --catraca`, pela suíte e pelo clippy sem
reprovar nada — porque nenhum dos três chama a catraca certa pelo nome. Só a
bateria completa, dias depois, achou.

**O que ele faz**: varre todo `.py` do repositório atrás de uma linha que
junte `sys.argv` (ou `add_argument`) com o texto `--catraca` — é assim que um
script DECLARA o próprio modo `--catraca`, e não apenas o menciona ao chamar
outro (um `subprocess.run([sys.executable, alvo, "--catraca"])` carrega
`sys.executable`, nunca `sys.argv`, na mesma linha). Hoje isso acha as
**cinco** de sempre; uma catraca nova em Python entra na próxima corrida sem
editar nenhuma lista. Roda cada uma e julga pelo **código de saída** (0 =
segura, != 0 = não segura) — nunca pela prosa, a mesma lei que o `--numeros`
de cada régua já segue.

**As TETO_\* do Rust** já são conferidas por `cargo test`; o comando lista
toda `const TETO\w*` achada em `crates/**/*.rs` (src e examples) — **33**
hoje — e diz, para cada uma, qual `#[test]` (no próprio arquivo ou nos
`tests/*.rs` do mesmo crate) cita o nome dela: **19 achou candidato, 14
não**. **Ele NÃO roda `cargo test` por padrão** — custo medido em
24/09/2026: `cargo test -p phxsql-server --lib -- --list` (arrasta
phxsql-core, phxsql-store e phxsql-sql, onde mora a maioria das TETO_*)
levou **~19 s FRIO**, e continuou perto disso mesmo sem nenhuma linha mudar
— o custo é o build de teste dos quatro crates, não a execução. Dezenove
segundos é de mil a duas mil vezes o custo das cinco catracas em Python
somadas (medido: **~7,9 s** as cinco juntas, numa corrida da árvore limpa —
3,9 s do mapa da trava, 1,0 s do mapa das threads, 2,1 s do `Debug` com
segredo, 0,04 s do `pkill`, 0,3 s do catálogo). Rodar o Rust em toda chamada
tornaria o comando caro demais para o uso que o motiva — todo commit, pelo
integrador. Quem quiser a conta na hora tem o `--com-rust`: builda e roda
`cargo test --lib` dos crates com alguma TETO_*, medido em **~48 s** nesta
árvore (`phxsql-core` + `phxsql-store` + `phxsql-server` + `phxzip`, frio).

**A diferença entre REPROVADA e QUEBRADA**: as duas reprovam a chamada
(código de saída != 0), mas o relatório diz qual é qual, porque só quem
escreveu a régua sabe explicar o próprio número. QUEBRADA acontece em três
casos — silêncio total (stdout e stderr vazios: o defeito nomeado pelo
próprio pedido, "medidor quebrado não é verde" — um script que capturasse
toda exceção e devolvesse `None`, que em Python vira saída 0, passaria por
aprovado sem este crivo), exceção (saída != 0 com um `Traceback` no stderr),
ou prazo estourado (padrão 120 s).

**Onde ele passou a ser chamado** — mesmo motor, nunca duas listas (lei do
dono, 23/09/2026):

- `bancada/bateria/prova-bateria.py`: o antigo `item_0_catraca_do_mapa` e o
  `item_0c_catraca_do_mapa_das_threads` viraram um só, `item_0_todas_as_
  catracas`, que chama o comando e gera um `confere()` por catraca (rótulo
  amigável por um dicionário só cosmético — a decisão de quais scripts SÃO
  catraca continua vindo exclusivamente da varredura). As duas catracas que
  moravam dentro do `item_0b` (`pkill` sem PID e catálogo envelhecido) saíram
  de lá — moviam-se com o resto para o item 0, estático, antes do servidor.
- `comunicacao.sh` (bloco "as catracas que só rodavam na bateria"): a lista
  fechada de dois mapas virou uma chamada ao mesmo comando — o mesmo buraco
  que motivou o pedido 476 também existia aqui: só os dois mapas cabiam no
  batimento de 15 min, e as outras três só apareciam na bateria completa.
  Custo das cinco somadas (~8 s) cabe no mesmo orçamento que os dois mapas
  cabiam sozinhos.

**A prova real, nos dois sentidos, numa CÓPIA da árvore** (nunca na viva):

- **(a) defeito reposto sobe a catraca certa.** Trocado o `impl Debug` manual
  de `Opcoes` (`crates/phxzip/src/escritor.rs`) por `#[derive(Debug)]`: o
  comando reprova nomeando `crates/phxzip/src/escritor.rs:61  Opcoes.senha:
  Option<String>  (derive(Debug))`, `SUBIU 1 (teto 0)`, saída 1. Desfeito, as
  cinco voltam a `ok`, saída 0.
- **(b) catraca nova entra sem editar lista nenhuma.** Um script sintético
  (`if "--catraca" in sys.argv: sys.exit(1)`) apareceu na varredura (`--lista`
  passou a mostrar **6** réguas em vez de 5) e foi rodado e reprovado
  (`REPROVADA`) sem uma linha de configuração tocada.
- **(c) medidor mudo reprova como QUEBRADA.** Um segundo script sintético
  (`if "--catraca" in sys.argv: sys.exit(0)`, sem imprimir nada) saiu como
  `QUEBRADA (nao imprime veredito)`, e a chamada inteira terminou em saída 1
  — o silêncio sozinho já reprova, mesmo com código de saída 0 do filho.
- **Árvore limpa**: as cinco `ok`, 33 TETO_* listadas, saída 0.

## 19. `portoes.sh` — os portões de commit num código de saída só (pedido 421)

Em 23/09/2026 a árvore junta passou por `fmt`, `clippy` com zero avisos e
2.739 testes verdes, e subiu com a catraca do mapa da trava reprovada
(`alcancam-fsync-2 25`, teto 24). Os conferidores em Rust rodam dentro do
`cargo test`. As catracas em Python ficam fora dele e só rodavam quando alguém
lembrava. O `portoes.sh`, na raiz, roda os quatro passos sempre, mesmo depois
de um vermelho: `fmt --check`, `clippy -D warnings`, `test --workspace` e o
`todas.py`. Ele sai 0 só se os quatro saírem 0. O `-D warnings` é o que faz o
«zero avisos» virar código de saída.

- `--raiz DIR` roda numa árvore exata (a de `git archive`). Sem `.git` ali, ele
  aponta `PHXSQL_GIT_DIR` para o repositório do próprio script. Sem isso, a
  régua dos aprendizados reprova como «não conferível» toda evidência dada por
  commit. Foi assim que a primeira cognição FRUTÍFERA de commit, a do 484, deixou
  a árvore exata vermelha: medido, e consertado junto.
- `PHX_CARGO` escolhe qual cargo chamar (`./cargo-da-frente.sh` quando há
  frentes compilando).

**Prova real** (`bancada/catracas/prova-portoes.py`, ~25 s, com um cargo falso
porque o que se prova é a costura dos passos, não a suíte). Árvore limpa sai 0.
Suíte vermelha sai 1. Suíte verde com uma catraca de mentira que reprova sai 1,
nomeando «catracas». **Com o defeito reposto** (o mesmo script sem o passo das
catracas), o terceiro caso sai 0 e a prova acusa. Sem isso, ela passaria por
engano.

## Metodologia

1. `grep -rn "TETO\|MAX\|LIMITE"` em `crates/*/src/**/*.rs` e em `bancada/`,
   mais `grep -rln "catraca"` para achar prosa que não bate em nenhuma das
   três palavras (achou as duas catracas de idiomas pela documentação antes
   de achar pela constante).
2. Cada constante lida com o comentário ao lado para decidir: mede uma
   contagem no código-fonte que só deve encolher (catraca) ou trava um
   comportamento do motor (limite)?
3. Build limpo antes de medir (`cargo build --release --examples -p
   phxsql-server` — a regra do binário velho vale aqui: medidor com binário
   de ontem mede o passado).
4. Números tirados do próprio gerador (`docs/qa/medir.py`, que já existia e
   já resolve as quatro catracas de `TETO*` públicas por auto-descrição —
   `cargo run --release --example <nome> -p <crate> -- --numeros`), mais
   `cargo test -p phxsql-server --lib conferidor` como segunda prova
   independente. As duas bateram: 24 testes verdes, medido == valor nas
   quatro.
5. `docs/QA-PDCA.md` tem a mesma tabela, gerada pelo mesmo script — ela é
   redundante com este documento por construção (mesmo gerador), e é
   esperado que continue igual até a próxima rodada que mexer em tabela HTML
   ou em texto de tela.
