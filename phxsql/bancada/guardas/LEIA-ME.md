# As guardas — provar que a prova pega

A casa exige que todo teste novo **falhe com o defeito reposto**. Isso sempre
foi feito à mão, uma vez, por quem escreveu o teste — e depois se perdia.
Ninguém conseguia dizer, hoje, quais das 1.259 asserções ainda pegariam o
defeito que as motivou.

```bash
python3 bancada/guardas/provar-guardas.py            # todas
python3 bancada/guardas/provar-guardas.py --listar   # o catálogo
python3 bancada/guardas/provar-guardas.py --so profiler
```

## A corrida COMPLETA é a última coisa da rodada

Provar a guarda quando ela entra (`--so <id>`) diz que ela **nasceu** certa.
Não diz que ela continua certa depois do que veio depois — e essa diferença já
custou três guardas nesta casa.

`fts-abrir-recusa-a-tabela` foi provada 1/1 e, **horas depois no mesmo dia**, a
correção da pista de leitura acrescentou um braço dentro do mesmo `match` que
ela cita: virou `QUEBRADA`, e continuou sendo contada no total. As duas do
Profiler tinham envelhecido numa rodada anterior, pelo mesmo mecanismo.

Nenhuma das três aparece em portão nenhum — `fmt`, `clippy` e a suíte inteira
ficam verdes. **Quem acusa é esta corrida**, e por isso ela roda no fim, depois
do último commit de código. Rodá-la no meio dá um retrato que o trabalho
seguinte invalida.

`docs/cognicao/cognicao_prova-individual-nao-sobrevive-ao-commit-seguinte_20260907_0710.md`


Três arquivos, e a divisão entre os dois primeiros é o ponto:

| arquivo | o que é |
|---|---|
| `catalogo.py` | **só dados**: cada defeito, o trecho de hoje, o trecho de antes, e quais testes têm de cair |
| `provar-guardas.py` | o executor: copia a árvore, repõe um defeito por vez, roda os testes nomeados, desfaz, e julga |
| `tabela-no-testes.py` | regrava a tabela das guardas no `docs/TESTES.md` a partir do `--json` de uma rodada — número visível que não sai de gerador está errado e ninguém percebeu ainda |

## O que sai

```
--- a arvore limpa, antes de qualquer defeito ---
  phxsql-server --lib                verde    8.5 s  540 testes
  phxsql-store  --lib                verde    0.7 s  133 testes
  phxsql-store  --test cifra-dos-dados verde  5.1 s    8 testes

--- com o defeito reposto, um de cada vez ---
  profiler-recorta         PROVADA      9.9 s  5/5 cairam
  cadeia-sem-teto          PROVADA      4.0 s  0/1 cairam
      o binario abortou, que e como esta guarda pega
  aad-fora-do-slot         REDUNDANTE   5.7 s  0/0 cairam
      confirmado: tirar so o AAD nao e sentido por teste nenhum
  ...
```

Cinco vereditos, e o terceiro é o motivo desta frente existir:

| veredito | o que quer dizer |
|---|---|
| **PROVADA** | todos os `caem` caíram e todos os `seguem` continuaram de pé |
| **REDUNDANTE** | a entrada declarou `espera: "nada muda"` e nada mudou mesmo — a guarda existe **duas vezes** no código, e tirar uma só não é sentida por teste nenhum. É resultado medido, e não falha |
| **NAO PEGOU** | um `caem` continuou passando — **é um teste que passa por engano**, e a casa considera isso pior que teste que falta |
| **ESTRAGOU** | um `seguem` caiu junto: a troca quebrou mais do que o defeito de origem quebrava, então ela não prova a guarda |
| **QUEBRADA** | o trecho não está mais no arquivo, aparece duas vezes, ou o código trocado nem compila — a entrada envelheceu |

Sai `0` quando todas ficaram provadas ou redundantes, `1` quando alguma não
ficou.

## As duas metades da prova real

O executor confere as **duas**, e nesta ordem:

1. **passa com o conserto** — a árvore limpa roda primeiro, inteira. Se ela não
   estiver verde, nada aqui prova nada e o executor para. Sem essa conferência,
   um teste já vermelho apareceria como guarda provada: o defeito reposto não
   teria feito diferença nenhuma e o teste cairia do mesmo jeito. **Foi ela que
   pegou o defeito do próprio executor** — ver abaixo.
2. **falha com o defeito** — a lista `caem`, teste a teste.

E a lista `seguem`, que é a terceira metade que ninguém pede: os testes que têm
de **continuar passando**. Sem ela, uma troca que quebrasse o arquivo inteiro
pareceria uma guarda excelente.

## Os três cuidados, e o que cada um custou

