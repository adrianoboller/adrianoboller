# Parecer do DBA: CHECK e coluna calculada contra a linha velha (O2 do pedido 245)

Papel C, 16/09/2026, **so leitura e provas de leitura**. Nada foi comitado, e o
conserto nao esta escrito aqui — este documento nomeia o **nao** e o porque.

**Veredito em uma linha:** os dois defeitos sao reais, a descricao do O2 acerta
o alvo e **erra por falta na causa** — medido, o estrago e maior do que o texto
diz, e alcanca a petrea da integridade. A saida e a **(a)**: `acrescentar_coluna`
julga as linhas que ja estao la e **recusa nomeando quantas violam**, dentro da
passada que ele ja faz. **E nao pede PSCH novo** — o formato v9 ja carrega tudo.

---

## 1. Como isto foi medido (a disciplina do binario velho)

| prova | binario | conferencia |
|---|---|---|
| semantica, pelo soquete | `target/debug/phxsqld` (16/09 11:41) | `find crates -name '*.rs' -newer target/debug/phxsqld` devolve **vazio** — o binario e mais novo que todo fonte |
| custo do ALTER e da leitura | `target/release/examples/custo-do-alter` (16/09 11:14) e `onde-doi-na-leitura` | nenhum `.rs` de `phxsql-store` mais novo, fora de outro `example` |
| os quatro motores | PostgreSQL **16.13**, MySQL **8.0.46**, MariaDB **10.11.14**, SQLite **3.45.1** | os tres primeiros subidos aqui, em datadir temporario, derrubados e apagados no fim |

O `target/release/phxsqld` **nao** serviu: ha `.rs` mais novo que ele. As sondas
sao quatro roteiros de protocolo, no scratchpad da sessao, e cada uma sobe e mata
so o proprio PID. Os dois `phxsqld` de release que rodam nesta maquina sao de
outras frentes e **nao foram tocados**.

---

## 2. Defeito 1, o CHECK: onde ele esta, por caminho e linha

### 2.1 A porta que nao existe

`Table::acrescentar_coluna` (`crates/phxsql-store/src/table.rs:789`) tem **seis**
recusas antes de tocar em arquivo: tabela em modo ledger (:800), nome de coluna
do motor (:809), `Sequence` (:815), `Str(0)` (:822), padrao em coluna externa
(:831) e obrigatoria-sem-padrao-com-linha (:838). **Nenhuma delas olha
`coluna.check` nem `coluna.calculada`.**

A coluna entra no esquema por `Schema::com_coluna`
(`crates/phxsql-core/src/schema.rs:898`), que confere posicao e nome repetido e
**nao conhece dado**. E `RegFile::acrescentar_coluna`
(`crates/phxsql-store/src/reg.rs:1208`) recebe `padrao: &[u8]` — **um** conteudo
de bytes para **todos** os slots — e o aplica em `refazer_payload` (:1269) sem
decodificar linha nenhuma.

Ou seja: a expressao chega inteira (o `coluna_de_json` a le em
`crates/phxsql-server/src/valores.rs:404-407`), e grava-se no `PSCH` v9
(`schema.rs:1335-1344`), mas **ninguem pergunta ao dado**.

Do outro lado, `Table::aplicar_regras` (`table.rs:2756`) julga o CHECK em **toda**
gravacao — no `inserir` (:2995) e no `atualizar` (:3290). A assimetria e o
defeito inteiro: a regra nasce sem conferir para tras e passa a valer para
sempre dali em diante.

### 2.2 «Aceito calado» — confirmado, com a resposta na mao

```
acrescentar_coluna limite Int8, default 0, check 'limite > 0'
    {"ok": true}   slots_reescritos: 2, registros: 2, indices_refeitos: false
ler rowid 1
    {"id": 1, "limite": 0}        <-- o valor que o CHECK declara impossivel, gravado
```

Nao ha campo de aviso na resposta. O `ok` e liso.

### 2.3 «E dai todo `atualizar` recusa» — verdade por tres caminhos, e **falso por um**, que e o pior

