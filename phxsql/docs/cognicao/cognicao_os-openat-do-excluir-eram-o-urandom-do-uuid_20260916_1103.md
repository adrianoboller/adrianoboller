# Os «~5 `openat` por exclusão» eram quatro `/dev/urandom` do UUID — e o instrumento mudava o ritmo que mudava a contagem

*Descoberto em 16/09/2026, 11:03, na primeira corrida do `--example
custo-do-excluir` reescrito para o pedido 259.*

## 1. O que aconteceu

A bancada CRUD (pedido 257) viu o excluir custar 24–28 µs sem `fsync` nenhum,
contra 3,7–4,4 do inserir, e o `strace -c` dela contou «8 `write` e ~5
`openat`» por exclusão. O pedido 259 nasceu com dois suspeitos: a lixeira e o
`.reason` abrindo arquivo por linha, ou a busca reversa da integridade
varrendo os esquemas do diretório.

## 2. O que eu concluí primeiro, e estava errado

**Primeiro, antes de medir**, aceitei os dois suspeitos do pedido e esperava
que os 5 `openat` fossem o `read_dir` mais os arquivos que a lixeira e o
`.reason` abririam. Não abrem: os dois escrevem pelo descritor que já está no
cache do `Volumes`.

**Segundo, com o traço na mão**, o leitor de `strace` atribuiu todos os 5
`openat` e os 9 `statx` a «`phxsql`» — o diretório de trabalho —, e por um
momento li isso como «a busca reversa abre o diretório cinco vezes». Era o
leitor: o `-y` decora também o `AT_FDCWD` do `openat` e do `statx` com o
diretório corrente, e essa decoração vem **antes** da aspa com o caminho de
verdade. Contagem certa, arquivo errado. Consertado em
`examples/apoio/strace.rs`: a decoração que segue `AT_FDCWD` não é alvo de
nada.

**Terceiro, com a atribuição certa**: 4 dos 5 `openat` são `/dev/urandom`, e
eu quis explicá-los com «um UUID por arquivo de anexo» — mas a exclusão gera
só dois UUIDs (`.trash` e `.reason`), e o `.log` não gera nenhum. A conta que
fecha é outra: o `sortear` do `phxsql-core/src/uuid.rs` abre o dispositivo a
**cada chamada** — uma pelos 8 bytes do id, e mais uma a cada milissegundo
**novo**, pela semente do contador. Sob `strace` cada exclusão passa de 1 ms,
então toda chamada cai em milissegundo novo: 2 × 2 = 4. Sem traço, a 28 µs
por exclusão, são 2 na maioria das vezes. O «~5» da bancada era em parte
artefato do próprio `strace -c`.

**E quarto, no meu próprio instrumento**: a seção «com 30 irmãs» das chamadas
de sistema saiu idêntica à «sozinha», e demorei a desconfiar. `montar()` apaga
o diretório antes de criar a tabela, e as irmãs tinham sido criadas antes
dele — a sonda rodou sem irmã nenhuma. O tempo por ablação estava certo porque
`medir()` monta de outro jeito. Duas construções do mesmo estado em dois
lugares, e uma delas errada em silêncio.

## 3. O que a medição disse

Máquina parada, N = 200.000, 20.000 exclusões, `strace -f -y` num filho por
diferença 1.000 − 200:

- excluir direto **28,05 µs**, soma das parcelas **99,7%**: busca reversa
  (`read_dir` + 8 `statx`) **8,96 µs, 32%, sem irmã nenhuma**; `.ndx`
  remover 6,87 (24,5%, **4 `write`** por exclusão — página e cabeçalho por
  índice, na hora, sem o write-back que o inserir tem); a linha lida **três
  vezes** 4,34 (15,5%); `.trash` 2,53; `.reason` 2,52; `.reg` 2,18; `.log`
  0,58;
- `openat` 5 = 4 `/dev/urandom` + 1 diretório; `statx` 9 = 8 arquivos da
  tabela + diretório; `write` 11 = `.ndx` 4 + `.reg` 2 + `.trash` 2 +
  `.reason` 2 + `.log` 1;
- com **30 irmãs** sem chave nenhuma: **430,77 µs**, 15,4× — a busca reversa
  abre o `.reg` de cada irmã a cada exclusão, e o excluir não tem o portão
  que o `ao_alterar` tem.

Nada do excluir foi mexido: as duas maiores parcelas batem em desenho (a
pétrea da busca reversa e a árvore do `.ndx`), e o número foi para a mesa
com quatro candidatos ranqueados (`DESEMPENHO.md` §24.5).

## 4. A regra

**Antes de acreditar numa contagem de `strace`, pergunte o que o `strace` fez
com o ritmo — e atribua cada chamada ao arquivo pelo caminho, nunca pela
primeira decoração da linha.**

## 5. Como está guardado hoje

- `crates/phxsql-store/examples/custo-do-excluir.rs` (ablação, chamada
  isolada, `--sonda` sob `strace`) e `examples/apoio/strace.rs` (o leitor,
  com o `AT_FDCWD` tratado e a lição da linha `unfinished` da cognição de
  05/09).
- `docs/DESEMPENHO.md` §24.4–24.5 e `docs/PENDENCIAS.md` #259 (◐).
- **Onde o buraco ficou**: a fonte de entropia do `uuid.rs` continua abrindo
  `/dev/urandom` por chamada; o `cifra::sortear` semeado uma vez existe ao
  lado e não é usado ali. É da frente que está no `uuid.rs`, e está nomeado
  no #259.
