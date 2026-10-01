# Parecer do DBA (papel C): formato em disco e garantias de dado, 30/09 a 01/10/2026, antes de selar a 0.19.0

Escopo: o que entrou ou está pela metade entre 30/09 e 01/10 e mexe em byte gravado ou em
garantia de dado. Método: leitura do `docs/FORMATO.md`, do código na `HEAD` (`f797027d`) e das
cópias de trabalho das frentes em curso (`.claude/worktrees/*`, só `git diff`). **Nada foi
rodado**: o que está escrito como «raciocinado» não tem medida, e diz isso.

Estado de integração medido por `grep` em `crates/`:

| item | na `HEAD`? | onde |
|---|---|---|
| 344 `EXTERNO_SELADO` | sim | `phxsql-store/src/table.rs:640`, `:6582-6623`, `:6711-6756` |
| 329 `numero_servidor`, `replicacao-numeros.json` | **não** (0 ocorrências, nem nas cópias de trabalho) | decisão em `decisoes-onda-01-10-2026.md` §329 |
| 331 rabo do «antes» / `chave_colunas` | **não** (0 ocorrências) | decisão §331 e lacuna 4 |
| 564 `PoliticaDoDiario` | sim | `phxsql-store/src/catalogo.rs:290-389` |
| 586/589/591/595 `PorSincronizar` | sim | `catalogo.rs:1511-1600`, `:1070-1146` |
| 513 retrato sob trava de leitura | **não** (cópia `agent-a9ff61e71bc9fa754`, `retrato.rs` novo) | |
| 534/535 | sim | `config.rs:2916` (`gravar_privado`), `REPLICACAO.md` §«A posição», `CLUSTER.md` |
| 290 faixa da `Sequence` | parcial na `HEAD`; conserto em `agent-aa1a8391124d9ca72` | `reg.rs:502-525`, `:800-832` |
| 601 identidade de nó no carimbo | **não** | `PENDENCIAS.md` #601, cognição `..._20261001_1100.md` |

---

## 1. 344 — bit `EXTERNO_SELADO` (0x8000) na imagem

**Garantia.** A origem diz se o externo da imagem está selado; quem recebe deixa de adivinhar
pelo próprio cofre. No fio a imagem sai aberta (`imagem_para_o_fio`), bit apagado; no `.log` e
na lixeira continua selada, bit aceso.

**Queda no meio.** Nada novo: o bit está dentro da imagem, a imagem dentro do CRC do evento.
Evento rasgado já é cortado pela cura do rabo do `.log`.

**Compatibilidade — dois NÃO.**

- **NÃO 344-a: o diário anterior ao 344 replica texto cifrado como dado, calado.** Antes do 344
  o `imagem_da_linha` já guardava o conteúdo **selado** do `.memo`/`.bin` de coluna marcada
  (o diff do merge `a27e18ae` só acrescenta o `externos_da_imagem`; o conteúdo continua o
  mesmo) — e **sem** o bit. Hoje `abrir_imagem_com_selo` lê bit ausente como «aberto»
  (`table.rs:6748-6749`), `imagem_para_o_fio` passa os bytes adiante, e a réplica grava o
  cifrado como se fosse o anexo. **Cenário:** source 0.18 com tabela cifrada e memo marcado,
  sobe para 0.19; réplica nova (ou atrasada, ou que perdeu a posição) puxa desde o evento 0 →
  os eventos pré-344 entram como lixo cifrado, `ok`. É exatamente o 344-1, pelo passado. O
  `FORMATO.md:1722-1723` justifica com «sem dado em produção»; isso vale para **produção**, não
  para os bancos de demonstração e de teste que já existem. **Pedido:** na origem, bit ausente
  + `.reg` cifrado + coluna externa marcada + conteúdo não vazio ⇒ **recusa** nomeando o
  evento («gravado antes do 344, não replicável selado»), em vez de mandar. Falhar fechado
  custa zero byte; mandar lixo custa dado.