| caminho | o que acontece | medido |
|---|---|---|
| `atualizar` com a **linha inteira** (o que a tela manda) | **recusa** | `restricao CHECK da coluna limite recusou a linha: limite > 0` |
| `UPDATE ... SET nome=...` pelo **SQL** | **recusa**, mesmo sem citar a coluna | idem, na linha 3 |
| `atualizar` **parcial** pelo protocolo (sem citar a coluna) | **passa** — e **apaga o valor**: a leitura seguinte devolve `limite: null` | `{"ok": true}` e depois `{"limite": null}` |
| CHECK sobre coluna **velha** (`id > 100`) | **recusa por todos os caminhos** | `restricao CHECK da coluna obs recusou a linha: id > 100` |

O terceiro caso e o que o texto do O2 nao previa, e e o mais grave dos quatro:
ele **nao** da erro — ele apaga em silencio o valor da coluna nova para fazer a
linha passar. E o O3 do mesmo pedido 245 («upsert parcial poe NULL nas
ausentes») encontrando o O2: **a guarda nova, combinada com o contrato velho,
destroi dado em vez de recusar**. Quem le so o O2 conserta o erro e deixa o
apagamento de pe.

### 2.4 O que o texto nao diz, e e o pior: a cascata quebra no meio e **deixa orfa**

Cenario medido, com controle:

```
mae(id) <- filha(mid), fk ao_excluir=restringir ao_alterar=cascata, imposta
acrescentar_coluna na FILHA: nota Int8 default 0 check 'nota > 0'   -> {"ok": true}
atualizar mae.id 1 -> 2
  {"ok": false, "erro": "integridade referencial: mae: a linha mae mudou, e a
   alteracao NAO chegou a linha 1 de filha pela chave \"fk_mae\"
   (restricao CHECK da coluna nota recusou a linha: nota > 0).
   Nao ha transacao aqui: a mae ja esta gravada, e essa filha ficou para tras"
ler mae    -> {"id": 2}
ler filha  -> {"id": 10, "mid": 1}          <-- aponta para uma mae que nao existe mais
inserir filha nova com mid=1
  {"ok": false, "erro": "fk_mae: nao existe mae(id) com esse valor"}
```

A ultima linha e a prova: o motor **recusa criar** a linha que ele acabou de
deixar existindo. O controle, a mesma montagem **sem** o CHECK, cascateia certo
— `filha.mid` vira 2.

**Isto e a petrea, nao um incomodo de usabilidade.** «So existe filho se o pai
existir primeiro» e «nunca se mata o pai que tem filhos» sao a mesma lei vista
pelo tempo; aqui a mae **mudou de identidade** e a filha ficou orfa, e quem
causou foi um `acrescentar_coluna` aceito sem uma palavra, numa **outra** tabela,
possivelmente dias antes. O erro aparece longe da causa — que e exatamente o que
o O2 dizia, so que o dano nao e «nao consigo salvar», e sim **orfandade
gravada**.

### 2.5 A linha marcada tambem recebe o valor, e o `restaurar` nao julga

```
excluir suave rowid 2            -> ok
acrescentar_coluna ... check     -> ok (a tabela tinha 1 linha viva)
restaurar rowid 2                -> ok            <-- o CHECK nao e julgado aqui
ler rowid 2                      -> {"limite": 0, "softdeleted": false}
atualizar rowid 2 (linha inteira)-> recusa
```

A lixeira guarda linha que volta **violando**. Quem contar so as vivas na hora do
ALTER entrega uma bomba com prazo: ela estoura no dia do `restaurar`. E a lei da
casa ja decidiu este caso no lado da FK — «pai logicamente morto deixa filha
apontando para linha que a tela nao mostra mais» —; aqui vale igual.

### 2.6 Duas coisas que o motor **ja** faz certo

`acrescentar_coluna` recusa na **declaracao** o CHECK com coluna inexistente
(«o check de z1 usa a coluna "naoexiste", que a tabela nao tem») e o CHECK com
sintaxe quebrada. E **nao entra em transacao** («acrescentar_coluna nao entra em
transacao … termine com COMMIT ou ROLLBACK e repita»), entao a recusa nova nao
tem semantica de transacao para resolver: ela e solitaria e limpa.

---

## 3. Defeito 2, a calculada: confirmado, e a agregacao e o pior

