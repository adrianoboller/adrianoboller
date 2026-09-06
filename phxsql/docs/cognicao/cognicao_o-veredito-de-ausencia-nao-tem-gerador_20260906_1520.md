# O veredito de AUSÊNCIA não tem gerador — e por isso ninguém o reconfere

**06/09/2026, 15:20.** Descoberto respondendo à pergunta do dono: *«o que falta
no PhxSql para ser melhor que o HFSQL? o que está completo, incompleto ou só
planejado?»*

## 1. O que aconteceu

O projeto já tinha a resposta escrita: `docs/HFSQL.md`, 204 linhas, item por
item contra a folha do HFSQL(R). Remedido contra a 0.18.0, **cinco dos seus
verdictos estavam vencidos** — e os cinco na mesma direção:

| dizia | era |
|---|---|
| «Não há transação aqui» | há `BEGIN`/`COMMIT`/`ROLLBACK`/`SAVEPOINT` |
| «Gatilhos e procedimentos: projeto grande» | existem, num interpretador só |
| «Marcar coluna como dado pessoal: falta» | existe: `dado_pessoal` no esquema, três ops, arquivo `.lgpd` |
| «ODBC e OLE DB: projeto grande» | ODBC provado com 73 conferências |
| «Cluster — não» | há, com eleição e promoção automática |

O arquivo tinha sido tocado no dia anterior (`71780b1`, 05/09) — não estava
esquecido num canto. Foi lido, editado, e os cinco passaram.

## 2. O que eu concluí primeiro, e estava errado

Concluí que o documento estava velho **porque ninguém o revisava**, e que a
correção era revisá-lo agora. Se fosse isso, a regra seria «revise o `HFSQL.md`
a cada rodada» — mais uma obrigação de memória, do tipo que esta casa já sabe
que não se cumpre.

Estava errado, e o commit de 05/09 é a prova: o arquivo **foi** revisado, e os
cinco sobreviveram à revisão. Não é falta de leitura.

E errei uma segunda vez no meio do caminho, de um jeito que vale mais que o
primeiro: fui medir as lacunas com `grep -c` e publiquei para mim mesmo
«gatilhos: **0** no catálogo». Zero verdadeiro na contagem, veredito falso na
conclusão — gatilho e procedimento entram pela op `sql`, não por operação
própria. **Contagem crua não é veredito**, e uma contagem que dá zero é a mais
perigosa de todas, porque zero se parece com ausência.

## 3. O que a medição disse

O que distingue os cinco vencidos dos números vencidos desta casa:

- **Número errado tem denunciante.** O selo dizia 0.11.0 por quatro
  lançamentos e alguém bateu o olho; o rodapé dizia 780 KiB quando eram 1.032 e
  a conta não fechou. Número errado **conflita** com alguma coisa.
- **Ausência errada não conflita com nada.** «Cluster — não» convive em paz com
  `bancada/cluster/`, com sete operações no catálogo e com o `docs/CLUSTER.md`
  ao lado dizendo o contrário. Nenhum gerador cobre a frase, porque não há
  número nela; nenhum leitor a confere, porque não há o que olhar.

Medido na mesma passada, e é o contra-exemplo que fecha o raciocínio: o
`PENDENCIAS.md` #30 publica «108 operações» e são **122** (contadas do array
`OPERACOES` do `catalogo.rs`: 122 entradas, 122 campos `nome:`). Esse é um
número — envelheceu igual, mas basta contar para achá-lo. A ausência, não.

## 4. A regra

**Veredito de ausência se remede por data, não por suspeita** — e o documento
que o carrega traz a data da remedição no alto, como as páginas de teste já
trazem. E, ao medir: **contagem crua não é veredito; zero é o resultado que
mais exige olhar o código.**

## 5. Como está guardado hoje

- `docs/HFSQL.md` reescrito, com a data da remedição no alto e uma §6 que lista
  os cinco verdictos errados e o padrão deles.
- O mesmo vencido está **num segundo lugar**: `docs/SQL.md` §3 ainda lista
  «Transação. Não há» — e o próprio arquivo tem a §2b descrevendo
  `BEGIN`/`COMMIT`. **O buraco fica aqui declarado**: não consertei o `SQL.md`
  nesta passada, e ele está nomeado na §3.7 do `HFSQL.md` para não se perder.
  É o padrão do *conserto entra no caminho que o motivou, e o caminho irmão
  fica* — só que desta vez o irmão está nomeado antes de morder.
- **Sem guarda automática, e isso é decisão, não esquecimento.** Um conferidor
  que casasse frases de negação («não há», «falta», «— não») contra o código
  reprovaria as dezenas de negações legítimas deste repositório, que são
  recusas fundamentadas com número. É o mesmo veredito das 8 interpolações de
  erro cru: o casador de texto acusaria os inocentes. O que fica no lugar é a
  data no alto do documento, que torna a idade **visível** em vez de
  detectável.
