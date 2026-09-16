# Uma catraca sobre as marcas `// DIVIDA:` protege ou estraga?

Parecer do papel G (QA), **16/09/2026**, sobre o risco **R19** do
`docs/RISCOS.md`. Frente de leitura e medição: aqui não se conserta nem se
afrouxa nada — nomeia-se.

Árvore limpa em `f64b822` (`git status --porcelain` vazio no momento da
medição). Tudo abaixo é medido; onde não deu para medir, está dito que não deu.

---

## 1. O número, e a peneira que o produziu

**19 marcas**, em **15 arquivos** de **5 crates**.

A peneira é a do próprio gerador, `docs/status/riscos.py:254`, usada pelo
módulo importado e não recopiada aqui:

```python
MARCA_DIVIDA = re.compile(r"^\s*//[/!]?\s*DIVIDA:\s*(.+?)\s*$")
```

varrida sobre `crates/*/**/*.rs` (`varrer_divida()`, mesma função que a sétima
página chama). Reproduz com:

```bash
python3 docs/status/riscos.py docs/status/status-do-projeto.html
# ...  divida: 19 marcas `// DIVIDA:` em 15 arquivos de crates/
```

| por crate | marcas |
|---|---:|
| `phxsql-server` | 10 |
| `phxsql-sql` | 3 |
| `phxsql-store` | 3 |
| `phxsql-core` | 2 |
| `phxsql-odbc` | 1 |

**A peneira crua dá 11, e o erro tem tamanho exato.** `grep -c "// DIVIDA:"`
sobre `crates/` devolve **11**, não 19. E o que falta *não* são os `///`:
`/// DIVIDA:` contém `// DIVIDA:` como subcadeia e o `grep` cru o pega. O que
some são os **8 `//! DIVIDA:`** — as marcas de cabeçalho de módulo, que são
justamente as que descrevem a limitação do módulo inteiro. A distribuição
medida:

| estilo | quantas |
|---|---:|
| `/// DIVIDA:` | 10 |
| `//! DIVIDA:` | 8 |
| `// DIVIDA:` | 1 |

Quem contar com a peneira crua mede 11 e perde exatamente a metade mais
estrutural do inventário.

**Marcas que citam pedido: 5** — `#164` (Parcial), `#207` (Parcial), `#209`
(Planejado), `#229` (Parcial), `#268` (Planejado). As outras 14 existem **só no
fonte**: não há pedido no `docs/PENDENCIAS.md` apontando para elas.

