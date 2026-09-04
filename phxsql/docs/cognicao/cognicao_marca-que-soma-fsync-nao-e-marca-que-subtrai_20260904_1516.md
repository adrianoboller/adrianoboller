# A mesma marca, lida nos dois sentidos, tem modos de falha opostos

*Descoberto em 04/09/2026, 15:16, consertando o `fsync` que faltava no fecho da
janela de durabilidade.*

## 1. O que aconteceu

`Volumes::sincronizar` (`crates/phxsql-store/src/volume.rs`) percorria
`self.abertos` — o cache LRU de descritores **desta instância** — e chamava
`sync_all()` em cada um. Isso mede o cache, não a durabilidade, e as duas coisas
divergem sempre que **quem escreve e quem sincroniza são objetos diferentes**.

São três caminhos no servidor, e todos reabrem a tabela só para sincronizá-la:
`descarregar_sujas_com` (o fecho da janela), `sincronizar_replicada` (uma vez por
alcance de replicação) e o `BULKINSERT(false)` (o `fsync` que a carga inteira
adiou). Nos três, o `.reg` da instância nova tinha o cache vazio — `RegFile::abrir`
lê o cabeçalho com um `std::fs::File` direto, porque a largura do sufixo de volume
mora **dentro** do cabeçalho e o conjunto não pode ser montado antes. O laço rodava
zero vezes e `sincronizar()` devolvia `Ok(())` **tendo sincronizado nada**, com o
`.ndx` indo ao disco duas vezes no mesmo fecho: índice durável apontando para dado
que não foi.

Nenhum teste via, e não por descuido: a bateria de durabilidade prova com
`SIGKILL`, e página suja no cache do núcleo **sobrevive a processo morto**. Só uma
queda de energia mostraria.

## 2. O que eu concluí primeiro, e estava errado

**Errei duas vezes, e as duas de um jeito que teria compilado e passado.**

**Erro 1 — «o defeito é do `.reg`».** Era o sintoma. O defeito é do contrato:
`sincronizar` prometia durabilidade e entregava «`fsync` no que este objeto
abriu». O `.reg` era só o componente onde a diferença aparecia, porque é o único
cujo `abrir` não passa pelo `Volumes`. Consertar só ele deixaria de pé outros dois
buracos do mesmo contrato — o volume do **meio** de uma tabela paginada, que um
`atualizar` suja e que a reabertura não abre, e o despejo do **LRU de 64
posições**, que joga fora o descritor de quem escreveu antes de o `fsync` chegar
nele.

**Erro 2, o pior — «o registro de sujeira também serve para PULAR `fsync`».** Ao
implementar o registro que resolve o erro 1, a economia parecia sair de graça: se
eu já sei quem foi escrito, sei quem **não** foi, e quatro dos oito arquivos do
fecho (`.trash`, `.reason`, `.bin`, `.memo`) um `inserir` comum nunca escreve. Uma
pesquisa do papel J já recomendava exatamente isso, com PostgreSQL e InnoDB no
fonte, e na forma «um campo dentro do próprio `Table`».

O que me tirou disso foi ler o `lixeira.rs`: com `recursos.exclusao_na_janela`
ligado, o `fsync` do `.trash` **sai de dentro do `excluir`** e passa a ser o do
fecho da janela. Quem escreveu o `.trash` é uma `Table` que já morreu; quem fecha
a janela é outra, que nunca escreveu nele — e o sinalizador **por instância** dela
está limpo. Ela pularia. Hoje o `.trash` vai ao disco *por acidente*, porque
`abrir` o abriu para ler o cabeçalho. Ou seja: eu estava prestes a converter
quatro acertos acidentais em quatro defeitos novos, e o pior deles inverte
justamente a ordem que `Table::sincronizar` existe para garantir — a cópia de
recuperação no disco **antes** da liberação do slot contra a qual ela protege.

## 3. O que a medição disse

