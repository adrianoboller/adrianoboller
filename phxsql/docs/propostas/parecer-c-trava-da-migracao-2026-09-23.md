# Parecer do papel C — a trava global da `migrar_esquema`

**Data:** 23/09/2026, 16:52–17:05 UTC · **Papel:** C (DBA sênior) · **Só leitura**

**Veredito: (A)**, e é a única das três que é verdadeira. Mas **não é uma
mudança de linha**, e a versão ingênua dela *compila, deixa a catraca verde e
perde escrita confirmada*. O teto **não sobe** e a catraca **fica vermelha** até
o conserto real entrar — vermelho é o estado honesto.

---

## 0. Antes de tudo: dois fatos que corrigem o enunciado

**0.1 — O `servidor.rs` mudou debaixo de mim, e eu avisei que avisaria.**
Entre a minha primeira leitura (16:52) e a segunda (16:53:46) o arquivo passou
de 50.366 para 50.519 linhas, `+360/−81` contra o `HEAD`. É a frente do 418.
Por isso **todo número deste parecer foi remedido contra o `HEAD` fixado**
(`97ddc71`), extraído para fora da árvore. Medido nos dois:

| árvore | seções críticas | alcançam durabilidade |
|---|---:|---:|
| `55ab903^` (= `bccd3dd`) | 86 | **24** |
| `HEAD` `97ddc71` (fixado) | 88 | **25** |
| árvore suja, 16:54 | 88 | **25** |

A conclusão **não depende** da frente viva: 25 nos dois.

**0.2 — `HEAD` já não é `55ab903`.** São `a671f2d` e `97ddc71` por cima. E o
`mapa-da-trava.py` **sem argumento sai com código 0 e não imprime `SUBIU`** — a
catraca só reprova com `--catraca` (rc=1). Quem rodar o comando do enunciado sem
a bandeira vê uma corrida limpa.

**A 25ª está confirmada por medição minha**, e a linha real da `fn` é
**`servidor.rs:16226`** no `HEAD` (o `16234` do enunciado é a linha do texto
limpo do mapeador, que cai dentro de um `push`):

```
16226  op_migrar_esquema   migrar_para_psch_v10 -> acrescentar_coluna(2/2) -> escrever_volume_alargado
```

**A família é de quatro, não de dois** — e isto muda o alcance de qualquer
conserto (*«conserto entra no caminho que o motivou, e o caminho IRMÃO fica»*):

| linha (`HEAD`) | seção | reescreve o volume inteiro? |
|---:|---|---|
| 16052 | `op_declarar_fk` | **condicional** — só se o bloco de esquema não couber antes do `data_offset` (`reg.rs:1234`) |
| 16112 | `op_acrescentar_coluna` | **sempre** |
| **16226** | **`op_migrar_esquema`** | **sempre, ×2 passadas** |
| 16407 | `op_excluir_fk` | **condicional**, idem |

O mapeador é **estático** e não vê a condicional — ele conta o caminho
alcançável. Guardar isso para a §3.

---

## 1. Quanto tempo a trava fica presa — MEDIDO hoje, nesta máquina

Não citei o `0,553 µs/linha` que está no `table.rs:1063`: **remedi**.

**Aferição de obsolescência primeiro** (*«medidor com binário velho mede o
passado»*): `target/release/examples/custo-do-alter` é de hoje 09:15, anterior ao
`55ab903` (16:40). Conferi os *hunks*: o `55ab903` tocou `table.rs` em
`@@ 54, 1018, 1079, 2697, 3819, 3829, 4581`, e **`Table::acrescentar_coluna` está
em `table.rs:904` — em nenhum deles**; `reg.rs`, onde moram
`acrescentar_coluna` e `escrever_volume_alargado`, **não foi tocado**. O binário
mede o mesmo código. Aferido, não suposto.

```
./target/release/examples/custo-do-alter 200000 1000000 2000000

    linhas     preparo       alter    us/linha  MiB antes      MiB/s
    200000       1.44s      0.158s       0.789       25.6      339.5
   1000000       7.36s      1.034s       1.034      127.8      259.2
   2000000      15.04s      1.382s       0.691      255.6      387.9
```