> **Um número digitado já envelheceu, no mesmo dia.** A linha do pedido 264 no
> `docs/PENDENCIAS.md` diz «**4** citam pedido do dono (#164, #207, #209,
> #229, todos abertos)». São **5**: o `#268` entrou na marca de
> `crates/phxsql-server/src/config.rs:1217`. A prosa que criou o contador de
> dívida envelheceu em horas — e é a lei da casa se cumprindo contra quem a
> escreveu.

## 2. De onde elas vieram: a série tem UM ponto

A pergunta do briefing — «está crescendo, parada ou encolhendo nos últimos
dias?» — **não tem resposta, e a falta de resposta é o achado.**

```bash
git log --oneline -S"DIVIDA:" --all -- 'crates/**/*.rs'
# f64b822 Motor: 245 (O2-O6) e 267, e as 19 marcas de divida que moram junto
```

**Um único commit**, `f64b822`, de **2026-09-16 21:30:10 +0000** — poucas horas
antes deste parecer. Nenhum outro commit na história do repositório jamais
tocou uma marca `// DIVIDA:`.

Ou seja: **zero marcas acrescentadas depois, zero apagadas, zero pagas.** Não
há série, não há tendência, não há «últimos dias». As 19 nasceram juntas, de um
único passo de inventário.

Isto sozinho derruba metade do raciocínio de R19: não se pode dizer que «a
contagem sobe sozinha», porque ela nunca subiu — ela *nasceu*. Congelar hoje um
teto sobre o resultado do primeiro e único passo de inventário é congelar a
régua no dia em que ela foi ligada.

## 3. O que cada marca É — e são SETE espécies, não uma

Li as 19 no contexto. **Todas as 19 estão ao lado de uma frase que já explicava
a limitação antes de a marca existir** — nenhuma dívida foi criada pelo
inventário; ela foi tornada contável. Isso importa para a decisão e volta na §5.

E elas não são a mesma coisa. Agrupadas pelo que *pagar* significaria:

### E1 — Frente aberta: funcionalidade grande, planejada (5)

| onde | o que falta |
|---|---|
| `crates/phxsql-core/src/tipo_database.rs:62` | só o motor padrão existe; hive e vetorial têm tipo, marcador e portão de recusa |
| `crates/phxsql-core/src/vetor.rs:39` | a coluna `VECTOR<F32,N>` da frente V3 não existe |
| `crates/phxsql-odbc/src/lib.rs:1620` | falta o `SQLEndTran`; o driver só dirige em autocommit |
| `crates/phxsql-server/src/carga.rs:16` | adiar o `.ndx` no BULKINSERT — a terceira parte, a maior |
| `crates/phxsql-server/src/servidor.rs:19` (#164) | não há trava de arquivo nem de registro |

Paga-se construindo. É a única espécie que se comporta como dívida no sentido
comum — e ainda assim, cada uma custa uma frente inteira.

### E2 — Espera por demanda: YAGNI registrado (2)

| onde | o que falta |
|---|---|
| `crates/phxsql-server/src/dblink/sincronia.rs:204` | chave composta no espelho — *«fica para quando alguém precisar dela de verdade, com o pedido na mesa»* |
| `crates/phxsql-server/src/mensagens.rs:113` | `erro.versao_nao_suportada` sem tradução — *«tradução fica para quando alguém precisar dela de verdade»* |

**Pagar sem demanda seria construir o que ninguém pediu.** Um teto que conta
estas duas manda fazer exatamente isso.

### E3 — Recusa declarada: o motor RECUSA, e recusar é o certo hoje (3)

| onde | o que falta |
|---|---|
| `crates/phxsql-sql/src/consulta.rs:57` | correlação não-igualdade, `EXISTS` não correlacionado, CTE recursiva — *«a tradução recusa nomeando»* |
| `crates/phxsql-sql/src/consulta.rs:496` | `COUNT(*)`/`GROUP BY` sobre visão — *«recusam nomeando»* |
| `crates/phxsql-sql/src/traduzir.rs:28` | `SELECT` que nenhum índice responde inteira — *«recusa em vez de varrer»* |

A prosa de `traduzir.rs:28` diz, com todas as letras, por que a recusa fica:
*«responderia sobre a PRIMEIRA PÁGINA e teria a cara de ter respondido sobre a
tabela — resposta errada com cara de certa, que é pior que erro»*. Um teto
apontando para esta linha é **uma catraca apontando para a correção**.

### E4 — Declara mais do que o disco cumpre (4)

| onde | o que falta |
|---|---|
| `crates/phxsql-server/src/config.rs:396` (#207) | o quórum é guardado e não imposto |
| `crates/phxsql-server/src/config.rs:1217` (#268) | marcar a tabela não cifra o que já está gravado — *«a lista declara mais do que o disco cumpre»* |
| `crates/phxsql-store/src/ledger.rs:78` | a coluna da assinatura existe no esquema e ninguém a preenche |
| `crates/phxsql-store/src/restaurar.rs:53` | a restauração não confere assinatura |

**Esta é a espécie que realmente só deveria encolher.** É a família da pétrea
«configuração que não é lida mente» — o `recursos.cache_paginas` que passou
três versões no arquivo, no MANUAL e na tela sem uma linha de código o lendo. A
marca de `config.rs:396` até cita esse precedente na prosa acima dela.

### E5 — Consequência de pétrea: o dado velho não se reescreve (2)

| onde | o que falta |
|---|---|
| `crates/phxsql-server/src/config.rs:1163` | ligar a cifra não alcança o diário já em claro — *«um append-only não se reescreve»* |
| `crates/phxsql-server/src/profiler.rs:41` | marcar tabela não apaga o `perfil.txt.N` já gravado |

**Pagar estas duas exige reescrever arquivo append-only** — e a pétrea da ordem
de digitação e do append-only é o que impede. Um teto que as conta é uma
catraca cujo caminho para o verde passa por cima de uma pétrea.

### E6 — Decisão pendente do dono (2)

| onde | o que falta |
|---|---|
| `crates/phxsql-server/src/servidor.rs:7671` | a porta MCP por HTTP sem escrita e sem login — *«fica para a fase 2, com o dono»* |
| `crates/phxsql-store/src/table.rs:3008` (#229) | *«falta decidir entre faixa por nó e contador durável propagado»* |

**Nenhum engenheiro fecha estas duas sozinho.** Um teto que as conta prende a
suíte a uma decisão que não é de quem programa.

### E7 — Alcance de guarda incompleto (1)

| onde | o que falta |
|---|---|
| `crates/phxsql-server/src/conferidor_temporarios.rs:36` (#209) | o conferidor não varre `examples/` |

Dívida de QA sobre a própria catraca. Paga-se, e é nossa.

### O número que decide

| paga-se com engenharia? | espécies | marcas |
|---|---|---:|
| **sim** | E1, E4, E7 | **10** |
| **não** — ou pagar é errado, ou é proibido, ou não é nosso | E2, E3, E5, E6 | **9** |

**9 das 19 marcas — 47% — não se pagam por trabalho de engenharia.** Um teto
único sobre 19 mede a soma de duas coisas, e a segunda não obedece a teto
nenhum. A hipótese do briefing («se forem duas espécies misturadas, uma catraca
única não significa nada») está confirmada com número: são sete espécies, e
quase metade da população é imune à régua.

## 4. O que uma peneira NÃO consegue: a completude

Tentei responder «19 é a dívida, ou é uma amostra?». **Duas peneiras, as duas
morreram medidas.** Registro as duas porque hipótese que morre medida é o que
impede a mesma ideia de voltar sem medição.

**Peneira larga** — linha de comentário (`//`, `///`, `//!`) contendo
`ainda não | não existe | fica para quando | fica para a fase | por enquanto |
frente aberta | não está feit | não suportad | recusa nomeando | falta o |
falta a`: **455 linhas em 112 arquivos**, dos quais **98 sem marca nenhuma**.
Parecia enorme. Amostrei 25 ao acaso (semente 19) e li uma a uma: **nenhuma das
25 é dívida não marcada.** São descrições de comportamento — «campo que não
existe devolve `None`», «3001 quer dizer “esse índice não existe”», «job que
falhou calado não existe: a linha entra igual». Precisão ≈ 0.

**Peneira estreita**, com as frases que a prosa das próprias 19 usa
(`fica para quando | frente aberta | não está feit | falta decidir |
não há migra | ficou para | continuam por fazer`…): **27 candidatos fora do
raio de 12 linhas de uma marca**. Lidos: **1 legítimo**. É
`crates/phxsql-store/src/catalogo.rs:518` — a mesma dívida do motor hive/vetorial
já marcada em `tipo_database.rs:62`, no **arquivo irmão, sem marca**. Precisão
1/27, e o único achado é *«conserto entra no caminho que o motivou, e o caminho
IRMÃO fica»* aplicado à marca em vez de ao conserto.

**Conclusão medida: não existe régua de texto que audite a completude deste
inventário.** O denominador é desconhecido e permanece desconhecido. Uma
catraca sobre o numerador de uma fração cujo denominador ninguém consegue medir
é precisamente a «lei que lista menos casos do que existem»: não protege menos
hoje, protege menos no dia em que alguém ler o 19 como inventário.

## 5. O precedente da casa: por que o `PISO_DAS_ENTRADAS` sobe

`bancada/guardas/trecho-vivo.py`, `PISO_DAS_ENTRADAS`. É a única régua desta
casa que sobe, e o argumento dela está escrito no cabeçalho do arquivo:

> «`TETO_TRECHO_MORTO` conta **trecho morto**, não **guarda viva**. Apagar as
> oito entradas do `catalogo.py` teria medido exatamente o mesmo `0` que
> consertá-las — e apagar é o caminho barato. Uma catraca que premia o
> apagamento igual ao conserto não segura o catálogo; segura só a aparência
> dele.»

E a metade que o briefing não cita, que é a que faz o piso funcionar: ele conta
**vivas + APOSENTADAS escritas**. Sem essa lista, um piso rígido proibiria
aposentar uma entrada cuja lógica deixou de existir — e guarda impossível de
aposentar vira entrada remendada no chute, pior que entrada nenhuma. **A
aposentadoria se ESCREVE**, e é isso que muda o preço relativo: consertar
continua custando ler o código, e apagar passa a custar escrever por quê.

> **O valor dela mudou enquanto eu escrevia este parecer, e vale registrar.**
> Medi `160` (catálogo com 160 entradas vivas, `APOSENTADAS` vazia) no começo
> desta frente; minutos depois a régua imprimia `PISO_DAS_ENTRADAS: 169
> (piso 169)`, porque uma frente paralela acrescentou nove guardas e subiu o
> piso no mesmo passo — que é a lei sendo cumprida, não quebrada. O número
> aqui é o do momento da medição, e o de hoje sai de
> `python3 bancada/guardas/trecho-vivo.py --catraca`. Nesta base um número
> digitado já envelheceu em noventa minutos por duas frentes mexerem na mesma
> catraca sem se verem; este envelheceu em menos.

**A marca `// DIVIDA:` é da família das ENTRADAS, não da dos DEFEITOS**, e isso
é medido e não opinado: as 19 estão todas ao lado de prosa que já existia (§3),
e o `f64b822` não criou dívida — criou contabilidade. A dívida existia ontem, e
ontem valia 19 do mesmo jeito; o que mudou foi alguém ter escrito.

Contar marcas **não** mede dívida. Mede **quanto do inventário foi feito.**

## 6. Recomendação: **(b) piso, e não (a) teto** — mais **(c)** na forma

### (a) teto que desce — DERRUBADA, com número

Um `TETO_DIVIDA = 19` reprovaria quem acrescentasse a vigésima marca. O teste
do incentivo, que o briefing exige: *quem quer ficar verde amanhã, o que faz?*
**Apaga uma marca, ou não marca a dívida que acabou de achar.** Dos dois
caminhos, o segundo é grátis e invisível: ninguém consegue provar que uma marca
faltou (§4 — precisão 0/25 e 1/27). A catraca puniria o único comportamento que
a seção existe para premiar, e o caminho barato para o verde seria sumir com o
inventário anunciando sucesso.

E o segundo argumento, independente do incentivo: **9 das 19 não se pagam**
(§3). Para 2 delas (E5) o caminho do verde atravessa a pétrea do append-only;
para 3 (E3) atravessa a correção do motor; para 2 (E6) depende de decisão do
dono; para 2 (E2) manda construir o que ninguém pediu. Um teto aqui não é uma
catraca frouxa — é uma catraca **apontada para o lado errado**.

### (b) piso que sobe, no molde do `PISO_DAS_ENTRADAS` — **é esta**

Um `PISO_DAS_MARCAS`, contando **marcas vivas + BAIXAS escritas**, no número
medido do dia (**19**), com uma lista irmã da `APOSENTADAS` em que cada linha
traz *onde estava, a data e o motivo*, e o motivo aceita dois naipes:

- **paga** — a dívida deixou de existir, e a linha diz o que a fechou;
- **não era dívida** — a marca foi reclassificada, e a linha diz por quê (E3 e
  E5 são candidatas honestas a isso; que a reclassificação seja **escrita** em
  vez de silenciosa é ganho, não custo).

Apagar uma marca sem escrever a linha faz a soma cair e **reprova, nomeando
quantas sumiram**. Acrescentar uma marca faz a soma subir, e o piso sobe junto
**no mesmo commit** — espelho exato do «DESCEU — BAIXE O TETO», e o mesmo que o
`PISO_DAS_ENTRADAS` já faz (ele subiu 143 → 150 → 160 → 169 ao longo de
16/09/2026, sempre no passo que fez o catálogo crescer).

**Esta régua não viola «catraca só desce»: ela não é catraca de dívida.** É a
mesma exceção declarada que a casa já aceitou uma vez, pelo mesmo argumento e
não por analogia — conta entradas, não defeitos.

**Por que agora, e não «quando houver um apagamento»** — o defeito que a motiva
já está medido nesta árvore, na mesma classe e com a mesma causa:

> `2fe8658` (12/09) tocou `table.rs` e `transacao.rs` e **aposentou cinco
> guardas de uma vez**. Ninguém percebeu por **quatro dias**.

As 19 marcas moram em 15 arquivos que, nos **últimos 14 dias**, receberam
**197 commits** — `servidor.rs` 81, `table.rs` 36, `config.rs` 24. O
`catalogo.py`, que o piso já protege, recebeu **54** no mesmo período. **As
marcas estão 3,6× mais expostas ao refator alheio do que o catálogo que a casa
já achou necessário proteger** — e uma marca é uma linha de comentário, que
some num rebase sem conflito e sem teste nenhum acusar.

### (c) na forma — a espécie tem de estar escrita

O gerador já para em duas coisas de forma (marca sem motivo de 30 caracteres,
marca citando pedido inexistente). Falta a terceira, e §3 é o argumento: hoje a
sétima página publica **19 marcas de dívida técnica** e quem lê entende «19
coisas a fazer». São 10. **Número que soma sete espécies não é número errado —
é número que informa errado**, e o remédio é de forma, não de teto: a marca diz
a espécie, e a página conta por espécie em vez de somar tudo numa barra.

### (d) nenhuma catraca — recusada, mas por pouco

Era a resposta defensável até a medição do churn. O que a derruba é o 197
contra 54 da §6(b): sem régua nenhuma, a única coisa que protege o inventário é
alguém reparar que uma linha de comentário sumiu de um arquivo que muda seis
vezes por semana. A casa já mediu que isso leva quatro dias — quando leva.

## 7. O incentivo que esta recomendação cria, em uma frase

> **Quem quer ficar verde amanhã paga a dívida e escreve a linha da baixa, ou
> deixa a marca onde está — e marcar dívida nova continua sendo grátis, porque
> o piso sobe junto no mesmo commit; o único caminho que reprova é apagar a
> marca calado.**

Se a resposta fosse «apaga a marca», a recomendação estaria errada. É o
contrário: apagar calado passa a ser **o único** ato que reprova.

## 8. Como se provaria, nos dois sentidos

**Falha com o defeito reposto.** Apague **uma** marca sem escrever a linha da
baixa — por exemplo a linha `//! DIVIDA:` de
`crates/phxsql-store/src/restaurar.rs:53`. A soma cai de 19 para 18, abaixo do
piso, e a régua reprova nomeando que **1** sumiu. É o defeito de `2fe8658`
reposto no catálogo novo: um refator apaga o registro e a contagem melhora.

**Passa com o conserto**, e são dois caminhos distintos, os dois têm de passar:

1. *Pagou* — a restauração passa a conferir assinatura, a marca sai **e** entra
   `{"onde": "crates/phxsql-store/src/restaurar.rs", "data": "…", "motivo":
   "paga — …"}`. Soma: 18 vivas + 1 baixa = 19. Passa.
2. *Marcou mais* — uma marca nova honesta entra; soma 20; o piso vai a 20 no
   mesmo commit. Passa.

**O controle negativo, que é o teste que mais importa aqui** — e é o do
comportamento *velho*, não o do novo, pela lei «guarda nova entra pedida, não
imposta»: **acrescentar uma marca NÃO pode reprovar em nenhuma hipótese.** Se
algum dia a régua reprovar quem marca, ela virou teto por acidente e esta
recomendação morreu junto. Esse teste trava a diferença entre (a) e (b), e sem
ele a diferença é só intenção.

**O que esta régua NÃO cobre**, dito junto do número para ninguém a ler como
inventário (a lei da §4):

1. **Troca.** Apagar uma marca e acrescentar outra mantém a soma. Não pega — é
   o mesmo buraco residual que o `PISO_DAS_ENTRADAS` tem.
2. **Completude.** Ela não diz nada sobre dívida que nunca foi marcada, e §4
   mostra que nenhuma peneira de texto diz.
3. **A dívida em si.** Ela conta marcas, não dívida. Ela **não** faz a dívida
   encolher, e anunciar que faz seria a mentira que R19 pediria.

## 9. R19 está mal redigido — e o conserto é do texto, não do código

R19 diz hoje: *«A contagem sobe sozinha e ninguém é obrigado a baixá-la — e
catraca é justamente o que impede isso.»*

A medição não sustenta isso como risco. A contagem **nunca subiu** (§2: um
commit, um ponto), e se subir amanhã é porque alguém marcou uma dívida que já
existia — o inventário andando, que é o comportamento desejado. **O risco
medido é o inverso do escrito:** a marca sumir de dentro de um arquivo que muda
81 vezes em 14 dias, sem que nada acuse.

Redação que a medição sustenta, para o dono e o papel H decidirem (não a
escrevo no `docs/RISCOS.md` — esta frente não conserta):

> *A dívida marcada no fonte pode ser **apagada** sem ser paga, e nada acusa: a
> marca é uma linha de comentário em arquivos que receberam 197 commits em 14
> dias, e a contagem **melhora** quando ela some. Um teto pioraria — puniria
> quem marca —, e 9 das 19 marcas não se pagam por engenharia. Sonda `divida`;
> régua proposta: piso sobre marcas vivas + baixas escritas.*

## 10. O que sobe indevidamente, e o que está vermelho hoje

**A catraca que subiria indevidamente se (a) fosse adotada** é a própria
`TETO_DIVIDA`: no primeiro dia em que uma frente honesta achasse uma dívida
nova, o argumento «a régua passou a enxergar mais» apareceria pronto — e é
exatamente o argumento que o `TETO_TABELA_NA_MAO` proíbe. Só que ali a régua
mudou de verdade; aqui quem mudou foi o mundo. Uma catraca que precisa subir
toda vez que alguém é honesto não é catraca: é pedágio.

**E há uma vermelha viva, medida agora**, que não é do meu escopo consertar mas
é do meu escopo nomear:

```
python3 bancada/concorrencia/mapa-da-trava.py --catraca
   SUBIU                    alcancam-fsync    23 (teto 22)
        Reprovado: a catraca nao sobe. Desfaca, ou traga o motivo ao dono.
REPROVADO.
```

Ela está **registrada** como vermelha por decisão do dono (#252, metade 1) no
comentário do próprio `CATRACAS` do arquivo — vermelha declarada é decisão, não
esquecimento —, mas continua sem resolução: 1 das 20 catracas medidas está
reprovando, e é o número que a sonda `catracas-reprovando` publica. **O teto 22
não pode ser levado a 23 para fechar isso**; a saída é desfazer ou levar o
motivo ao dono, como a própria régua imprime.

**E o catálogo de guardas não cobre nada disto:** das entradas vivas (160 na
minha medição, 169 poucos minutos depois), **todas** têm o `porque` escrito e
**zero** citam dívida técnica. R19 é, hoje, pétrea sem
guarda — e continua sendo até que a régua da §6(b) nasça com a prova dos dois
sentidos da §8.