- **NÃO 344-b: a premissa do bit livre está errada no próprio documento.** O comentário
  (`table.rs:638`) diz «coluna acima de 32767 não existe»; o `Schema::do_disco`
  (`phxsql-core/src/schema.rs:1058`) aceita até `u16::MAX`, e o `FORMATO.md` §14 publica
  **65 535 colunas por tabela**. Tabela com ≥ 32 768 colunas e externo depois da 32 767 teria
  a coluna lida como `c − 32768`, selada. Ninguém tem tabela assim hoje — e é por isso que o
  teto **tem de baixar para 32 767 antes de selar**: depois da 0.19.0, baixar o teto é recusar
  banco que abria.

**Ordem de digitação / regra primordial:** não toca.

**Preço dito no commit, que o DBA não assina:** «réplica sem cofre guarda em claro». A garantia
«coluna marcada é cifrada em repouso» **não vale na réplica sem cofre**. Cenário: réplica de
leitura numa filial sem cofre configurado → CPF/memo marcado em claro no `.memo` dela. Não é
formato e não bloqueia a 0.19.0, mas a frase do produto não pode dizer «dado pessoal cifrado
em repouso» sem o asterisco «na réplica, só se ela tiver cofre». O caminho que o DBA prefere,
para o pesquisador/dono pesar: réplica sem cofre **recusa** tabela com coluna marcada, salvo
pedido explícito — guarda nova pedida ao contrário, porque aqui o padrão inseguro é o calado.

## 2. 331 — o rabo do «antes» na imagem (troca de chave no bidirecional)

**Não integrado** (0 ocorrências). Hoje a alteração que muda a chave vira inserção do outro
lado (lacuna 4 da decisão), e a linha velha fica viva lá: **dado errado ativo**, sem pedido.

**O defeito de formato que já existe, e que decide como o rabo pode entrar:** o decodificador
`abrir_imagem_com_selo` **não confere se sobrou byte** depois dos externos (`table.rs:6727-6757`
devolve sem olhar `i == imagem.len()`). Consequência: um rabo novo acrescentado depois dos
externos é **ignorado calado** por todo binário anterior — o par 0.19↔0.20 diverge sem erro, o
pior jeito de introduzir campo.

**Parecer:**

1. **Antes de selar a 0.19.0**, o decodificador passa a **recusar sobra** («imagem com campo
   que este binário não conhece — atualize a réplica»). Zero byte; muda só a leitura. Com isso
   a 0.19.0 falha **fechada** diante do rabo que vier depois.
2. Quando o rabo entrar, que tenha **marca própria** (`[0xA5 'antes'][tam u32][payload]` — só o
   payload; chave não é `Bin`/`Memo`) e que o `posicao` anuncie a capacidade: a origem não manda
   evento de troca de chave a quem não a anuncia, e para dizendo por quê.
3. Antes, por si só, não precisa de `EXTERNO_SELADO` (chave não mora em externo); se um dia
   morar, o bit vale igual.

Compatibilidade do `.log` já gravado: evento sem rabo = alteração sem troca de chave ou anterior
ao 331 — ausência benigna, sem subir versão do `.log`.

## 3. 329 — `numero_servidor` e `replicacao-numeros.json`

**Não integrado.** Zero formato no `.reg`, mas **não é zero formato no dado**: o número vai para
o campo `origem u16` de **todo** evento aplicado no `.log` (`log.rs:150`, `:181`, `:336`), e o
`.log` é passado — não se reescreve.

**Garantia que tem de valer (condições do DBA):**

- **Um número nunca é reatribuído a outro id, nem depois de o id sair** — a ordem de digitação
  aplicada ao espaço de números: o `.log` dos parceiros guarda o número para sempre, e
  reatribuí-lo faz eventos velhos mudarem de dono. O mapa é **id → lista de números já usados**,
  não um número só (o nó que passa de `hash_id` para `numero_servidor` tem dois na vida).
- **O mapa vai ao disco ANTES de o primeiro evento daquele número ser aceito** — a mesma ordem
  do 535 ao contrário: perder o mapa e aceitar outro id com o mesmo número grava a ambiguidade
  dentro do `.log`, onde nenhuma recuperação a desfaz. Troca durável (`gravar_privado`).
- Arquivo ilegível ⇒ **recusa** o par (como o 534 fez com o cluster), nunca «mapa vazio»: vazio
  aceita colisão.

**Queda no meio:** com a ordem acima, a queda perde no máximo um par ainda não aceito.

