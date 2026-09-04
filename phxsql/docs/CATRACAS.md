# As catracas do PhxSql — inventário medido

Levantamento do papel G (QA) sobre **todas** as catracas numéricas da árvore:
o teto declarado, o valor medido hoje e a folga entre os dois. Reproduza com

```bash
flock /tmp/phx-cargo.lock cargo build --release --examples -p phxsql-server
flock /tmp/phx-cargo.lock python3 docs/qa/medir.py
```

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
`cargo clippy -D warnings`, `cargo test --workspace` e o conferidor de zero
dependências (`conferidor_dependencias.rs`) não têm "folga" — são
verdadeiro/falso, não contagem. Não entram na tabela abaixo pelo mesmo
motivo que `TETO_DA_CASCATA` não entra: não há número que decresça.

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

27 constantes. Nenhuma delas tem um conferidor que meça uma contagem no
código-fonte contra ela. `docs/qa/medir.py` já sinaliza um pedaço disto
sozinho — varre `pub const TETO*` e encontrou `TETO_DO_REGISTRO` sem
medidor —, mas o alcance dele é mais estreito que esta varredura: das outras
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
- **`bancada/guardas/catalogo.py`** (o catálogo de defeitos repostos, hoje
  com 77 entradas) — é a OUTRA metade do papel G, as guardas de regressão
  provadas por mutação. Não é catraca: cada entrada prova um defeito
  específico voltando e sendo pego, não uma contagem que sobe e desce. Tem
  seu próprio inventário em `docs/TESTES.md` §12 e não se repete aqui.
- **Os três portões** (`cargo fmt --check`, `clippy -D warnings`, `cargo
  test --workspace`, `docs/PORTOES.md`) — estruturais, sem folga numérica.
- **`bancada/concorrencia/escolher-o-desenho.py`, `mapa-da-trava.py`,
  `bancada/profiler/sonda-log.py`, `bancada/cifra-do-fio/prova.py`** — usam
  "teto"/"limite" em prosa de bancada de desempenho (teto teórico de
  paralelismo, teto de latência do profiler, overhead do Base64), não são
  conferidores de qualidade de código.
- **`crates/phxsql-store/tests/tabela.rs:16`** (`const LIMITE: usize = 3`) —
  falso positivo do grep: é o índice da coluna "limite" (limite de crédito)
  num schema de teste, sem relação nenhuma com catraca.
- **`crates/phxsql-server/src/catalogo.rs:71`** (`const MAX: Parametro =
  ...`) — falso positivo: é a descrição do parâmetro `"max"` do protocolo
  (documentação do catálogo de operações), não um teto numérico.

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