```
k(a,t) com duas linhas;  acrescentar_coluna b Int8 calculada 'a * 2'   -> ok
SELECT a,b   ->  [{a:5, b:null}, {a:7, b:null}]
UPDATE k SET t='z' WHERE a=5           (nao toca em b)
SELECT a,b   ->  [{a:5, b:10},   {a:7, b:null}]      <-- mudou sozinha
SELECT SUM(b), COUNT(b), COUNT(*)  ->  s=10, n=1, t=2
```

Tres estragos, e o terceiro e o que ninguem ve:

1. a linha velha le **nulo** numa coluna cuja definicao diz que o valor e
   derivavel — a coluna mente sobre si mesma;
2. o primeiro `UPDATE` que tocar a linha, **por qualquer motivo**, muda `b`
   sozinho. Ninguem pediu, nada avisa, e a versao da linha sobe;
3. `SUM(b)` devolve **10** sobre uma tabela cuja soma e **24**, com `COUNT(b)=1`
   de `COUNT(*)=2`. Relatorio de coluna calculada **conta metade da tabela e nao
   diz que contou**. Filtrar por ela nem chega a ser opcao: `WHERE b = 14` e
   recusado por falta de indice.

E um quarto, se quem alterou mandar `default` junto:

```
acrescentar_coluna c calculada 'a * 3' com default 999  -> ok
SELECT a,c  ->  c = 999 nas duas velhas
```

**999 e um valor que a expressao nunca produz.** Um `default` numa coluna
calculada e um numero que a proxima gravacao apaga — «configuracao que nao e
lida mente», na forma de dado.

---

## 4. O que os quatro motores fazem — medido hoje, nao lembrado

### 4.1 `ALTER TABLE … ADD COLUMN … CHECK` contra linha que viola

| motor | peso | comportamento medido |
|---|---:|---|
| PostgreSQL 16.13 | 4 | **RECUSA**: `check constraint "c1_limite_check" of relation "c1" is violated by some row` |
| MariaDB 10.11.14 | 3 | **ACEITA** — e o `UPDATE` seguinte falha: `ERROR 4025 CONSTRAINT 'c1.limite' failed` |
| MySQL 8.0.46 | 2 | **RECUSA**: `ERROR 3819 Check constraint 'c1_chk_1' is violated` |
| SQLite 3.45.1 | 1 | **RECUSA**: `CHECK constraint failed` |

**Nao ha convergencia dos tres maduros** — o MariaDB diverge —, entao **nao ha
aceite automatico**: vale a regua ponderada. **Recusar = 4+2+1 = 7; aceitar = 3.**
Recusar ganha 7 a 3.

E a divergencia e **diagnostica**, o que raramente acontece: o unico motor que
faz como nos hoje e o MariaDB, e o comportamento dele **reproduz exatamente o
defeito relatado no O2** — aceita calado e depois recusa o `UPDATE` de rotina.
Nao estamos escolhendo entre dois desenhos defensaveis; estamos do lado que o
proprio voto minoritario demonstra ser o estrago. (O MariaDB, alias, discorda de
si mesmo: `ALTER TABLE … ADD CONSTRAINT CHECK` sobre tabela que viola ele
**recusa** — `ERROR 4025` — e so a porta do `ADD COLUMN` fica aberta. E o mesmo
buraco nosso, no mesmo lugar.)

Tres notas de fronteira, todas medidas:

- **CHECK nulo passa em todos os quatro.** `ADD COLUMN saldo int CHECK (saldo>0)`
  sem default entra em PG, MySQL, MariaDB e SQLite: a coluna nasce NULL e `NULL`
  nao e `FALSE`. Nisso **ja convergimos** — nosso `aplicar_regras` (:2781) usa
  `== Some(false)`, e esta certo.
- **MySQL recusa na declaracao o CHECK de coluna que fala de outra coluna**
  (`ERROR 3813 … references other column`). Nos aceitamos, PG e MariaDB tambem.
  Nao ha trio: fica como esta.
- **`NOT VALID` do PG nao resolve o sintoma do O2.** Medido: com a restricao
  `NOT VALID`, o `UPDATE` que nem toca na coluna **continua recusado**
  (`new row for relation "c2" violates check constraint "v_pos"`). O `NOT VALID`
  do PG so pula a varredura; ele nao liberta a linha velha. Ja o `NOT ENFORCED`
  do MySQL liberta — porque desliga a restricao inteira.

