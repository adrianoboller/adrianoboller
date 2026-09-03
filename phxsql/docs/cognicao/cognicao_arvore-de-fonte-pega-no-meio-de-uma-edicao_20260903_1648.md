# «Árvore limpa reprova» tem uma terceira causa: o fonte pego no meio de uma edição

**Descoberto em 03/09/2026, 16:48**, caçando teste que passa por engano.

## 1. O que aconteceu

O executor das guardas copia `crates/`, `exemplos/`, `docs/`, `Cargo.toml` e
`Cargo.lock` para `~/.cache/phx-guardas` e só mexe lá. Rodando a caça com uma
lista própria de defeitos, a **árvore limpa** reprovou antes de qualquer
defeito ser reposto:

```
phxsql-server --lib   nao compilou  3,1 s
  error[E0583]: file not found for module `phx`
  error[E0004]: non-exhaustive patterns: `dblink::Motor::Phx` not covered
```

A cópia tinha `crates/phxsql-server/src/dblink/mod.rs` declarando
`pub mod phx;` na linha 26, e **não tinha** `dblink/phx.rs`. A árvore de
verdade tinha os dois.

## 2. O que eu concluí primeiro, e estava errado

Que era o `COPIAR` incompleto — a causa que a cognição de 03/09 às 02:46
(`cognicao_alcance-da-copia-do-executor-de-guardas`) documenta, e cuja regra
diz: *«se o arquivo é IDÊNTICO ou AUSENTE, a causa é o `COPIAR`»*. O arquivo
estava AUSENTE, então a regra existente apontava direto para lá.

Errado. O `_sincronizar` percorre a **origem** com `os.walk` e copia tudo que
achar — um arquivo novo entra sozinho, e ele apaga da cópia o que sumiu da
origem. Nenhuma lista digitada estava furada.

Também considerei a segunda causa que o `LEIA-ME.md` da pasta documenta —
duas árvores de trabalho disputando a mesma cópia. Também errado: a cópia
tinha sido sincronizada desta árvore, e minutos antes ela compilava (a rodada
das quatro guardas redundantes, 16:39, saiu com `phxsql-server --lib` verde em
667 testes).

## 3. O que a medição disse

As datas dos dois arquivos, na árvore de verdade:

```
16:43:37  crates/phxsql-server/src/dblink/mod.rs     (já com `pub mod phx;`)
16:46:05  crates/phxsql-server/src/dblink/phx.rs     (2 min 28 s depois)
```

O `montar` da minha rodada correu dentro dessa janela. **A árvore de origem
estava, ela própria, num estado que não compila** — outra frente escreveu o
`mod.rs` que declara o módulo antes de escrever o módulo. A cópia é fiel; o
original é que não estava consistente.

Segunda ocorrência na mesma tarde, com outro sintoma e outra frente: uma
rodada seguinte pegou `phxsql-store` com
`error[E0599]: no method named contains found for enum Result` — outra edição
em voo, em `apoio_teste.rs`.

## 4. A regra

**Antes de culpar a cópia, olhe a DATA dos arquivos da origem.** Numa árvore
compartilhada com frentes paralelas, «árvore limpa reprova» tem três causas, e
elas se separam em segundos:

| o arquivo na cópia | a causa |
|---|---|
| **difere** do da origem | contaminação entre rodadas (`LEIA-ME.md`) |
| **ausente na cópia e na origem** | `COPIAR` incompleto (cognição das 02:46) |
| **ausente na cópia, presente na origem, com `mtime` recente** | o fonte foi copiado no meio de uma edição — **esperar, não consertar** |

E o corolário para quem escreve executor: **a conferência da árvore limpa tem
de PARAR a rodada.** O `provar-guardas.py` para; o meu roteiro de caça não
parava, e a rodada saiu com **seis** «NÃO COMPILOU» com cara de seis medições
— seis defeitos repostos sobre um binário que já não compilava — ao lado de
duas medições legítimas, dos dois binários de teste que não dependiam do
`phxsql-server` e estavam verdes na mesma passada. Seis e dois, na mesma tela,
com o mesmo formato: é a versão nova do «teste que passa por engano», do outro
lado — a **medição** que falha por engano.

## 5. Como está guardado hoje

Nada mudou no `provar-guardas.py` — ele já para, e já registra as duas causas
que conhecia. O que entra aqui é a terceira linha da tabela da seção 4, para
que a próxima sessão não gaste a rodada procurando um furo no `COPIAR` que não
existe.

**Onde o buraco ficou:** não há como o executor distinguir sozinho «a entrada
envelheceu» de «peguei o fonte no meio de uma edição» — os dois chegam como
`QUEBRADA` ou `nao compilou`. Um aviso barato existiria: comparar o `mtime`
mais novo de `crates/` com a hora do `montar` e dizer «a origem mudou há N
segundos» quando N for pequeno. Fica registrado como falta, e não como feito.