**Nunca na árvore de verdade.** O executor copia `crates/`, `exemplos/`,
`Cargo.toml` e `Cargo.lock` (5 MB) para `~/.cache/phx-guardas` e mexe só lá.
Cada troca é desfeita num `finally`, e há uma rede no `atexit`: um Ctrl-C no
meio não deixa defeito plantado em lugar nenhum.

E esse caminho é **compartilhado**: duas árvores de trabalho na mesma máquina
disputam a mesma cópia, e o estrago engana — três guardas saíram `QUEBRADA` com
«o código com o defeito reposto não compila», citando campos de uma `struct`
que não existe nesta árvore, porque a cópia era da árvore vizinha. Não é
entrada envelhecida, é cópia trocada.

**Hoje o executor tranca a cópia, e a segunda rodada espera a primeira.** A
regra era passar `--arvore` com um nome próprio, e ela dependia de alguém
lembrar — a rodada das transações não lembrou, e pagou os três vereditos de
mentira de uma vez, todos com cara de código que mudou:

| o que apareceu | o que era |
|---|---|
| `trava-sem-guarda-de-reentrancia` **QUEBRADA**, «o trecho não está mais em `servidor.rs`» | o trecho estava lá, uma vez só: a outra rodada tinha o defeito **dela** plantado no arquivo |
| `recuperar-sem-reindexar` **NÃO PEGOU**, com um teste de trava reprovando | o binário rodou com o defeito da outra rodada dentro |
| `sujas-com-a-trava` **QUEBRADA** por prazo, 420 s | o `--limpar` da outra rodada apagou o `target/` embaixo, e a compilação recomeçou do zero |

A tranca é um `flock` num arquivo ao lado do diretório (`~/.cache/phx-guardas.tranca`,
**fora** dele, porque o `--limpar` apaga o diretório inteiro). O núcleo a solta
sozinho quando o processo morre — inclusive num `SIGKILL`, que é o único jeito
de o `atexit` não rodar —, então tranca pendurada por rodada morta é
impossível. Quem espera vê a linha `esperando a vez` e quanto esperou.
`--arvore` continua valendo para quem prefere não esperar.

**Só o binário de teste que a entrada nomeia.** Medido nesta máquina, com o
`target/` quente e uma recompilação por mutação: o binário nomeado custa
**8,1 s**, o `cargo test --workspace` custa **49,2 s**. Para as 19 entradas são
~2 min contra ~15 min — não é «horas», como eu tinha escrito antes de medir, é
uma ordem de grandeza. O que a escolha compra é caber **dentro** da bateria
única (14m35s inteira) em vez de dobrá-la.

**Prazo em toda rodada.** Defeito que **pendura** em vez de falhar travaria a
bateria — e o `sujas-com-a-trava` é exatamente esse: um `Mutex` não reentrante
pedido duas vezes pela mesma thread. O teste dele já tem prazo próprio de 30 s;
o executor tem o dele por cima, e **mais largo**, senão mataria a rodada antes
de o teste conseguir reprovar. Medido: essa guarda leva 35,3 s, e as outras 18
levam de 1,4 a 13,2 s.

A `trava-atras-da-rede` é a segunda do mesmo naipe, e por outro caminho: o
defeito dela é o laço da réplica segurando a trava de dados durante uma leitura
de rede, e com ele reposto a sonda do teste **pendura** por 30 s em vez de
falhar. O teste tem prazo próprio de 8 s em cada sonda, o executor tem o dele
por cima (120 s), e a mensagem de reprovação já traz o diagnóstico: *«`varrer`
sem resposta em 8 s; o `ping`, que não precisa da trava, respondeu em 570 µs»*.

## Três coisas que só apareceram rodando

**A cópia por `copytree` reintroduziu a regra do binário velho, dentro da
ferramenta que existe para pegá-la.** `copytree` copia com `copy2`, que
**preserva a data**. A rodada anterior compilava o `target/` da cópia a partir
do fonte mutado; a seguinte devolvia o fonte limpo com a data velha; e o cargo,
que decide por data, achava o artefato mais novo que o fonte e não recompilava
nada — a «árvore limpa» rodava o binário **com o defeito ainda dentro**. Quem
pegou foi justamente a conferência da árvore limpa. Hoje a cópia é por
**conteúdo**, com a data de agora no que mudou, e os arquivos que o catálogo
sabe mutar levam `utime` a cada invocação.

**A cópia não pode morar no `/tmp`.** `restaurar.rs` tem um teste que exige que
o palco da restauração **não** caia em `std::env::temp_dir()`, e ele mede isso
contra o diretório de trabalho. Com a cópia em `/tmp/…`, o próprio diretório de
trabalho é temporário e o teste reprova sem haver defeito nenhum. Ler o teste
não mostrava isso.

