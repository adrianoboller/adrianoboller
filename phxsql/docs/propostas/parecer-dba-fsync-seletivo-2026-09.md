# Parecer do DBA: pular `fsync` de descritor limpo (pedido 258)

Papel C, 16/09/2026, so leitura. Pergunta unica: **existe hoje um sinal
confiavel de que um descritor nao foi escrito desde o ultimo `fsync`?**

**Veredito: pode pular limpo, COM uma condicao nomeada — e o alcance e menor
do que o pedido supunha.**

O processo da descoberta, o erro que veio primeiro e o numero estao na cognicao
`cognicao_sinal-de-sujeira-nao-ve-processo-morto_20260916_1031.md`. Este
documento e a decisao e o desenho.

## 1. O sinal, nomeado

`ESCRITAS_PENDENTES`, `crates/phxsql-store/src/volume.rs:151` — um
`BTreeMap<PathBuf, Arc<Mutex<BTreeSet<u32>>>>` do processo inteiro, com chave =
familia (`diretorio/nome.ext`, absolutizada em `familia()`, :181). Cada
`Volumes` guarda o `Arc` da sua familia em `self.pendentes` (:68).

| pergunta | resposta, com a linha |
|---|---|
| quem marca | `marcar_escrito` (:442), chamado nos cinco caminhos de escrita: `escrever` (:568), `escrever_par` (:594), `definir_tamanho` (:614), `escrever_so_no_principal` (:358), `criar` (:524), e no espelho de cada um |
| quem limpa | so `sincronizar` (:642, `mem::take` antes do `fsync`, devolvendo a lista inteira se qualquer `fsync` falhar, :646) e `apagar_tudo` (:721) |
| quem le sem marcar | todos os caminhos de leitura |
| escrita por baixo, no mesmo inode | **nenhuma** — os escritores fora do `Volumes` sao o `ndx.rs` (arquivo proprio, sincroniza sozinho), o `reescrever_volume`/`escrever_volume_novo` do `reg.rs` (escrevem um `*.novo`, inode diferente, `sync_all` antes do `rename`), o `restaurar.rs` (`File::create` com `sync_all` proprio) e o `pag.rs` (`.pag`, fora dos oito) |
| `mmap` | zero ocorrencias no codigo |
| trava entre processos | zero `flock` e zero lockfile — o motor ja assume **um processo escritor por diretorio** |

O registro e, portanto, **completo dentro do processo**.

## 2. Por que isso nao basta: o caminho irmao, e o chamador que quebra

O fecho de janela decide em **duas camadas**, e so a segunda e compartilhada.

- **Camada 1, servidor** (`servidor.rs:14294-14318` `gravar_de_verdade` ->
  `self.sujas`; `:14387-14487` `descarregar_sujas_com`): granularidade de
  **tabela**. Diz qual tabela mudou desde o ultimo fecho, reabre cada uma num
  `Table` novo e chama `Table::sincronizar` (`table.rs:5108-5122`), que
  sincroniza os oito arquivos na ordem `.trash` -> ... -> `.ndx` -> `.reg`.
- **Camada 2, store** (`volume.rs:662-682` `sincronizar_listas`):
  `alvos = abertos ∪ pendentes-que-existem`. O registro e lido **so para
  somar**, e o comentario :117-125 diz por que.

O `por_operacao` **nao tem camada 1** — a tabela que gravou e a que sincroniza
—, entao a pergunta dele e inteira na camada 2. E a camada 2 e uma funcao so:
mexer em `Volumes::sincronizar` muda de uma vez o fecho, o
`sincronizar_replicada` (`servidor.rs:2759`), o `BULKINSERT(false)` (`:12264`)
e a **recuperacao no arranque** (`transacao.rs:1309`).

O criterio nao diverge do irmao — mas tem de valer para o chamador mais duro,
que e o ultimo. **A restricao nossa:** quem escreve e quem sincroniza sao
objetos diferentes (o servidor abre um `Table` por pedido), e na recuperacao
sao **processos** diferentes. E o que o InnoDB nao tem — o `modification_counter`
dele vive num processo que faz crash-recovery por redo, e nao por heranca de
cache do nucleo. E por isso que o molde dele, copiado, quebra aqui.

O modo de falha concreto esta na secao 2 da cognicao.

## 3. A condicao, e o desenho

> `// Pular descritor limpo SO' depois do primeiro fsync desta familia neste processo: a pagina suja que um processo morto deixou no nucleo nao tem marca em RAM nenhuma, e o primeiro fsync e' o unico que a alcanca.`

