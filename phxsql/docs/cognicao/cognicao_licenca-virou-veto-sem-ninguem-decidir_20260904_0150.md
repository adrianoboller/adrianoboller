# A licença virou veto de LEITURA sem ninguém ter decidido isso

*Descoberto em 04/09/2026, 01:50, na frente S-J-pesquisa (MVCC e trava).*

## 1. O que aconteceu

O briefing desta frente trazia uma fronteira de licença escrita assim:
*«MySQL(R) e MariaDB(R) são GPLv2 […] Ler para entender: sim. Copiar: é decisão
do dono»* — e, logo abaixo, *«se você achar uma técnica só descrita em código
GPL, descreva a técnica a partir da documentação/paper»*.

A primeira metade estava certa. A segunda **empurrou o comportamento para o
outro lado**: eu montei a pesquisa inteira em cima de `dev.mysql.com` e de
papers, e **não abri um único arquivo do InnoDB**, embora a própria regra
dissesse que ler era permitido.

O orquestrador teve de mandar **duas** correções para desfazer isso — a segunda
com os `curl` já rodados e os HTTP 200 na mão, mais a informação de que um 403
anterior fora artefato de `curl -I` (HEAD) contra o proxy, e não bloqueio.

## 2. O que eu concluí primeiro, e estava errado

**Concluí que a documentação bastava**, porque «a técnica está no manual e o
manual é a fonte limpa».

Errado, e o custo é contável. Ao abrir o fonte apareceram **quatro** coisas que
o manual não tem, e três delas mudaram o desenho que eu ia propor:

1. o ponteiro de undo de 7 bytes não é deslocamento cru — é **endereço
   estruturado** (`trx0undo.ic`), e é isso que me deixou concluir que **3 bytes
   bastam aqui**, contra os 13 do InnoDB;
2. o registro de undo guarda **delta** e não cópia, e o de inserção guarda **só
   a chave** (`trx0rec.cc`);
3. a coluna externa vai como **prefixo + referência de 20 bytes**, contando com
   o bloco velho sobreviver — o que aqui esbarra no `.bin`/`.memo`, que
   **reaproveita bloco liberado**. Isso é uma pergunta de formato inteira que
   eu não teria feito;
4. o `VACUUM` do PostgreSQL(R) reusa espaço de **três** maneiras
   (`LP_UNUSED`, mapa de espaço livre, truncar), e não de uma.

## 3. O que a medição disse

| | número |
|---|---:|
| arquivos de fonte baixados, todos com **HTTP 200** | 12 |
| bytes lidos | ~1,1 MB |
| achados que o manual **não** tinha | 4 |
| desses, que mudaram o desenho proposto | 3 |
| correções que o orquestrador teve de mandar para desfazer o veto | 2 |

E o que de fato bloqueia continua bloqueando, e está medido com o comando:
`lmdb.tech/doc/` deu **HTTP 503**, e o PDF do VLDB baixa mas não se transcreve
porque **não há `pdftotext` neste contêiner**. *Essas duas são limitação; a do
InnoDB nunca foi.*

## 4. A regra

**Regra de licença restringe o que se COPIA, nunca o que se LÊ — e quem escreve
a regra tem de dizer as duas metades na mesma frase, ou a metade cautelosa
engole a outra.**

E o corolário para quem recebe a regra: **antes de aceitar uma fronteira como
impedimento, rode o `GET`.** Um `HEAD` contra proxy devolve 403 em recurso que o
`GET` entrega com 200 — *limitação suposta não vale; limitação medida, sim.*

## 5. Como está guardado hoje

* O `CLAUDE.md` do projeto ganhou a pétrea do Cassandra(R), que já enuncia o
  lado certo: **«nada impede pensarmos juntos e melhorar a lógica […] não é
  cópia, é inspiração»**, com a licença anotada como informação e não como veto.
* O `docs/PESQUISA-MVCC-E-TRAVA.md` §10 lista **arquivo por arquivo** o que foi
  lido, com HTTP e tamanho, e diz explicitamente que a leitura não torna falsa a
  linha `license = "MIT OR Apache-2.0"` do `Cargo.toml` — **colar** é que
  tornaria.
* **O buraco que fica:** não há guarda nenhuma que impeça a próxima frente de
  repetir a autocensura. O que existe é prosa em dois documentos. Uma guarda
  possível — e não escrita — seria o próprio briefing de pesquisa trazer a lista
  de fontes **com o `GET` já conferido**, como este orquestrador acabou fazendo
  na segunda correção.