**Uma passada custa 0,691–1,034 µs/slot** nesta máquina hoje (contra 0,553 na
medição histórica — máquina mais quieta).

**A migração v10 são exatamente DUAS passadas**, e isto é do código, não de
estimativa: `COLUNAS_DO_V10` (`table.rs:109`) tem dois elementos —
`rowstamp` (`UInt8`) e `rowtime` (`DateTime`) — e `migrar_para_psch_v10`
(`table.rs:1087`) roda um `acrescentar_coluna` por coluna faltante.

**Trava global presa, então:**

| slots | 1 passada (medida) | migração = 2 passadas | vezes a unidade da catraca (1,3 ms) |
|---:|---:|---:|---:|
| 2.000.000 | 1,382 s | **2,76 s** | ~2.100× |
| 10.000.000 | 6,9 s (projeção do medidor) | **~13,8 s** | **~10.600×** |

**E dobra de novo com `recursos.espelho` ligado**: `acrescentar_coluna`
(`reg.rs:1462-1476`) chama `escrever_volume_alargado` outra vez para o `.bkp` de
cada volume, dentro da mesma passada.

**O que eu NÃO medi, e digo:** não medi a `migrar_esquema` de ponta a ponta.
Medi a **unidade** (uma passada de `acrescentar_coluna`) e multipliquei por duas,
que é o que o código faz. Os 10 M são **projeção do próprio medidor**, rotulada
como tal na saída dele.

**O que a mediria exatamente, e é barato:** a operação **já devolve o número** —
`op_migrar_esquema` empilha `("ms", …)` em `servidor.rs:16256`. Basta semear uma
tabela v9, chamar `migrar_esquema` com `confirmar`, e ler o campo. Não o fiz
porque exige subir servidor e construir (disco em **2,5 GiB** livres, e `--release`
está vedado nesta tarefa).

**Conclusão da §1:** a catraca foi calibrada em **1,3 ms por seção**. Esta seção
é **três a quatro ordens de grandeza** acima disso. Ela não é «mais uma»: é, com
folga, **a maior detentora da trava global de todo o conjunto de 88**.

---

## 2. Pode rodar sob trava de TABELA? — **Não. Não há o que tomar.**

Esta é a resposta que importa, e ela não é de princípio: é do código.

**2.1 — A trava global é a ÚNICA exclusão que existe para tráfego comum.**
`dados: RwLock<Raiz>` (`servidor.rs:817`). `travar_dados()` (`:1628`) é
`.write()`; `travar_dados_para_ler()` (`:1722`) é `.read()`. Não existe trava por
tabela no motor. O que parece uma é o gestor `travas: Mutex<Travas>`
(`:889`), e ele é **de transação**, com três limites medidos:

- **`barrado_por_travas`** (`servidor.rs`, corpo do gate) **curto-circuita em
  `transacoes_abertas == 0`**: sem transação aberta no servidor, todo escritor
  comum **pula o portão inteiro**. Uma migração que tomasse X por ali não
  incrementaria esse contador, e os escritores passariam direto por cima do
  arquivo sendo reescrito.
- **Ele não espera, recusa** — `EM_TRANSACAO` com `repetir: true`. Isso é
  deliberado e está certo para o que ele faz; para a migração significaria que
  todo cliente antigo passa a receber um erro que não sabe tratar durante a
  janela — e isso esbarra em *«guarda nova entra pedida, não imposta»*.
- **O campo que ele lê é o furo**, e a lei da casa já nomeia isto: ele olha
  `tabela` e `destino`. As operações que escondem a tabela (`juntar`, `unir`,
  `pivotar` — hoje **10 de 141**, pela recontagem do `a671f2d`) **não seriam
  barradas**.

**2.2 — O que a trava global protege aqui que a de tabela não protegeria.**
Não é desempenho. São **duas garantias de formato**:

**(a) A janela de volumes MISTURADOS — FASE B.** O comentário em
`reg.rs:1480-1485` diz: *«FASE B — trocar. Só `rename` … a janela em que a tabela
pode ficar misturada é de n renomeações»*, com o volume 1 como ponto de
compromisso. Um leitor que abra dentro dessa janela pega um conjunto com volume 1
já novo e volume 3 ainda velho — e o `abrir` **recusa o conjunto misturado
nomeando o volume**. Hoje isso é **inobservável**: leitores seguram `.read()` e a
FASE B segura `.write()`. Sob trava de tabela a janela fica exposta, e a recusa é
de classe `Corrompido` na cara de quem só estava lendo.

**(b) A obsolescência do `*.novo` — FASE A, e esta é a grave.** A FASE A
(`reg.rs:1440-1478`) escreve um `*.novo` completo **ao lado** do volume vivo; o
arquivo vivo continua sendo o velho e inteiro. Isso é o que a torna retomável.
Mas o `*.novo` é um **retrato**: uma linha inserida no volume velho durante a
FASE A **não está nele**, e a FASE B renomeia por cima. **Escrita confirmada,
perdida, sem bilhete.** Hoje o único impedimento é a trava global.

**(c) O registro `sujas` fecha por REABERTURA.** `sujas:
Mutex<HashSet<String>>` (`servidor.rs:826`) guarda `database/tabela` por **nome**,
e `descarregar_sujas_com` (`:15770`) reabre cada tabela para sincronizar. Sob
trava de tabela a janela do *group commit* poderia fechar no meio da migração e
sincronizar um handle para um arquivo que está sendo renomeado.

**Veredito da §2:** a migração **não pode** rodar sob trava de tabela hoje —
não porque a de tabela seja insuficiente em teoria, mas porque **exclusão de
escopo de tabela para tráfego não-transacional não existe neste motor**.

---

## 3. Precisa da global → pode soltar entre passadas? — Sim para queda, **não** para vizinho

**Ela já é retomável, e isso é verdade**: `migrar_para_psch_v10` recalcula
`colunas_do_v10_que_faltam()` a cada chamada (`table.rs:1084`) e a FASE A/FASE B
com o volume 1 como compromisso sobrevive à queda. A frente do 407 provou com
queda real. **Mas retomável contra QUEDA não é seguro contra VIZINHO**, e este é
exatamente o eixo que esta casa já errou e pagou, no pedido 180:

> *«Eu concluí primeiro que a quebra era segura, e o erro era de eixo: a linha
> que reprova não é uma queda, é uma queda **depois de um vizinho ter escrito**.»*

Soltar a trava entre as passadas 1 e 2 é seguro contra queda (a passada 2 refaz o
que falta) e **inseguro contra escrita concorrente**, pela §2.2(b): um `inserir`
entre as passadas entra no volume da passada 1 e some no `rename` da 2.

**E fica registrado o conserto que NÃO se deve fazer:** mover o `drop(dados)`
para antes da FASE A **compila, passa nos testes e deixa a catraca em 24** — e é
perda de dado. É a mesma armadilha, letra por letra, que a frente do PITR
documentou no pedido 252 (*«isso compila, passa nos doze testes do PITR e deixa a
catraca em 24, e é corrupção»*). Quem pegar esta tarefa vai tropeçar aqui.

---

## 4. Os quatro motores — convergência, e eu decido

Por escopo de trava num `ALTER TABLE` que reescreve a tabela inteira. As duas
primeiras citações são do **help oficial, já transcrito nesta casa**
(`docs/PESQUISA-ESTADO-DERIVADO.md:265-282`):