Em codigo:

1. Um `batizada: bool` ao lado do `BTreeSet` em `Pendentes` — RAM, por familia,
   por processo.
2. A **primeira** `sincronizar` daquela familia mantem o comportamento de hoje
   (`abertos ∪ pendentes`), e so vira `batizada` se **todos** os `fsync`
   confirmarem.
3. Dai em diante, `alvos = pendentes-que-existem`.
4. No mesmo commit: mover a marca para **antes** do `write`. Hoje ela vem depois
   do `write_all` em todos os caminhos, e um `write_all` que falha no meio
   deixa pagina suja sem marca — inofensivo enquanto `abertos` cobre, furo
   assim que se pula limpo. O unico lugar que entrega descritor de escrita e
   `arquivo(volume, criar=true)` (:457); a marca vai para la, e sai dos
   chamadores.

## 4. Alcance: seis familias. O `.ndx` fica de fora, e por motivo proprio

Vale para `.reg`, `.bin`, `.memo`, `.log`, `.trash` e `.reason`.

O `.ndx` **nao entra**, e estender a ideia a ele pelo campo `sujo` do cabecalho
seria o segundo modo de falha: ele esta fora do `Volumes` e nao tem registro de
processo, e o `fechar()` (`ndx.rs:861-874`, chamado pelo `Drop` :1639 a cada
pedido do servidor) leva as paginas ao nucleo e grava `sujo=0` **sem `fsync`**.
Quem lesse esse byte como «sincronizado» publicaria, numa queda de energia,
`.reg` no prato e `.ndx` sem as chaves com o cabecalho dizendo «limpo» — o
indice atrasado em silencio, que o `FORMATO.md` (l.719-721) chama de o unico
sem conserto. O byte 52 significa «a arvore pode estar incompleta», nao «nao
sincronizado»: sao duas perguntas, e um byte so responde uma.

**O teto medido:** dos oito `fsync` de um inserir, dois sao do `.ndx` (fora do
alcance) e um e do `.log` (diario append por operacao, legitimamente sujo).
Sobra **8 -> 4**, que e o «4 de 8» que o `PESQUISA-FSYNC-SELETIVO.md` §8 ja
contava. Quantos dos **nove** do excluir sao limpos, so o `strace -y` da frente
de engenharia dira — este parecer nao mediu.

**E o aviso de tamanho:** `DESEMPENHO.md` §16.2 mediu que, na ablacao do fecho,
cortar 4 de 8 comprou **17%**, e nao 2x. A recusa daquele paragrafo continua de
pe nos motivos 1 e 2 **para a forma «por instancia»**; o que a forma «por
processo com batismo» acrescenta e exatamente a condicao acima, que faltava la.

## 5. Custo ao formato em disco: nenhum

A condicao vive em RAM. Um sinalizador em disco so compraria pular o
**primeiro** `fsync` de cada familia por vida do processo — um por arquivo por
arranque — ao preco de duas escritas de cabecalho por ciclo sujo, de um
protocolo que o `fechar()` nao pode quebrar (o `.ndx` mostra como se quebra) e
de um byte novo em que `0` teria de significar «pode estar sujo», para banco
antigo continuar seguro. Ha reservados (`.reg` byte 116, oito bytes; `.ndx`
53..123), mas **«mudanca de formato entra cedo» vale quando o numero a
justifica**, e aqui o numero e um `fsync` por arquivo por arranque.

## 6. As guardas que este parecer exige (papeis F e G)

1. **Prova real da sequencia**: escrever numa instancia, matar o registro da
   familia (gancho de teste ou subprocesso, como as sondas ja fazem),
   recuperar, e cobrar `sincronizados() >= 1` no `.reg`. Com o batismo
   **removido** tem de dar **0**.
2. `o_fecho_alcanca_o_que_outra_instancia_escreveu` (`volume.rs:824-859`) cobra
   `gastos + 1` na segunda sincronizacao e passa a cobrar `+ 0`. Isso e
   comportamento documentado mudando, e a mudanca do teste **se anuncia**, com
   o motivo — nao se edita calado.
3. `tests/fecho-em-paralelo-conta-os-mesmos-fsync.rs:82-88` compara com `==` a
   `TETO_FSYNC_POR_FECHO_V2 = 8`. Se o medidor mede o **primeiro** fecho da
   familia no processo, esse numero **nao desce** com esta mudanca; quem desce
   e o segundo fecho. Regua que passa a medir outra coisa **aposenta** a
   catraca e faz nascer outra, no numero medido do dia — catraca nao sobe nem
   se remenda.