**Compatibilidade:** sem `numero_servidor`, `hash_id(id)` como hoje (guarda pedida). OK.

**Entra antes da 0.19.0?** Sim, pelo motivo acima: cada evento aplicado com número sorteado é um
`origem` que nunca mais muda. Quanto mais tarde, mais `.log` com origem que pode colidir
(0,31996% a 21 nós, número da decisão).

## 4. 564 — `PoliticaDoDiario` num lugar só

**Garantia:** a imagem no diário é decisão do servidor; toda abertura pelo `Database` a herda,
inclusive a recuperação de marcas. **OK.**

**Queda no meio:** a recuperação (`recuperar_marcas`) completa o evento com a política da
instância — certo no servidor.

**Compatibilidade:** é configuração, não formato. O `.log` já mistura eventos com e sem imagem
pelo bit 7 da operação; nada sobe versão. OK.

**NÃO (601, primeira metade, defeito ativo):** o embutido abre `Instancia::nova` com a política
padrão (`phxsql-ffi/src/lib.rs:401`, `:419`) e recupera as marcas **sem imagem**, mesmo quando a
aplicação ligou `phx_imagem_no_diario`. Cenário: aplicação embutida que replica, morre no meio do
COMMIT, reabre → evento completado sem imagem → réplica para em «veio sem imagem». O conserto
existe (`com_politica_do_diario`, `catalogo.rs:382`) e não é usado pelo FFI. Não é formato; é
pequeno; deve entrar na 0.19.0 porque é dado que não chega.

**Documento em contradição:** `FORMATO.md:1725` ainda diz «Exclusão não leva imagem: o rowid
basta» — desde o 416/564 leva, pela política. Quem escrever leitor do `.log` pelo documento erra.

## 5. 586/589/591/595 — `PorSincronizar`, `levar_ao_disco` e a ordem dos `fsync`

**Garantia:** criar/copiar/excluir só respondem «ok» depois de: cada arquivo no descritor que o
escreveu → a pasta onde nasceram/saíram → a mãe de cada pasta nova, da mais funda para a mais
rasa (`catalogo.rs:1563-1600`). É a ordem do `durable_rename`. Excluir leva ao disco **mesmo no
erro do meio** (`catalogo.rs:1072-1076`). Regra primordial: `excluir_tabela` recusa o pai com
filha antes de apagar qualquer arquivo (`catalogo.rs:1114-1120`). **OK** para quem chamou.

**Queda no meio:** antes do «ok», qualquer subconjunto dos nomes pode voltar — inclusive o
`.reg` sem o `.ndx` (`EXTENSOES_TODAS` apaga `.reg` primeiro, mas sem `fsync` entre um e outro
nada garante que a pasta persista nessa ordem). O cliente não ouviu «ok», então não é promessa
quebrada; é a «tabela pela metade» que o arranque precisa saber tratar (não verifiquei se trata).

**NÃO 5-a (raciocinado, não medido): o «ok» de um TERCEIRO dentro da janela.** O comentário diz
«a tabela já é visível, mas ninguém ouviu ok por ela ainda» (`catalogo.rs:1500-1501`). Não é
verdade para quem **escreve** nela: A cria `T` e solta a trava; antes de o `fsync` da pasta de
A terminar, B toma a trava, insere em `T`, o commit de B faz `fsync` dos arquivos (o
`Volumes::sincronizar` não sincroniza pasta — `volume.rs:1053-1095`) e B ouve «ok». Queda →
a entrada de `T` some e leva a linha de B, que ouviu «ok». Mesma forma com database novo: o
`criar_tabela` de B não sincroniza a entrada do database de A na base. A janela é a duração do
`fsync` de A (milissegundos), e a réplica que cria e aplica em seguida é quem mais cai nela.
**Conserto que o DBA pede (sem formato):** a pasta que deve `fsync` entra num registro do
processo (o irmão do `ESCRITAS_PENDENTES`), e todo `sincronizar` de família dentro dela paga o
`fsync` da pasta antes de responder. Não bloqueia selar o formato; está na conta como garantia
que não vale.

**Ordem de digitação no copiar (586):** a cópia leva os slots byte a byte, mortos inclusive —
rowids iguais aos da origem. **OK.** Ressalva para o item 9: a cópia leva o bloco `PSCH` também
byte a byte.

