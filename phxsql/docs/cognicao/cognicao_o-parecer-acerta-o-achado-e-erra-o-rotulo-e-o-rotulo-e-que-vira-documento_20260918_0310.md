# O parecer acerta o achado e erra o rótulo, e o rótulo é que vira documento

**18/09/2026, 03:10** — hora da descoberta, integrando três pareceres (SEC,
C/DBA e J) na rodada do pedido 347.

## 1. O que aconteceu

O papel SEC entregou sete achados sobre o alcance da coluna marcada. O mais
grave — o **A1** — dizia que a marca `.tx` do `COMMIT` grava a linha inteira em
claro, a antiga inclusive, fora da cifra e em `0644`. Fui conferir no fonte,
como manda a integração, e **cada parte técnica bateu**:

| o que o parecer afirmou | o que eu medi |
|---|---|
| `transacao.rs` não menciona cofre/cifra/selar | `grep -c -E "cofre\|cifra\|selar"` = **0** |
| `Str`/`Memo`/`Bin` vão com o conteúdo embutido | `transacao.rs:765-767`, confirmado |
| a linha antiga vem do disco **decifrada** | `:241-246`, e o comentário diz isso |
| `File::create` sem `mode(0o600)` | `:959`, confirmado |
| durável | `sync_all` em `:963`, confirmado |

E então, na última frase do achado, veio o rótulo: *«o `.tx` não está na tabela
de extensões do `FORMATO.md:19-28` (**a que tem a coluna «cifrado»**)»*.

Fui olhar a tabela. O cabeçalho dela é:

```
| Arquivo | Papel | Assinatura | Pagina? | Quem lê |
```

**Não existe coluna «cifrado».** A quarta coluna é `Pagina?` — se o arquivo é
paginado. O `.fts` com «não» ali não quer dizer «não cifrado»; quer dizer «não
paginado».

## 2. O que eu concluí primeiro, e estava errado

Concluí que o trabalho de integração era **conferir o achado**: o caminho de
código, a linha, o `grep`. Nisso eu fui rigoroso e nisso o parecer estava
inteiro — sete achados, sete caminhos que existem.

O que eu ia fazer com a última frase era **copiá-la**, porque ela não parecia
uma afirmação: parecia a moldura do achado. E a moldura é justamente o pedaço
que eu não ia medir, por três motivos que se somam:

1. **Ela fala de um documento, não de código.** Documento não compila.
2. **Ela fortalecia o achado**, e achado forte convence o leitor de ser
   verdadeiro por inteiro.
3. **Ela era verificável com um comando** — `sed -n '17,18p'` —, e é exatamente
   por ser barata que ninguém a paga: o barato parece dispensável.

Se eu tivesse copiado, a §11.3 do `SEGURANCA.md` passaria a mandar o leitor
procurar uma coluna que não existe. E o erro sobreviveria bem, porque o achado
em volta dele é verdadeiro — *o errado sobrevive melhor quando o conserto
funcionou por outro motivo*, e aqui o conserto funcionaria: o `.tx` **está**
mesmo fora do inventário.

## 3. O que a medição disse

- **7 achados** no parecer do SEC; **7 caminhos de código confirmados** por
  mim, um a um, com linha.
- **1 rótulo errado**, e era sobre a única afirmação do parecer que falava de
  um documento em vez de código.
- A tabela do `FORMATO.md` tem **5 colunas** e **nenhuma** se chama «cifrado»:
  `Arquivo`, `Papel`, `Assinatura`, `Pagina?`, `Quem lê`. O achado de verdade,
  medido, é **maior** que o do parecer: *nenhuma das duas listas que servem de
  inventário diz quem é cifrado* — nem o `FORMATO.md` (que diz quem é paginado)
  nem a §11.3 (que diz o que vaza). É isso que o pedido 354 registra hoje.
- E não foi o único: o parecer do QA afirmou que a §17 do `provar-guardas.py`
  copia «5 MB», e o medido é **36 MiB** — só que ali **ele mesmo mediu** e
  disse os dois números. A diferença entre os dois casos é o que este arquivo
  ensina: o QA mediu o que era barato de medir; o SEC descreveu o que era
  barato de descrever.

## 4. A regra

> **Confira também a frase que só emoldura o achado — e comece por ela quando
> ela fala de um documento.** Neste repositório todo número tem gerador e toda
> guarda tem catraca; uma afirmação sobre a *forma de um documento* é a única
> que não tem conferidor nenhum. **O que não compila é o que passa.**

E o corolário para quem orquestra: **o parecer de especialista se integra por
partes, não por peças.** Aceitar o achado não é aceitar o parágrafo, e a parte
que fortalece o argumento é a que menos se mediu — porque quem escreveu também
não precisou medi-la para acreditar nela.

## 5. Como está guardado hoje

**Guardado em dois lugares, e nenhum é uma guarda automática — isto é um
buraco, e ele aparece como buraco:**

- O pedido **354** carrega o achado com o rótulo **corrigido**, e diz o que
  medi: a quarta coluna é `Pagina?`, e a ausência de coluna «cifrado» é achado
  próprio, não erro de citação.
- A **§11.3** do `SEGURANCA.md` ganhou as seis representações que faltavam
  (`.fts`, `.tx`, o hash do ledger, o `perfil.txt`, o `.lgpd`, o primeiro
  caractere pelo `rowid`), e o conselho antigo — «tire o índice da coluna
  sensível» — ganhou a segunda metade que faltava: tirar a árvore não tira o
  índice de texto.

**Onde o buraco ficou:** não há conferidor que pegue uma citação errada de
estrutura de documento — «a coluna X da tabela Y», «a §N diz». O conferidor de
números existe (catorze geradores), o de textos cravados existe, o de grades
existe. Este não, e eu **não** o proponho: um casador de citações reprovaria as
boas junto com as ruins, que é o mesmo motivo pelo qual o conferidor de erro
cru está **recusado com número** (8 interpolações, 2 defeitos). O que segura
esta classe hoje é o integrador ler a frase da moldura — e este arquivo existe
para que a próxima integração saiba que essa frase é a de mais risco, não a de
menos.