### 4.2 Coluna calculada acrescentada a tabela com linha

| motor | o que faz | o que a linha velha le |
|---|---|---|
| PostgreSQL 16.13 | `GENERATED … STORED`: **preenche** no ALTER | 10 e 14; `SUM=24`, `COUNT(b)=2/2` |
| MariaDB 10.11.14 | `STORED`: **preenche** | 10 e 14; `SUM=24` |
| MySQL 8.0.46 | `STORED`: **preenche** | 10 e 14; `SUM=24` |
| SQLite 3.45.1 | **recusa** `STORED` (`cannot add a STORED column`); aceita `VIRTUAL` | 10 e 14 pela leitura; `SUM=24` |

**Convergencia de quatro em quatro no COMPORTAMENTO**: depois do ALTER, a linha
velha **nunca** le nulo numa coluna calculada computavel. Os tres maduros
convergem tambem no **meio** (preencher), e o quarto so muda onde paga.
Nada nosso se opoe — entra **sem pergunta**, pela lei de 11/09.

---

## 5. A decisao: (a), recusar varrendo antes

**(a) e a saida.** Motivo, na ordem em que pesa:

1. **E a unica que preserva a petrea da integridade.** (b) e (c) deixam a orfa da
   §2.4 existir. Uma tabela cuja FK cascateia nao pode ter, numa **coluna nova**,
   uma regra que faz a cascata parar no meio — e esse e o unico dos tres modos de
   falha que grava dado errado em vez de recusar trabalho.
2. **E onde esta casa ja decidiu que a recusa mora** (§6).
3. **Ganha o voto 7 a 3**, e o voto minoritario e o nosso proprio defeito.
4. **Custa pouco, e so para quem declara CHECK** (§8).

Exigencias da recusa, que sao decisao de DBA e nao detalhe de implementacao:

- **conta todas as linhas que violam, e nomeia a primeira** — nao para na
  primeira. Parar barato transforma uma tabela com K violadoras em K tentativas
  de ALTER, cada uma com a passada inteira; contar tudo custa **uma** passada e
  entrega o tamanho do conserto. Quem modela precisa saber se sao 3 linhas ou
  300 mil antes de escolher entre consertar o dado e mudar a regra;
- **separa vivas de marcadas**, e conta as duas (§2.5). Marcada que volta
  violando e a mesma orfa-que-ninguem-ve;
- **nomeia tabela, coluna, o texto da restricao e o rowid da primeira** — e a
  mesma familia da recusa que «nomeia a tabela que falta» em vez de vazar
  «nenhum volume de clientes.reg»;
- **nada se troca no disco antes de a conferencia passar.** O ponto de
  compromisso do `acrescentar_coluna` e o `rename` do volume 1
  (`reg.rs:1385`, FASE B); a conferencia tem de terminar **antes** dele. A FASE A
  (`reg.rs:1345`) ja e reversivel por desenho: `*.novo` orfao e lixo que a
  abertura reconhece como lixo.

### Por que **nao** (c), exigir DEFAULT quando ha CHECK

**Morre medida.** O caso que quebra nas quatro provas e justamente
`DEFAULT 0 CHECK (limite > 0)` — o default **existe** e **e ele** que viola. (c)
nao cobre o caso que motivou o pedido. E nao cobre o CHECK sobre coluna velha
(`id > 100`), onde a coluna nova nem participa. Alem disso, nenhum dos quatro
motores exige default para aceitar CHECK, e mudar o contrato de quem ja chama
`acrescentar_coluna` cobraria um preco real por uma protecao que nao protege.
**Recusada, com o numero: cobre 0 dos 2 modos de falha medidos.**

### Por que **nao** (b) como padrao — e o que (b) seria, se alguem pedir

(b) e «marcar a coluna como nao conferida para tras». Duas objecoes, e a primeira
e medida:

- **na forma do PG (`NOT VALID`), ela nao conserta nada**: a linha velha continua
  sem poder ser atualizada (§4.1). O ganho do `NOT VALID` e pular a varredura num
  banco de producao, e nao libertar a linha;