## 6. 513 — retrato do backup sob a trava de leitura

**Não integrado** (cópia `agent-a9ff61e71bc9fa754`). Passo 1: a cópia toma a ficha
**compartilhada** e o escritor espera num portão antes da fila do `RwLock`.

**Garantia:** «nenhuma escrita durante a cópia» continua valendo: todo escritor de `raiz` passa
pela ficha exclusiva, e o leitor que precisa escrever (cura do cabeçalho do `.log`, trilha
`.lgpd`, as três razões do `abrir_para_ler_travada`) **solta a compartilhada e pede a
exclusiva** — e então espera a cópia. A prova que a decisão exigia («nada grava sob a trava de
leitura») está no desenho: a pista de leitura é `Legivel` e recusa tabela com dado pessoal
(`servidor.rs:22006-22030`). **OK para o passo 1.** O que escreve em `raiz` fora da trava
(`cluster.estado.json`, posições, configs) já escrevia fora dela com a exclusiva — nada piora.

**Queda no meio:** o backup é cópia em pasta/zip com manifesto no fim (524/552/579); a queda
deixa backup sem manifesto, que a restauração recusa. OK.

**Condições do DBA para o passo 2 (H3, cópia sem trava + acerto):** não aceito inferir «o que
mudou» só pelo `eventos()` do `.log`. Há escrita no `.reg` **sem evento**: contadores do
cabeçalho (`proxima_sequencia`, `marcadas`, `ultimo_carimbo`, `proximo_rownum`), `ajustar_sequencia`,
cura de cabeçalho. A fase 2 tem de recopiar o **cabeçalho de todo volume tocado** e usar como
fonte o registro de escritas do processo (`ESCRITAS_PENDENTES`), não o diário; e tem de levar as
marcas `.tx` de transação sob a exclusiva, senão o retrato pega metade de um commit entre
tabelas. Sem isso, NÃO.

## 7. 534 — estado do cluster por troca durável

**Garantia:** a época e o papel vivo vão ao disco por temporário + `fsync` + `rename` + `fsync`
da pasta; o `promover` grava **antes** de liberar a escrita; arquivo ilegível nasce réplica sem
escrita. **OK** — é a ordem certa: nunca há master que o disco não conheça.

**Queda no meio:** antes do `rename`, volta o estado anterior (réplica); depois, o novo. Os dois
são seguros. Ressalva: arquivo **ausente** cai no papel do `config.json`; um master destronado
cujo `cluster.estado.json` sumiu por fora (restauração de backup antigo, por exemplo) volta
mandando. A época do cluster resolve o choque, mas a janela existe; anotar no manual de
restauração.

## 8. 535 — a posição do bidirecional depois do dado

**Garantia:** `fsync` da tabela, **depois** a posição pela troca durável, uma vez por alcance,
fora da trava global; o mapa compartilhado só recebe a posição já durável. A posição nunca passa
o dado — o comportamento em que os três maduros convergem. **OK.**

**Queda no meio:** posição atrás do dado ⇒ releitura; a aplicação por chave com «mais recente
vence» é idempotente. A premissa «reaplicar perde para o toque igual» depende do mapa de toques
reconstruído do diário local (330) — se o 330(b) tirar o mapa da RAM para colunas de sistema,
este argumento tem de ser refeito. **Não medido** contra queda de energia (o próprio commit diz:
SIGKILL não perde o cache).

**Ordem de digitação:** cada servidor mantém a sua; o bidirecional não casa por rowid. OK.

## 9. 601 — identidade de nó no carimbo (decisão parada; ENTRA ANTES da 0.19.0)

**O buraco, medido na cognição de 01/10 11:00:** o `rowstamp` é contador do processo; dois
`phxsqld` recém-nascidos emitem o mesmo `1`; a conferência da exclusão/alteração replicada
(`conferir_identidade`) vê `1 == 1` e apaga/sobrescreve a linha errada com `aplicados: 1`.
**A garantia «a réplica para antes de apagar a linha de outra origem» não vale** entre
servidores com histórias iguais (dois caixas que nasceram no mesmo dia e gravaram o mesmo número
de linhas).

**Hipóteses, escritas antes de decidir:**