| motor | peso | escopo | fonte |
|---|---:|---|---|
| PostgreSQL | 4 | **tabela** — `ACCESS EXCLUSIVE`: *«the holder is the only transaction accessing the table»*, e *«many forms of ALTER TABLE also acquire a lock at this level»* | manual de *explicit locking* |
| MariaDB | 3 | **tabela** — MDL, mais `ALTER ONLINE` | família MySQL |
| MySQL | 2 | **tabela** — *«a metadata lock on a table prevents changes to the table's structure»*; `INPLACE`/`LOCK=NONE` ainda deixa DML concorrente por *row log* | manual de *metadata locking* |
| SQLite | 1 | **base inteira** — não tem trava de tabela | um arquivo = uma base, embarcado |

**Soma: escopo de tabela 4+3+2 = 9; escopo de base 1.** Os três maduros
convergem, e **SQLite diverge por arquitetura, não por semântica** — ele não tem
servidor para segurar.

**Nenhuma pétrea nossa se opõe ao COMPORTAMENTO.** A ordem de digitação é
preservada pela migração (reescrita slot a slot, rowid é a posição); a
integridade referencial não é tocada; zero dependências não é tocado. Então,
pela cláusula de 23/09/2026: **aceite automático, eu decido, não sobe ao dono.**

> **Aceito o alvo: um DDL que reescreve a tabela não deve segurar o SERVIDOR.**
> `migrar_esquema` está errada neste ponto, e as três irmãs também.

**E o limite, que é o precedente do TLS:** o aceite é **do comportamento, não do
meio**. Nós não temos o meio (§2.1). Então isto entra como **meta medida com
pendência**, não como conserto pronto — e a catraca fica vermelha enquanto isso,
que é o retrato honesto.

---

## 5. Por que não (B) e por que não (C)

**(B) recusar acima de um tamanho — NÃO FUNCIONA, e é fato medido, não opinião.**
O `mapa-da-trava.py` é **analisador estático**: ele lê o texto, monta o grafo de
chamadas (`alcancaveis`, linha 462) e conta caminho **alcançável**. Uma guarda de
tamanho é decisão de **execução** e é **invisível** para ele. A seção continuaria
alcançando `escrever_volume_alargado`, e a catraca continuaria em 25. **(B) não
desce a catraca**, e adotá-la seria pagar uma recusa ao usuário sem comprar a
guarda. *Não há teto a propor porque o teto não resolveria nada.*

**(C) mudar a régua para «caminho QUENTE» — NÃO, e o número explica.**
Eu aposentaria e recomeçaria se a régua estivesse medindo coisa errada. Ela não
está, e há três medições contra:

1. **A régua sempre contou DDL.** Das 24 de ontem, **quatro já eram DDL**
   (`op_declarar_fk`, `op_acrescentar_coluna`, `op_excluir_fk` e, das outras,
   várias administrativas). Tirar DDL agora não seguraria em 24: **cairia para
   ~21**, e um corte «só caminho quente» de verdade levaria o número a **~10** —
   as administrativas (`op_idiomas_*`, `op_reparar`, `op_reindexar`,
   `op_esvaziar_lixeira`, `op_marcar_lgpd`, `op_ajustar_sequencia`,
   `op_dblink_sincronizar`, `semear_mensagens`) também sairiam. Isso não é ajuste
   de régua: é uma catraca **diferente**, que deixaria de guardar justamente a
   metade rara.
2. **A régua mede TEMPO DE TRAVA PRESA, e a rara é a pior.** O texto da própria
   catraca: *«cada uma nova é 1,3 ms de trava presa que a próxima conexão
   espera»*. `op_migrar_esquema` prende a trava por **2,76 s a 13,8 s** (§1).
   Isentá-la por ser rara **inverte o propósito**: removeria do contador a maior
   detentora de todas. Uma parada de 13,8 s uma vez é pior, para quem está do
   outro lado, que 1,3 ms dez mil vezes — porque as dez mil são intercaladas e a
   uma não é.
3. **Rara não é o critério; o critério é «alcança `fsync` com a trava na mão».**
   Isso é exatamente o que esta seção faz, sem condicional nenhuma.

Ou seja: a régua **não** passou a medir coisa diferente. **Subiu o risco.** E a
lei diz o que fazer com risco que subiu: *«Desfaça, ou traga o motivo ao dono»* —
não aposentar a régua.