- **na forma do MySQL (`NOT ENFORCED`), ela conserta desligando a regra** — a
  coluna passa a ter um CHECK que nao e CHECK. E ai (b) nao e uma terceira saida:
  e «nao declare o CHECK», escrito com mais bytes.

E a objecao de lei: (b) como **padrao** repete, letra por letra, o erro que a
petrea «**chave declarada nasce conferida**» ja corrigiu nesta casa — regra que
precisa ser *lembrada* de conferir transforma o esquecimento em padrao. O irmao
exato desta lei diz que **CHECK declarado nasce conferido**.

(b) so tem lugar como **saida escrita**, no molde do `"verificar": false` da FK:
quem **quer** declarar sem conferir manda, e fica gravado que quis. Isso pediria
formato novo (§7). Minha recomendacao: **nao agora** — byte que so existe para
desligar uma guarda que ainda nao entrou e campo sem leitor, e campo sem leitor
mente. Quando alguem pedir, o desenho esta em §7 e continua barato.

### Qual garantia cada saida quebra ou preserva

| saida | preserva | quebra |
|---|---|---|
| **(a)** recusar varrendo | integridade referencial (a cascata volta a funcionar); «recusa na declaracao»; ordem de digitacao (nao mexe em slot antes de decidir); CHECK declarado nasce conferido | nada — cobra uma leitura da tabela, so quando ha CHECK |
| **(b)** marcar nao conferida | o custo do ALTER | deixa gravado o dado que o CHECK diz ser impossivel; mantem a orfa da §2.4 (forma PG); precisa de PSCH v10 |
| **(c)** exigir DEFAULT | nada que as outras nao preservem | muda o contrato de quem ja chama, **e nao cobre nenhum dos dois modos de falha medidos** |

---

## 6. A recusa acontece na declaracao ou na gravacao? A petrea alcanca — e diz que hoje esta invertida

Alcanca, e este e um caso de livro.

A petrea diz: «a recusa acontece na **declaracao**, nao na gravacao, e isso e
decisao: uma tabela nasce uma vez e grava um milhao de vezes». A economia e
identica aqui: **uma coluna com CHECK nasce uma vez e e gravada um milhao de
vezes**. Hoje o motor faz o contrario do que a lei manda — aceita a declaracao
sem uma palavra e cobra a recusa em **cada** `atualizar`, para sempre, longe da
causa. Nao e um caso que a petrea nao previu: e a inversao que ela nomeia.

Ha uma dobra que vale dizer em voz alta, porque ela confunde: **o
`acrescentar_coluna` e as duas coisas ao mesmo tempo** — declara uma coluna *e*
reescreve todo slot. «Recusar na declaracao» aqui quer dizer, literalmente,
**recusar dentro do ALTER, antes do ponto de compromisso**, e nao em algum
momento anterior que nao existe. A FASE A / FASE B do `reg.rs` ja da o lugar
exato.

E a segunda metade da petrea nao muda: «**e ela e imposta na gravacao**». O
`aplicar_regras` continua julgando todo `inserir` e todo `atualizar`. O que
entra e a conferencia que falta no nascimento, nao uma troca de lugar.

---

## 7. Formato em disco: **nao muda nada** — e o que mudaria, se um dia

**Esta e a resposta que o pedido mandava dizer alto, e ela e a boa: o conserto
do O2 nao pede PSCH novo.**

- o `PSCH` v9 ja grava `padrao`, `check` e `calculada` por coluna, em texto, no
  fim do bloco (`docs/FORMATO.md` §«As expressoes de esquema, v9»;
  `schema.rs:1335-1344`). A regra ja viaja no disco e ja volta do disco;
- o valor preenchido da calculada **ja tem lugar**: a coluna ocupa a largura dela
  no payload desde o instante em que entra — hoje esse lugar recebe nulo. Gravar
  o valor calculado usa o mesmo espaco;
- a conferencia do CHECK **nao grava nada**;
- banco antigo continua legivel, porque nada no bloco muda. Tabela que ja tem um
  CHECK aceito calado **continua com ele**, e continua com as linhas que violam:
  a recusa nova vale para o que nascer daqui em diante. E o mesmo alcance do byte
  `verificar` da v7 — a guarda nova protege o dado que ja esta la de **piorar**,
  nao conserta o passado.