| | hipótese | formato | veredito |
|---|---|---|---|
| H1 | empacotar o nó nos 16 bits altos do `rowstamp` | 0 byte | **morta**: nó ≥ 32 leva o valor acima de 2⁵³ e o fio JSON (`f64`) arredonda — o mesmo motivo medido que recusou o carimbo em ns (`FORMATO.md` §«As duas colunas»); e a réplica, que «empurra o contador local para o que chegou», adotaria o prefixo do outro nó |
| H2 | conferir o par `(rowstamp, rowtime)` | 0 byte | **morta como conserto**: estreita, não fecha — dois caixas no mesmo ms empatam (964 de 1.000 `insert` caem no mesmo ms aqui). Pode ficar como reforço |
| H3 | coluna de sistema `roworigem u16` por linha (PSCH v11) | +2 B por linha, para sempre; pode empurrar o bitmap | **vence só se** uma tabela pudesse misturar origens **por rowid** — e no modo A ela não pode: rowid de duas origens no mesmo `.reg` já é divergência |
| H4 | **linhagem da tabela**: UUID v7 de nascimento no bloco `PSCH` (v11) | +16 B por **tabela**, 0 por linha | **vence** |

**Por que H4.** No modo A (réplica fiel, por rowid) a pergunta certa não é «de que nó é esta
linha?», é «este evento é da mesma **história** desta tabela?». E o `PSCH` é exatamente o que
viaja **byte a byte** para a réplica (`abrir_para_replicar`, decisão do dono no 290): um UUID
gravado no nascimento da tabela chega igual em toda réplica dela — e **sobrevive ao failover**,
porque o novo master tem o mesmo `PSCH` —, enquanto duas origens que criaram «clientes» cada uma
por conta têm UUIDs diferentes. A conferência deixa de depender de sorte de contador: a réplica
compara a linhagem que já recebe no `posicao` (`com_esquema`, `REPLICACAO.md` §6) com a dela, e
recusa **no primeiro evento**, inclusive inserção — que a conferência por `rowstamp` nunca pega.
No bidirecional a identidade continua sendo a chave (e o par `(carimbo, origem)` do `.log`), e a
linhagem não se confere ali — os 20 caixas de um central legitimamente divergem nela.
É a decisão do J de 23/09 levada ao dado: identidade entre nós por **par** (`(SID,GNO)`,
`(domain_id,server_id,seq_no)`, `(roident,LSN)`), aqui `(linhagem, rowid)`.

**Formato:** `PSCH` v11, no **fim** do bloco (molde da v10): `[tem u8][uuid 16]`. Ausente
(v ≤ 10) = «sem linhagem» → conferência como hoje (só `rowstamp`); guarda nova entra pedida.
Truncado no bloco v11 = **erro** (molde da v9/v10: linhagem que some calada desliga a guarda).

**Compatibilidade:** banco existente abre igual (v2..v10 continuam lidos); tabela nova nasce v11.
Binário 0.18 diante de `PSCH` v11 **recusa** com «versão de esquema 11 não suportada»
(`schema.rs:2060-2063`) — réplica velha de source novo para dizendo por quê, não diverge.

**Três obrigações que vêm junto:**

1. **`copiar`/`duplicar` tabela (586) cunha linhagem NOVA** — hoje a cópia leva o `PSCH` byte a
   byte, e cópia com a linhagem da origem seria «a mesma história» para a conferência.
2. **`aplicar` (empurrar) passa a mandar a linhagem**, campo opcional; ausente = sem conferência.
   O 416 prova pelo `aplicar`, e sem o campo o buraco do teste continua.
3. **Restaurar backup preserva a linhagem** (é a mesma história) — já é o comportamento da cópia
   de arquivo; escrever no `FORMATO.md`.

**Por que antes de selar:** é mudança de `PSCH`, e toda tabela criada pela 0.19.0 sem ela nasce
sem linhagem **para sempre** — o UUID de nascimento não se inventa depois (seria a «adivinhação
retroativa» que o 290 já recusou). Hoje o custo é um bump; depois, é um conjunto de tabelas que
nunca terão a guarda.