**`crates/` sozinho nem compila.** O `lib.rs` do servidor faz
`include_str!("../../../exemplos/Config_exemplo_01.json")`. A primeira cópia
levou só `crates/` e o compilador disse exatamente qual arquivo faltava.

## Acrescentar uma guarda

1. escreva a entrada no `catalogo.py`, com o `trecho` copiado **do fonte** — ele
   tem de aparecer uma vez só, porque trocar a errada provaria outra coisa, e
   tem de casar byte a byte: um `\n` que existe de verdade dentro do fonte Rust
   pede uma *raw string* no catálogo, e o executor recusa a entrada quando não
   casa (foi o que aconteceu com o `evento-linha-sem-escape` na primeira
   tentativa);
2. rode `--so <id>`;
3. se der `NAO PEGOU`, **pare**: o achado é seu, e vale mais que a guarda. É um
   teste que passa por engano. Conserte o teste, ou registre com precisão por
   que ele não pega o que dizia pegar.

O `troca` tem de ser o **defeito de origem**, e não um sabotador qualquer que
derruba o teste por outro motivo. Três entradas mostram o que isso custa:

- **`profiler-recorta` e `profiler-recorta-largo` são o mesmo defeito com a mão
  mais e menos pesada**, e cada um derruba um conjunto diferente de testes. A
  entrada nasceu única, listando sete testes «porque o comentário do fonte diz
  que todos caem com um `find` e um corte». Medido: caem cinco, e depende de
  qual corte. «Este teste pega aquele defeito» é uma afirmação como outra
  qualquer.
- **`aad-fora-do-slot`, `nonce-sem-endereco` e `endereco-fora-da-amarracao` são
  três entradas para uma garantia só**, porque o endereço do slot cifrado está
  amarrado **duas vezes** (o dado associado e o nonce) e cada uma segura
  sozinha. As duas primeiras *afirmam* a redundância; a terceira prova a
  garantia. Tirar uma ponta só e chamar de defeito reposto teria dado
  `NAO PEGOU` num teste que está certo.
- **`regra-de-tabela-imposta` é a regra que a casa mais repete**, virada em
  asserção: *guarda nova entra pedida, não imposta*. Com o defeito reposto — sem
  regra de tabela, nega — caem **14 dos 540** testes do `--lib`, e o
  `supervisor_passa_por_cima` sobrevive, que é o `seguem` dela. A largura do
  estrago é o argumento: uma guarda imposta tira o direito de todo mundo que já
  funcionava, e quem trava isso é o teste do comportamento **velho**.

## Auditar uma redundância: o veredito e o motivo são duas coisas

```bash
python3 bancada/guardas/medir-redundancia.py
```

O executor audita o **veredito** das quatro entradas `espera: "nada muda"`, e
com rigor: para elas ele exige que **nenhum** teste do binário caia, e não só
os da lista. No dia em que algum cair, a afirmação morreu e o relatório avisa.

O que ele não audita é a `nota_da_redundancia` — a frase que diz **por que**
nada muda. E ela envelhece primeiro, porque uma redundância pode continuar
dando «nada muda» por um motivo diferente do declarado.

Foi o que se mediu em 03/09/2026 nas duas entradas da cifra. As notas
creditavam a redundância a `(rowid, volume, versao)`, os três valores que as
duas fechaduras carregam. Só que o teste que decide copia o slot 5 **inteiro**
por cima do slot 9 — cabeçalho junto, então a `versao` e o `tempero` viajam com
a cópia — e os dois slots moram no mesmo volume: **dos três, dois são iguais
dos dois lados.** Quem segura sozinho é o `rowid`.

| o que sai | o teste que decide |
|---|---|
| só o AAD (a guarda de hoje) | ok — 0 de 9 caíram |
| o AAD **e** só o `rowid` do nonce | **FAILED** |
| só o endereço do nonce (a guarda de hoje) | ok — 0 de 9 caíram |
| o endereço do nonce **e** só o `rowid` do AAD | **FAILED** |

Quem removesse `volume` de qualquer uma das fechaduras leria a nota antiga como
satisfeita e estaria certo por acaso. Quem removesse **o `rowid` de uma só**
também a leria como satisfeita — e estaria errado, porque é o único dos três
que trabalha. Por isso as notas de hoje nomeiam o `rowid`, e por isso este
medidor está versionado: a próxima sessão refaz a tabela em vez de acreditar
nela.

**A regra que sai daí:** quando uma nota de redundância citar mais de uma coisa
como causa, meça **qual delas sozinha sustenta**. A que não sustenta é
exatamente a que alguém vai remover confiando na nota.

E uma «falta» que foi registrada e **não existe** — a correção é de 03/09/2026,
e a história dela vale mais que o fato.