O que **pediria** formato novo e so a saida (b): um byte por coluna, «esta regra
nasceu sem conferir para tras», no molde do `verificar` da FK. Se um dia entrar,
entra como a v4, a v6 e a v8 entraram — **bloco proprio, no fim**, para que um
leitor de v9 pare antes dele. Com uma diferenca que a v9 ja decidiu e que vale
para ele: bloco de expressao truncado e **erro**, nao «fica sem». Pela mesma
razao, o byte tem de dizer «conferida» quando ausente, nunca o contrario.

**E fica dito, porque e a minha petrea:** se essa marca for entrar algum dia, o
dia mais barato e **agora**, enquanto nao ha dado em producao. Depois vira
migracao. Minha recomendacao continua sendo nao entrar — mas a recomendacao e
sobre a utilidade da marca, nao sobre o momento.

---

## 8. O custo, medido — e por que ele e menor do que o pedido supunha

Medido hoje, release, nesta maquina:

| medida | numero |
|---|---|
| `acrescentar_coluna`, 200.000 linhas | **0,865 us/linha** (273,6 MiB/s; 22,5 -> 24,8 MiB) |
| `acrescentar_coluna`, 100.000 linhas | 0,956 us/linha — o custo por linha e constante |
| projecao de 10 milhoes de linhas (e projecao) | **8,6 s** |
| ler uma linha em varredura sequencial (pagina de 200) | **0,635 us/linha** |
| escolher os rowids da pagina | 0,570 us/linha |
| ler uma linha por rowid, aleatorio | 1,81 us/linha |

O pedido enxergava «(a) custa uma varredura da tabela inteira numa operacao de
modelagem». **A varredura ja existe.** `acrescentar_coluna` ja visita todo slot,
na ordem, e ja abre cada payload (`reg.rs:1269`, `refazer_payload`, sobre o
`claro` devolvido por `abrir_slot_com`). O que falta nao e a passada: e
**decodificar a linha em valores e avaliar a expressao**, dentro do laco que ja
esta la.

O acrescimo, pelo limite de cima medido, e **<= 0,635 us/linha** mais a avaliacao
da expressao: o ALTER com CHECK sai de 0,865 para a ordem de **1,5 us/linha** —
**menos que 2x**, e 10 milhoes de linhas passam de 8,6 s para a ordem de 17 s
(projecao, nao medida). Para uma operacao que ja reescreve 24 MiB por 200 mil
linhas e que se faz uma vez na vida da coluna, isto e barato.

E o portao que decide o custo vem **antes** do trabalho, como o Profiler ensinou:
`coluna.check.is_some() || coluna.calculada.is_some()`. Quem acrescenta coluna
sem regra nenhuma continua pagando **exatamente** os 0,865 us/linha de hoje.

Duas ordens de trabalho, e as duas satisfazem a exigencia «nada se troca antes de
conferir». Nao escolho entre elas — e desenho, e e do papel B:

- **julgar dentro da passada da FASE A**, abortando antes da FASE B: zero
  leitura a mais no caminho de sucesso; na recusa, perde-se a FASE A escrita;
- **uma pre-passada de leitura**, antes de escrever byte nenhum: recusa muito
  mais barata (0,635 us/linha e nenhuma escrita), sucesso com uma leitura a mais
  do arquivo.

O que eu **exijo** e so o invariante: a decisao acontece antes do `rename` do
volume 1.

---

## 9. A calculada: preencher na hora, na mesma passada

O comportamento entra por convergencia de quatro em quatro (§4.2): **a linha
velha nao le nulo**. O meio e nosso, e a escolha e **preencher no ALTER**, pelas
nossas restricoes e nao pelas deles:

- **a nossa calculada e STORED por definicao.** O `PSCH` v9 ja lhe da largura no
  slot, e todo `inserir`/`atualizar` ja grava o valor (`table.rs:2772-2776`).
  Calcular na leitura criaria **duas verdades no mesmo arquivo**: linhas novas
  com o valor gravado, linhas velhas computadas na hora — e quem le o `.reg` por
  fora (backup, replica, imagem do diario, `verificar`) veria as duas. E o
  desenho que o SQLite so consegue sustentar porque la a coluna **inteira** e
  virtual, nunca meio-meio;
