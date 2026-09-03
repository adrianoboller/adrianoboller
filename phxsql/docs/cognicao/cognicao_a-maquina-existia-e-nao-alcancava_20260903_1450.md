# A máquina existia, rodava, e não alcançava a tabela que ia consertar

**Descoberto em 03/09/2026, 14:50**, implementando o pedido 172.

## 1. O que aconteceu

A recuperação de transação recusava refazer a cascata quando o `.ndx` da filha
ficava sujo — mãe no valor novo, fração das filhas no velho, marca já apagada.
A `§5.5.3` registrava isso como **`IMPOSSÍVEL`, de propósito**, com o
argumento: *«consertar pediria a recuperação saber tentar de novo mais tarde…
a marca já não existe mais para retomar»*.

A máquina de reconstruir índice sujo estava **trinta linhas acima**, no mesmo
`completar()`, rodando para toda tabela **nomeada na marca** — com um
comentário dizendo que a prova por soquete a tinha achado e que reconstruir
era *«o único caminho honesto»*.

Ela não alcançava a filha da cascata, e o motivo é estrutural: a cascata nunca
vira `Escrita`, então a filha não aparece em marca nenhuma. **O `completar()`
itera `marca.operacoes` e indexa por `op.tabela`.**

## 2. O que eu concluí primeiro, e estava errado

Antes de medir, li o `recascatear` e **dispensei o achado**: escrevi que era
benigno porque *«na recuperação a mãe já está no disco, então não há o que
proteger recusando cedo»*. Isso era sobre o defeito irmão (§5.5.4), e eu já
registrei o erro lá.

Aqui o erro foi outro, e mais caro: **eu acreditei na `§5.5.3`.** Ela dizia
«de propósito» e dava um argumento, e argumento escrito parece decisão tomada.
Só que o argumento respondia a **outra proposta** — retomada adiada — e não à
que o código já tinha pronta. Uma limitação pode estar registrada, argumentada
e **errada ao mesmo tempo**, porque o argumento envelhece junto com o código
que ele descrevia.

E a terceira coisa que eu errei foi na **montagem do teste**, duas vezes
seguidas: levantei o byte 52 do cabeçalho na mão (caiu com *«CRC inválido»* —
o cabeçalho protege a própria marca, então virar o bit simula **adulteração**,
não queda), e depois deixei a linha pendente apontando para o pai que a mãe
tinha acabado de deixar de ser (caiu na chave conferida). **Cenário que falha
pelo motivo errado engana duas vezes: uma quando passa e outra quando cai.**

## 3. O que a medição disse

* **Custo:** ~**2,2 µs por linha**, linear — 2,2 ms a mil, 21,2 ms a dez mil,
  219 ms a cem mil, 1,16 s a meio milhão. Não é classe nova de gasto: é o mesmo
  preço que a recuperação já pagava pelas outras tabelas.
* **Frequência:** a matriz de durabilidade mediu **9 de 21 corridas** caindo
  nesse caso, com `SIGKILL` real.
* **Prova real:** desligado o interruptor, o commit volta para
  `operacoes IMPOSSIVEIS`; ligado, completa. Suíte em **1.547**.

## 4. A regra

**Limitação registrada envelhece com o código, e o que se remede é a PREMISSA
dela, não a conclusão.** Antes de aceitar um «impossível de propósito»,
pergunte contra qual proposta o argumento foi escrito — se a que você tem na
mão é outra, a decisão não cobre o seu caso e nunca cobriu.

## 5. Como está guardado hoje

`Table::ligar_reconstrucao_do_indice_da_filha`, que nasce **desligada** e é
ligada só pela recuperação — *guarda nova entra pedida, não imposta* —, com o
número entrando em `indices_reconstruidos` para que o relatório **conte** em
vez de reparar em silêncio. Guarda `recuperacao-nao-reconstroi-a-filha`,
**provada**, com as duas recuperações sem índice sujo como controle. A `§5.5.3`
deixou de dizer «de propósito» e passou a contar por que a premissa caiu.

O que **não** está guardado: nada procura, hoje, por «mecanismo que roda num
caminho e não no irmão». As três instâncias do dia foram achadas por leitura, e
a lei entrou no `CLAUDE.md` justamente porque o conferidor genérico está
**recusado com número** — 8 candidatos, 2 defeitos.
