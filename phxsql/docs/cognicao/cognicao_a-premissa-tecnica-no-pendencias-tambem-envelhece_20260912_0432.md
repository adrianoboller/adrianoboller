# A premissa técnica escrita no PENDENCIAS também envelhece — o gap 238 não era parede do dono

Data da descoberta: 12/09/2026, 04:32 (o J mediu; eu conferi no fonte em
seguida).

## 1. O que aconteceu

No PDCA dos gaps, o papel J foi medir a premissa do gap **238** (parametro de
SAIDA/ENTRADA-SAIDA no driver ODBC). O `docs/PENDENCIAS.md #238` e o
`docs/ODBC.md §2.1.1` diziam, havia rodadas, que fechar isso "exige a op do
servidor devolver um valor alem da linha, que nenhuma operacao tem — e decisao
de protocolo". A premissa e **falsa**: a op `chamar_procedimento` (que atende
`CALL`) ja monta e devolve um objeto `saida` para todo parametro OUT/INOUT. O
gap era conserto de **driver**, nao decisao de protocolo — e estava classificado
como "parado no dono" a toa.

## 2. O que eu concluí primeiro, e estava errado

Na minha triagem inicial dos gaps eu classifiquei o 238 como "bloqueado no dono
(protocolo)" — lendo o **rotulo** escrito na linha do PENDENCIAS, sem remedir a
premissa no fonte. Confiei na frase registrada em vez de conferir o codigo. Se o
dono nao tivesse mandado convocar o J, eu teria deixado o 238 como parede e
nunca teria visto que o conserto ja era possivel.

## 3. O que a medição disse

- **Servidor:** `crates/phxsql-server/src/servidor.rs:15062-15078` — a
  `chamar_procedimento` monta `saida` a partir de todo parametro cujo
  `modo != Modo::Entrada`, lendo `ctx.valor_de(&q.nome)` **apos** `executar`, e
  devolve `Json::objeto([("procedimento", …), ("saida", Json::Objeto(saida))])`.
  O servidor JA devolve valor alem da linha.
- **Driver:** `crates/phxsql-odbc/src/parametro.rs:276` recusa parametro
  nao-entrada com `HYC00`; `crates/phxsql-odbc/src/resultado.rs:218-264` so le
  `resposta.campo("linhas")` e ignora `saida`.
- **Convergencia:** PostgreSQL e MySQL tambem entregam OUT como conteudo do
  resultado da chamada (row de resultado / resultset marcado
  `SERVER_PS_OUT_PARAMS`), nao como campo de fio exotico — o nosso `saida` e o
  analogo direto. Zero de novo no protocolo.

## 4. A regra

Antes de carimbar um gap como "decisao do dono" ou "parede", **remeça a premissa
tecnica dele no fonte**. A frase escrita na linha do PENDENCIAS envelhece igual a
limitacao de infraestrutura — e um rotulo velho afasta todo mundo do conserto
que ja e possivel.

## 5. Como está guardado hoje

Isto **nao e lei nova**: e o **alcance** da petrea que ja existe — *"limitacao
registrada tambem envelhece"* (nasceu do 403 do push, `docs/BACKUP.md`, e a
prova ao lado foi o que a conservou por tres rodadas). O que este arquivo
registra e que ela cobre tambem **premissa tecnica classificada no PENDENCIAS**,
nao so bloqueio de infraestrutura.

Guardado no `docs/PDCA-GAPS.md` (secao do gap 238), com a recomendacao medida:
238 e trabalho de driver (papel B), tres passos em `crates/phxsql-odbc/`. As
correcoes das duas linhas velhas (`PENDENCIAS #238`, `ODBC.md §2.1.1`) ficam para
o commit que efetivamente ligar o OUT no driver — nomeadas na ultima secao do
`PDCA-GAPS.md` para quem as fizer nao precisar remedir nada.