| o que | número |
|---|---|
| `fsync` por fecho, antes | **7** (`.trash .bin .memo .log .reason .ndx .ndx`) |
| `fsync` por fecho, hoje | **8** — o oitavo é o `.reg` |
| custo do conserto, cronometrado (K=16, 10 rodadas intercaladas) | **não aparece**: mediana 22,3 ms antes contra 21,6 ms depois, com espalhamento de ±3 ms |
| custo do registro no caminho quente (`custo-do-fsync 50000`, 5 corridas de cada lado) | **zero medido**: 11,88 contra 11,78 µs/linha |
| `fsync` num arquivo **limpo**, nesta máquina | **52–54 µs** |
| `fsync` num arquivo com página **suja** | **139 µs** |
| o que o `fsync` seletivo compraria, por **ablação** (4 em vez de 8) | **17%** — 21,6 ms para 17,9 ms, e **não 2×** |
| previsão pela conta por arquivo (4 × 54 µs × 16 tabelas) | 3,5 ms — a ablação mediu **3,7 ms** |

As duas últimas linhas são o par que dá confiança: a conta por arquivo e a
ablação, medidas por caminhos independentes, concordam. O «~2× mais barato» que a
proposta trazia vinha da **contagem** (8 → 4) e morreu no relógio, porque os
quatro que se cortariam são justamente os quatro mais baratos.

## 4. A regra

**Marca que SOMA trabalho é segura quando esquecida; marca que SUBTRAI trabalho
custa o dado quando esquecida. Antes de usar um registro de sujeira, pergunte em
que sentido ele vai ser lido — e se o esquecimento degrada para o comportamento
antigo ou para a perda silenciosa.**

E o corolário, que é o alcance de uma pétrea que já existe: *conserto entra no
caminho que o motivou, e o caminho irmão fica.* Aqui os irmãos eram três
(`descarregar_sujas_com`, `sincronizar_replicada`, `BULKINSERT`) e nenhum deles
precisou ser tocado — porque o conserto entrou no lado da **escrita**, que é
comum aos três. Quando os irmãos são muitos, procure a função que todos chamam
antes de consertar cada um.

## 5. Como está guardado hoje

* **O registro**: `ESCRITAS_PENDENTES` em `crates/phxsql-store/src/volume.rs`, do
  processo e não da instância, alimentado só pelos caminhos de escrita do
  `Volumes` e lido só para **acrescentar** `fsync`. O comentário do `static`
  carrega a assimetria por escrito, para que a próxima leitura não a inverta.
* **A prova real, nos dois sentidos**: três testes de unidade em `volume.rs`
  (`o_fecho_alcanca_o_que_outra_instancia_escreveu`,
  `o_fecho_alcanca_volume_do_meio_de_tabela_paginada`,
  `o_despejo_do_lru_nao_leva_o_fsync_junto`) — os três reprovam com a chamada a
  `marcar_escrito` apagada do `escrever`. Eles só provam alguma coisa porque a
  semeadura **sincroniza antes** da escrita que interessa: na primeira versão a
  marca que o `criar` deixava fazia dois deles passarem com o defeito reposto.
* **O contador que torna isso possível sem `strace`**: `Volumes::sincronizados()`
  conta o **arquivo**, e não a chamada. O `sincronizacoes` e o `selo` que já
  existiam medem a INTENÇÃO — os dois passariam com o defeito de pé, e foi por
  isso que a guarda que achou o buraco precisou de `strace`.
* **A guarda que atravessa processo**: `tests/fecho-da-janela-sincroniza-o-reg.rs`
  (com `strace`), verde desde este conserto e vermelha com ele desfeito.
* **A recusa medida**: `docs/DESEMPENHO.md` §16.2, com a tabela da ablação e o que
  faria a proposta voltar.
* **O buraco que FICOU, e é decisão**: escreveu no meio de uma tabela paginada,
  morreu o **processo** sem sincronizar, e **outro** processo fechou a janela.
  Fechá-lo pediria `fsync` em todo volume existente a cada fecho — 52 µs cada,
  13 ms numa tabela de 256 volumes. Está escrito em `docs/FORMATO.md` §8 e no
  comentário de `RegFile::sincronizar`, com o número ao lado.