**Sobe ao dono?** Não: a régua (identidade por par, 3 de 3 maduros) decide, nenhuma pétrea se
opõe, e a ordem de digitação não é tocada — 16 bytes no esquema, zero no `.reg` de dado.

## 10. 290 — `inicio`/`passo` da `Sequence` no bump de `PSCH`

**Estado:** o `passo` está no `PSCH` v10 desde 17/09; o `inicio` sai da identidade do nó. Na
`HEAD`: **zero** chamadores de produção do `inicio` (todo servidor numera na faixa 0) e o
contador gravado é `v + 1`. O conserto está na cópia `agent-aa1a8391124d9ca72`:
`replicacao.inicio_da_sequencia` no config e o contador passa a ser o **próximo número da faixa**.

**NÃO 290-a: na `HEAD`, toda tabela com `passo > 1` deixa de abrir depois da primeira inserção.**
`proxima_sem_andar` grava `v + 1` (`reg.rs:811` na `HEAD`), a abertura seguinte confere
`proxima_sequencia ≡ inicio (mod passo)` (`reg.rs:515`) e recusa. Pelo servidor, que reabre a
tabela por pedido, a segunda inserção cai. O conserto da cópia de trabalho está certo e **tem de
entrar antes da 0.19.0**: ele muda o significado dos bytes 36..44 do cabeçalho do `.reg`.

**NÃO 290-b: a tabela já gravada pelo defeito fica sem saída, e a mensagem manda o operador para
a faixa errada.** Uma tabela com `passo = 20` gravada pela `HEAD` tem contador `21`. Com o
conserto, a abertura recusa e diz «ajuste `replicacao.inicio_da_sequencia` para 1». Obedecer
põe **este** nó a numerar na faixa de outro — a colisão que a faixa existe para impedir. E o
remédio certo (`ajustar_sequencia`) exige abrir a tabela, que a recusa impede. **Pedido:** um
caminho de reparo que recalcula o contador pelo **maior valor gravado** da coluna
(`na_faixa(max + 1)`, lido do índice ou da varredura) — isso não é adivinhação, é o dado — e a
mensagem nomeando as duas causas possíveis (defeito do contador `v + 1` ou nó na faixa errada).
População provável: ~0 (a tabela parava na 2ª inserção), mas banco de teste existe.

**Queda no meio:** inalterada — o contador só vai ao disco no `sincronizar`, e o índice `unico`
é quem recusa a repetição depois de uma queda (documentado no próprio `reg.rs`).

**Ordem de digitação:** não toca; a faixa é sobre o valor da `Sequence`, não sobre o slot.

---

## Lista: o que tem de entrar ANTES de selar a 0.19.0

Formato e significado de byte (depois vira migração):

1. **601 — linhagem da tabela no `PSCH` v11** (H4), com cópia de tabela cunhando linhagem nova
   e `aplicar` levando o campo.
2. **344 — teto de colunas em 32 767** (`schema.rs:1058` e `FORMATO.md` §14), para o bit
   `0x8000` ser livre de verdade.
3. **331 — decodificador da imagem recusa sobra** (`abrir_imagem_com_selo`), para a 0.19.0 falhar
   fechada diante do rabo do «antes» que vier depois. O rabo em si pode vir depois, com marca e
   anúncio no `posicao`.
4. **290 — contador `próximo da faixa` integrado** (bytes 36..44 mudam de significado), com
   caminho de reparo pelo `MAX` e a mensagem corrigida.
5. **329 — `numero_servidor` com mapa durável e número nunca reatribuído**: o número vai para o
   `origem u16` de todo evento do `.log`, que não se reescreve.

Garantia que não vale e deve entrar na mesma versão (sem byte novo):

6. **344-a** — recusa ao mandar evento pré-344 de coluna marcada sem o bit.
7. **601-1** — o FFI recupera as marcas com a política da aplicação (`com_politica_do_diario`).

Pode ficar para depois, na conta, com o motivo escrito:

- **5-a** — `fsync` da pasta devido a um terceiro que escreve na tabela recém-criada (raciocinado).
- **513** passo 1 — OK para integrar; passo 2 só com as condições do §6.
- **534** — anotar no manual de restauração o caso do `cluster.estado.json` ausente.
- `FORMATO.md:1725` — «exclusão não leva imagem» desmentido pelo 416/564.
