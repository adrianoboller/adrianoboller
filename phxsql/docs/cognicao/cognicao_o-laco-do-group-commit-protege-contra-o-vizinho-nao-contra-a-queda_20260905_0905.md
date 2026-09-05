# O laço do group commit protege contra o VIZINHO, não contra a queda

*Descoberto em 05/09/2026, 09:05, decidindo o pedido 180 — se dá para soltar a
trava global entre uma tabela e a seguinte no fecho da janela de durabilidade.*

## 1. O que aconteceu

O comboio do fecho estava medido e o mecanismo lido no fonte (§12 do
`docs/CONCORRENCIA.md`): `descarregar_sujas_com`, em
`crates/phxsql-server/src/servidor.rs`, roda **com a trava global de dados na
mão** e faz `abrir_database → abrir_qualificada → sincronizar` por tabela suja,
em laço, sem soltar. Com os escritores fixos em 4 e variando só quantas tabelas
distintas eles escrevem, o p99 do escritor sobe 2,25× de K=1 para K=4 e o do
**leitor**, que lê uma tabela que ninguém escreve, 2,01×.

O que ficou por decidir foi de propósito, e é o assunto deste arquivo: o laço
apaga as marcas de commit `.tx` **depois** de todas as tabelas sincronizarem, e
o comentário dizia *«esta é a ordem que faz o group commit ser seguro, e ela
não se inverte»*. Quebrar o laço mexe nessa ordem.

## 2. O que eu concluí primeiro, e estava errado

**Concluí que a ordem era o que o laço protegia, e que a matriz de queda ia ser
sobre INSTANTES.** Escrevi as seis linhas «queda antes da marca, queda no meio
da passada, queda entre a tabela A e a tabela C…», cruzei com o que a
recuperação faz, e as seis deram ✔ — inclusive no laço **quebrado**. Cheguei a
concluir que a quebra era segura.

Estava errado, e o erro era de eixo: **uma queda sozinha não quebra nada.** A
recuperação anda para a frente e é idempotente pelo rowid; se a marca está no
disco, o commit volta inteiro, tenha a queda acontecido onde tiver. Enquanto a
matriz só tinha «instante da queda», o laço quebrado passava por ela.

O que faltava na matriz era o **outro ator**. A linha que reprova não é uma
queda: é uma queda **depois de um vizinho ter escrito**. Soltar a trava entre
uma tabela e a seguinte deixa outro escritor entrar; a janela **reabriu** ao
fechar, então a gravação dele apenas suja a tabela e pendura a marca dele em
`marcas_pendentes` — e o fim do laço apaga **todas** as marcas pendentes, a
dele inclusive, sem nunca ter sincronizado a tabela dele.

Ou seja: o laço não protege uma ordem contra a queda. Protege o **encontro
inteiro contra o escritor concorrente** — e a queda é só o que revela.

## 3. O que a medição disse

**Que o defeito seria invisível para a bateria que existe.** A
`bancada/durabilidade/prova.py` prova com `SIGKILL`, e página suja no cache do
núcleo **sobrevive** a processo morto (medido no pedido 186 e de novo em
`docs/ACID.md` nesta rodada). O Q8 da matriz só aparece em queda de energia, e
nenhum processo em espaço de usuário provoca uma. As 27 corridas da §5.7 do
`docs/TRANSACOES.md` continuariam verdes com a perda de dado de pé.

E que **o Q8 não é corrida rara, é o caminho comum**: quem fecha a janela a
reabre ao fechá-la, então toda gravação que chegar durante o fecho cai
exatamente nesse estado.

**E a medição que mudou a decisão foi outra, e mais barata.** Antes de pagar a
contabilidade por marca que tornaria a quebra segura, medi a pergunta que
ninguém tinha feito: *esse `K × fsync` precisa mesmo ser em série?* Não precisa
— não há ordem **entre** tabelas para preservar, só dentro de cada uma. Duas
baterias limpas, alternando os dois arranjos dentro da mesma corrida
(`--example o-comboio-em-paralelo`):

| K | ganho A | ganho B |
|---:|---:|---:|
| 4 | 1,62× | 1,57× |
| 8 | 2,06× | 1,97× |
| 16 | **2,52×** | **2,42×** |

Com o **mesmo número de `fsync`**: 32 para K=4 nos dois arranjos, contados por
`strace` num processo filho — 8 por tabela, que é a `TETO_FSYNC_POR_FECHO_V2`.
Ganho de tempo num caminho de durabilidade que viesse de `fsync` a menos não
seria ganho, e é a única forma de esse número ser falso.

## 4. A regra

**Matriz de queda que só tem instantes não prova concorrência: ponha o vizinho
na matriz.** Quando a proposta é soltar uma trava, a linha que reprova não é
«caiu aqui» — é «caiu aqui **depois** de outro ter escrito».

E o corolário, que vale para todo comentário de invariante: **quando o
comentário diz uma ordem e a garantia depende de atomicidade, ele está
listando menos casos do que existem** — e protege menos no dia em que alguém
usar a lista como inventário.

## 5. Como está guardado hoje

* A matriz de queda inteira, nas duas metades (laço de hoje e laço quebrado),
  na **§12.6 do `docs/CONCORRENCIA.md`**, com a decisão do papel C: recusada a
  quebra, aceito o `fsync` em paralelo.
* O invariante de verdade **no código**, no comentário de
  `descarregar_sujas_com` — que agora diz o que faltava: *«a ordem sozinha não
  é o que faz; o encontro inteiro acontece sob uma tomada só da trava»*.
* Três guardas no `servidor.rs`, e as três provadas com o defeito reposto:
  `tabela_que_nao_sincroniza_segura_as_marcas` (a falha na abertura),
  `fsync_que_falha_no_fio_tambem_segura_as_marcas` (a falha **dentro do fio**,
  que é o único caminho que passa pelo `join`) e
  `com_todas_sincronizadas_as_marcas_saem` — esta última medindo o **fato**
  (`volume::familias_devendo_em`) e não a intenção, porque é a única que pega
  um fio que nunca subiu.
* Uma guarda no `phxsql-store` contando os `fsync` dos dois arranjos por
  `strace`: `tests/fecho-em-paralelo-conta-os-mesmos-fsync.rs`.
* Duas entradas novas no `bancada/guardas/catalogo.py`
  (`fecho-em-paralelo-engole-o-erro` e `fecho-em-paralelo-fio-que-nao-sobe`),
  as duas PROVADAS pelo `provar-guardas.py`.

**Onde o buraco ficou:** a contabilidade por marca que tornaria a quebra da
trava segura — `(marca, [(tabela, geração)])`, o conjunto de sujas virando
`tabela → geração`, e um portão de reentrância para dois fechos não se
atropelarem — está **desenhada e não feita**, na §12.6.3(c). Ela não muda
formato em disco; muda o protocolo do group commit, que é o que a §5.7 do
`docs/TRANSACOES.md` prova célula a célula. Quem a fizer refaz a matriz antes.