- **o laco quente e ler e inserir; alterar coluna e raro.** A mesma conta que
  poe a busca reversa da FK no `excluir` e nao num catalogo reverso poe este
  custo no ALTER e nao em toda leitura;
- **e nao custa formato**: o lugar ja esta la, recebendo nulo hoje.

Duas recusas que vem junto, e sao decisao de DBA:

1. **`default` numa coluna calculada passa a ser recusado na declaracao.** Hoje
   ele grava 999 numa coluna cuja expressao e `a*3` (§3) — um valor que a
   proxima gravacao apaga. `docs/FORMATO.md` ja diz que na calculada «o valor que
   vier e ignorado»; o `default` do ALTER e exatamente um valor que vem, so que
   explicito, e explicito se recusa em vez de se ignorar;
2. **a calculada preenchida nao muda a ordem de digitacao.** O ALTER continua
   saindo slot a slot, na mesma ordem, com o rowid preservado e o `.ndx`
   intocado. Nada aqui reaproveita slot, e nada aqui renumera.

---

## 10. Quatro armadilhas nomeadas para quem for implementar

1. **O preenchimento da calculada NAO pode passar por `julga_integridade()`.**
   `aplicar_regras` (`table.rs:2757`) desliga na replica de proposito — a imagem
   que chega ja veio julgada. Mas `acrescentar_coluna` **e local e nao se
   replica** (`docs/FORMATO.md`): a replica roda o **proprio** ALTER. Se o
   preenchimento herdar esse portao, a origem preenche, a replica deixa nulo, e
   os dois lados divergem **em silencio** numa tabela que o backup e o
   `verificar` comparam. Este e o «caminho irmao» classico desta casa, e ele esta
   marcado antes de existir.
2. **O que torna isso seguro e a gramatica ser determinista.** As funcoes de
   expressao sao **oito** — UPPER, LOWER, TRIM, LENGTH, ROUND, ABS, COALESCE,
   CONCAT (`crates/phxsql-core/src/expressao.rs:201-229`) — e **nenhuma** e
   `NOW()` ou `RANDOM()`. E por isso que origem e replica, calculando cada uma por
   si, chegam ao mesmo byte. **No dia em que alguem acrescentar uma funcao nao
   determinista a essa lista, este desenho quebra** — e a divida e da gramatica,
   nao do ALTER.
3. **Ordem entre os dois lados.** O ALTER com CHECK deve rodar na replica **depois
   que ela converge**; uma replica atrasada pode nao ter ainda a linha que viola
   e aceitar o que a origem recusou, deixando os esquemas diferentes. A
   `bancada/alter/provar.py` ja estabelece «a replica converge antes de alterar»
   — vira pre-condicao escrita, nao costume.
4. **Hoje so existe uma porta; amanha podem existir duas.** Nao ha
   `alterar_coluna` nem `criar_indice` no despachante do servidor — conferido —,
   entao `acrescentar_coluna` e o **unico** caminho que instala uma regra sobre
   tabela que ja tem linha. No dia em que nascer uma operacao que troque o
   esquema de uma tabela com dado, a conferencia tem de nascer com ela. Guarda
   que cobre uma porta so nao e guarda menor hoje: e guarda que nao cobre a porta
   de amanha.

---

## 11. O que eu nao decido, e fica para o dono / para o papel B

- **A redacao exata da recusa** e o formato do relatorio (quantas vivas, quantas
  marcadas, qual o primeiro rowid) — e mensagem de tela, e passa pela fabrica de
  idiomas.
- **Julgar dentro da FASE A ou em pre-passada** (§8): as duas honram o
  invariante; a escolha e medida de implementacao.
- **Se a saida (b) entra** como `"conferir": false` explicito. Eu recomendo
  **nao**; se o dono quiser, o desenho e o §7 e o momento barato e agora.
- **O O3** (`atualizar` parcial pondo NULL) — ele **agrava** o O2 (§2.3), mas e
  pedido proprio, e mexer nele muda contrato de cliente antigo. Nao o toque a
  reboque deste.