---

## 6. (A) — o conserto, o que ele custa, e o que não se perde

**A mudança, e ela é de FORMA da seção, não de formato em disco.** O padrão já
está provado nesta casa: é o `ae8a58b` do pedido 252, onde o PITR saiu da
catraca **por mérito**. A lição de lá: não se move o `drop`; **muda-se ONDE se
escreve** — o trabalho caro acontece no **palco**, antes de o resultado entrar na
raiz, e por isso soltar a trava antes do `fsync` é seguro.

**Na migração o palco já existe: são os `*.novo` da FASE A.** Então:

1. `travar_dados()` — plano, recusas, e **marcar a tabela «em migração»**;
2. **`drop(dados)`** — solta a trava global;
3. **FASE A** com o `fsync` (`reg.rs:2586`) fora da trava;
4. `travar_dados()` de novo — **revalidar** que o `slot_count` não mudou;
5. **FASE B** (só `rename`, sem E/S de dado) e limpar a marca.

**E o mapeador credita isso honestamente — conferi.** `mapa-da-trava.py:455-459`
procura um `drop(<guarda>)` dentro da seção e **trunca a seção ali**
(`solta_cedo`). Não é enganar o medidor: a trava **genuinamente** não está na mão
durante o `fsync`. Era a pergunta certa a fazer, e a resposta é que o conserto é
real.

**O que falta para (3) ser seguro, e é o custo honesto de (A):** o **congelamento**
da tabela no passo 1, que hoje não existe (§2.1). Ele precisa de um registro
consultado no ponto único onde uma tabela é aberta para cliente —
**`abrir_travada_com`, `servidor.rs:10917`** —, que é onde a lei *«o portão é UM
só»* já manda pôr. E precisa **recusar, não travar**, sem quebrar cliente antigo.

**Portanto:**
- **(A) é a direção certa e a única verdadeira**, mas é **frente com desenho**, não
  edição de linha. Quem prometer «uma linha» está prometendo o conserto que perde
  dado (§3).
- **Alcance: as quatro irmãs**, não só a 25ª — senão o irmão fica, que é o defeito
  que esta casa já pagou três vezes em 03/09.
- **Enquanto não entra: teto fica em 24, catraca fica VERMELHA.** Há precedente
  exato e o mecanismo para isso existe: o `--numeros` sai com código 0 de
  propósito para que a vermelha **apareça vermelha** no inventário em vez de
  sumir dele.
- **A saída barata que a catraca também oferece** — *«Desfaça»* — seria reverter a
  porta do 407. Nomeio e **não recomendo**: devolveria 24 na hora, e devolveria
  também o defeito que o 407 existia para consertar (tabela v9 sem caminho
  nenhum para o v10). Trocar um risco medido por um beco sem saída não é
  melhoria; mas é decisão de produto, e fica dita.

---

## 7. Achado de passagem, fora do pedido

`op_migrar_esquema` **não consulta `sujas`** antes de reescrever. Hoje é inócuo —
a trava global garante que nenhuma janela de *group commit* está aberta. Mas é
uma dependência **implícita** da trava global que ninguém escreveu: qualquer
conserto que solte a trava tem de tratá-la junto (§2.2c). Não é defeito hoje;
é uma amarra invisível que vira defeito no dia do conserto.

---

## 8. O que eu não medi

- A `migrar_esquema` de ponta a ponta (medi a unidade; o caminho está na §1).
- Os 10 M são projeção do medidor, não corrida.
- O custo do congelamento proposto na §6 — é desenho, e desenho não se mede antes
  de existir.
- Não li o fonte de PostgreSQL/MySQL/MariaDB/SQLite nesta sessão: usei o help
  oficial **já transcrito** em `docs/PESQUISA-ESTADO-DERIVADO.md:265-282` para PG
  e MySQL, e conhecimento de manual para MariaDB e SQLite. A convergência dos
  três maduros é sólida; se alguém quiser o vetor, é uma leitura de manual, não
  uma medição.