O que estava escrito aqui: *«no `julgar`, o ramo "nada muda" devolve antes de
olhar a lista `seguem` — nessas quatro entradas ela é decorativa… um `seguem`
renomeado não vira `QUEBRADA`»*. A **observação** sobre o `julgar` está certa; a
**conclusão** é falsa, e por dois caminhos independentes:

- **existência** já é conferida na conferência prévia, *antes* de repor o
  defeito: o executor casa `caem + seguem` contra a árvore limpa e devolve
  `QUEBRADA` ali (`provar-guardas.py`, o `faltando` do laço principal);
- **passagem** já cai no `cairam` do próprio ramo, que olha **todos** os
  vereditos e não só os `caem`.

O controle **não é decorativo**. Provado repondo o defeito: renomear
`cifrada_a_tabela_funciona_igual` no `seguem` do `aad-fora-do-slot` devolve
`QUEBRADA` — *«teste que o catálogo nomeia e o binário não tem»* —, e desfeito
volta a `REDUNDANTE`.

**Como o engano quase virou código:** escreveu-se um conserto de doze linhas
para o `julgar`, e ele *passou* na prova real — porque o `QUEBRADA` vinha da
conferência prévia e o conserto nunca executou. O que denunciou foi a
**mensagem não ser a do conserto**. O código foi removido: código morto que
parece guarda é pior que código nenhum, porque o próximo leitor acredita que
está protegido.

E a lição para quem sabota: nas três primeiras tentativas a sabotagem **não
sabotou** — um regex renomeou a chave `"seguem"` em vez do teste, e depois um
`assert` pegou que o nome aparece três vezes no catálogo e abortou antes de
gravar. Nas duas primeiras dava para reportar sucesso. *Teste que passa por
engano é pior que teste que falta* — inclusive quando o teste é a sabotagem.

## A caça ao teste que passa por engano — e o que ela NÃO achou

Fora do catálogo há **163 testes** que afirmam recusa (`expect_err`,
`assert!(… .is_err())`) e não têm guarda nenhuma. Nenhum deles foi provado nos
dois sentidos: eles passam, e ninguém sabe se passariam também com o defeito
reposto. Essa é a definição de «teste que passa por engano», e esta casa já
teve dois.

Em 03/09/2026 nove deles foram sondados — amostra escolhida pelas pétreas que
mais importam, não pela facilidade: a integridade referencial, a permissão por
tabela, a janela de conflito, o índice dos dois lados, a validação de nome, a
recusa de UUID torto e a injeção de comando no firewall. Para cada um, o
**portão de produção** foi neutralizado à mão e o binário rodou inteiro.

| defeito reposto | teste | caiu? |
|---|---|---|
| o portão do `ao_excluir` sai (`valores.rs`) | `ao_excluir_so_aceita_restringir` | cai |
| `verificar` nasce `false` (`valores.rs`) | `a_chave_declarada_nasce_conferida` | cai |
| `executar_derivado` pula `portoes_do_pedido` | `o_sql_nao_e_a_porta_dos_fundos_para_a_tabela_negada` | cai |
| `Uuid::de_texto` aceita qualquer texto | `uuid_torto_no_json_e_recusado` | cai |
| o firewall deixa de conferir o IP | `firewall_recusa_o_que_nao_e_endereco` | cai |
| `validar_nome` vira `Ok(())` | `nomes_perigosos_sao_recusados` | cai |
| idem | `nome_de_destino_hostil_e_recusado` | cai |
| versão divergente deixa de ser conflito | `gravar_com_versao_velha_recusa` | cai |
| o índice da filha deixa de ser exigido | `sem_indice_na_filha_recusa_dizendo_qual_falta` | cai |

**9 de 9 caíram. Zero achados.** Árvores limpas verdes antes de cada perna:
`phxsql-server --lib` com 677 testes, `phxsql-store --lib` com 145,
`--test conflito` com 10 e `--test cascata-ao-alterar` com 15.

É recusa medida, e vale como resultado: **nesta amostra não há teste que passa
por engano.** O que ela não autoriza é generalizar para os 154 restantes — o
que se mediu é que a prática desta casa (escrever o teste junto do portão que
ele defende) sobrevive à conferência nos casos onde a pétrea é mais forte, que
é justamente onde a atenção era maior. A amostra é enviesada **a favor** do
resultado, e dizer isso faz parte do resultado.

O roteiro da caça não ficou versionado de propósito: ele é uma lista de
defeitos de uma tarde, e cada entrada dele que valesse a pena guardar deveria
virar **entrada do catálogo** em vez de script à parte. Nenhuma virou, porque
todas as nove guardas já estavam provadas — acrescentá-las custaria nove
recompilações por bateria para travar o que já se sabe travado.
