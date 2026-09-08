# Restaurar um backup

Backup que não restaura não é backup. Até a 0.18.0 o PhxSql sabia copiar e
sabia conferir, e a volta era «pare o servidor, apague a raiz de dados e copie
o conteúdo de volta» — um procedimento de manual, à mão, no caso em que a mão
treme. Este documento é o desenho da volta: o que ela faz, o que ela **não**
faz, e por que cada decisão é esta e não outra.

---

## 1. As três saídas, e a escolhida

A tela de backup enunciava o problema em aberto antes de existir solução:

> «Sobrescrever um database em uso, com conexões abertas e a trava tomada,
> precisa de um desenho — parar o serviço, restaurar ao lado e trocar, ou
> restaurar com outro nome.»

São três, e a escolha entre elas é o projeto inteiro:

| | O que é | Custo | Estrago se errar |
|---|---|---|---|
| (a) parar o processo e trocar por cima | derrubar o `phxsqld`, mexer no disco, subir de novo | ninguém atende, nem a interface web | total: sem interface, sem desfazer |
| (b) restaurar ao lado e trocar os diretórios | copiar para fora, `rename` no fim | só a troca para | contido, se o antigo for guardado |
| (c) restaurar com **outro nome** | copiar para um database que ainda não existe | nenhum | nenhum: nada existente foi tocado |

**A (c) é o caminho principal, e a (b) é o «restaurar por cima».** A (a) não foi
implementada, e a ausência é deliberada: o `servico_parar` já para a porta de
dados **sem derrubar o processo**, e a interface web continua no ar para
religar. Derrubar o processo inteiro seria tirar do ar justamente a tela pela
qual a restauração é pedida.

### Por que a (c) como padrão

Restaurar com outro nome tem uma propriedade que as outras não têm: **é seguro
de errar.** Quem restaurou o backup do mês errado apaga o database e refaz;
quem restaurou por cima descobre o engano depois. E ela não precisa de nada —
nem da trava global durante a cópia, nem da porta de dados parada — porque o
database de destino **ainda não existe**: ninguém está lendo dele, e não há
descritor aberto para ficar apontando para um arquivo trocado.

O caminho normal fica então: *restaure com outro nome → confira o dado na
tela → decida*. Só quem precisa mesmo do nome original vai ao modo por cima.

### Por que a (b) para o «por cima»

Substituir um database em uso exige três coisas, e as três são exigidas:

1. **A porta de dados parada** (`porta_no_ar == false`). Com ela no ar, um
   cliente pode estar no meio de uma leitura do database que sai debaixo dele.
2. **Nenhuma conexão de dados aberta.** Parar o `accept` não fecha o que já
   está aberto, e a operação recusa dizendo quantas restam.
3. **`"confirmar": true`** no pedido. Substituir um database inteiro não
   acontece por engano de campo — é a mesma regra do `motivo` obrigatório do
   `esvaziar_lixeira`.

E o database substituído **não é apagado**: sai da raiz de dados para um
diretório vizinho, e o caminho volta na resposta (`anterior_em`). Restauração
que apaga o que substituiu não tem volta, e a hora em que se descobre que o
backup era do mês errado é sempre depois.

---

## 2. A ordem que faz a conferência valer

```text
1. lê o manifesto do backup (backup.json)          ── nada foi tocado
2. extrai para um PALCO, fora da raiz de dados     ── nada foi tocado
3. confere o SHA-256 de CADA arquivo               ── aqui recusa
4. troca, com um rename                            ── com a trava na mão
```

A regra é o passo 3 acontecer **antes** de o destino ser tocado: *backup
corrompido não vira database restaurado pela metade*. Ele não vira database
nenhum — o palco é apagado e o erro diz qual arquivo não bate.

O palco fica **fora da raiz de dados** de propósito. Um diretório dentro da
raiz seria listado como database enquanto a cópia acontece, e o `bancos`
mostraria um banco meio escrito. E fica **vizinho** dela, não em `/tmp`: a
troca final é um `rename`, que só é instantâneo — e atômico — dentro do mesmo
sistema de arquivos. Com `base: "dados"` (o padrão do `config.json`) o pai do
caminho é vazio, e o vizinho é o diretório de trabalho; cair no `/tmp` ali
seria quase sempre cair em outro sistema de arquivos, às vezes num `tmpfs`, e
a troca deixaria de ser um `rename` para virar uma cópia do database inteiro
para dentro da RAM.

### Onde a trava entra, e onde não entra

O pedaço caro — ler o backup, descomprimir e calcular um SHA-256 por arquivo —
acontece **fora** da trava global de dados. Só a troca final entra nela.
Restaurar um banco de dez gigabytes não para o servidor pelo tempo de dez
gigabytes; para pelo tempo de um `rename`. A trava ainda é necessária ali, e
por um motivo só: impedir que dois pedidos criem o mesmo database ao mesmo
tempo.

---

## 3. O protocolo

### `restaurar_backup`

```json
{"op":"restaurar_backup",
 "origem":"/backup/Comercial_ana_2026-08-29_0300.zip",
 "database":"Comercial_de_ontem"}
```

| Campo | Obrigatório | O que é |
|---|---|---|
| `origem` | sim | o `.zip` ou a pasta do backup |
| `database` | sim, fora da simulação | o nome com que o backup será restaurado |
| `de` | quando o backup tem mais de um database dentro | qual database de dentro do backup |
| `modo` | não | `novo` (padrão) ou `por_cima` |
| `confirmar` | sim, no `por_cima` | `true` |
| `simular` | não | lê e devolve o conteúdo, sem escrever nada |
| `ate` | não | **PITR**: reaplica o diário vivo até este instante (§ 7) |
| `ate_ms` | não | o mesmo instante em milissegundos; com `ate` junto, recusa |

`simular` é o que a tela usa para mostrar o que a cópia tem dentro antes de
alguém decidir: devolve quando foi gravada, por que versão, quantos arquivos,
quantos bytes, os databases e as tabelas.

### `backups`

```json
{"op":"backups","pasta":"/backup"}
```

Lista as cópias de uma pasta (sem `pasta`, a do `backup.destino`), com o que
cada uma traz dentro. De um ZIP lê **só o fim do arquivo e o manifesto**:
listar dez cópias de um gigabyte não custa dez gigabytes. Arquivo ilegível
entra na lista **dizendo que é ilegível** — sumir com ele seria esconder
justamente a cópia que precisa de atenção.

### A permissão, e o campo que o portão não enxerga

As duas exigem **administrar**, e a `restaurar_backup` está entre as operações
de escrita: um servidor `somente_leitura` a recusa.

O portão único do `despachar` confere o campo `"database"` do pedido — que na
restauração é o **destino**, o nome novo. O database que vem **dentro** do
backup não tem campo no pedido, e é ele que carrega o dado. Sem uma
conferência própria, bastaria administrar um banco de rascunho para despejar
nele o backup da folha de pagamento e ler tudo. É a mesma porta dos fundos do
`juntar` e do `unir`, e o conserto é o mesmo: um portão próprio dentro da
operação (`poder_no_backup`), que exige administrar **também** no database de
origem — inclusive na simulação, que mostraria as tabelas de quem não pode
vê-las. O `backups` aplica a mesma regra escondendo da lista o que aquela
sessão não poderia restaurar, e dizendo **quantos** escondeu.

---

## 4. O que a restauração NÃO faz

- **Não junta e não mescla.** O database que sai é byte a byte o que entrou no
  backup. Linha gravada depois da cópia não sobrevive à restauração por cima —
  é o que restaurar quer dizer. *A exceção pedida é o `ate` (§ 7), e ela não
  mescla nada: reaplica o diário na ordem em que ele foi gravado.*
- **Não restaura a raiz inteira de uma vez.** Um database por vez: restaurar
  seis por cima com um clique seriam seis estragos com um clique. A cópia da
  raiz continua servindo — escolhe-se de qual database dela restaurar.
- **Não migra formato.** Restaurar um backup de uma versão de formato que esta
  build não abre devolve um database que esta build não abre. O manifesto diz
  a versão que gravou, e a tela a mostra.
- **Não prova autoria.** O manifesto SHA-256 prova que o backup **não
  apodreceu** — bit trocado, arquivo truncado, cópia incompleta. Não prova que
  ninguém o reescreveu de propósito: quem alterar um arquivo *e* recalcular o
  SHA dentro do `backup.json` passa. Isso exigiria assinar o manifesto (HMAC
  com segredo do servidor, ou Ed25519 com a chave que o cadastro já usa), e
  hoje **não é feito**.
- **Não desfaz sozinha.** No modo por cima, o database anterior fica guardado
  fora da raiz e o caminho volta na resposta — mas devolvê-lo ao lugar é
  trabalho de quem administra, e o espaço em disco continua ocupado até
  alguém apagar.
- **Não avisa a tabela que já está em memória.** Um `memoria_carregar` feito
  antes da restauração continua servindo o retrato antigo até ser recarregado.
- **Não fala ZIP64.** O escritor de ZIP guarda deslocamentos em 32 bits, então
  uma cópia acima de 4 GiB não é representável — e isso já valia antes da
  restauração existir. O que mudou é que agora **falha alto**: o leitor confere
  a assinatura e o nome do cabeçalho local contra o diretório central, e um ZIP
  cujos deslocamentos não fecham é recusado em vez de restaurar lixo em
  silêncio. Backup grande, hoje, vai sem `zip` (árvore de diretórios).

---

## 5. O manifesto ganhou três campos

O `backup.json` passou a gravar **de que** a cópia é cópia:

```json
{"phxsql":"0.18.0","quando":"2026-08-29 03:00:04",
 "arquivos":9,"bytes":1048576,
 "escopo":"database","database":"Comercial",
 "conteudo":[{"caminho":"clientes.reg","bytes":8192,"sha256":"…"}]}
```

`escopo` é `"raiz"` (a raiz inteira; cada diretório de primeiro nível é um
database) ou `"database"` (um banco só, e os caminhos de dentro já são
relativos a ele). **Por que não deduzir sempre:** os caminhos quase sempre
bastam — numa cópia da raiz todo arquivo mora dentro de um diretório de
database, então um `.reg` solto no primeiro nível só acontece em cópia de um
banco. Quase sempre. Um database que só tenha schemas se escreve igualzinho a
uma raiz, e ali «quase sempre» quer dizer *restaurar um schema como se fosse um
banco*.

**Backup antigo continua restaurando.** Manifesto sem os campos cai na
dedução, e a resposta diz que deduziu (`escopo_declarado: false`) em vez de
afirmar — a tela mostra a marca «deduzido» ao lado da cópia. Nenhum backup já
gravado é reescrito, e o `conferir_backup` de sempre lê os manifestos novos
sem mudança nenhuma: os dois campos são acréscimos que um leitor antigo ignora.

E depois veio o terceiro, `quando_ms`, com o PITR: o `quando` sempre disse a
hora, mas em **texto de tela**, e quem compara instante com carimbo de evento
precisa do número. Os dois saem do mesmo valor dentro do gerador do manifesto —
dois campos de tempo preenchidos por dois caminhos são dois campos que um dia
divergem. Cópia gravada antes dele continua fazendo PITR pela volta do
`instante_iso`, e manifesto cujo `quando` não seja instante recusa **nomeando o
campo**. O leiaute está em `docs/FORMATO.md` § 10.

Nenhum número de versão aparece nesta seção de propósito: o que a restauração
pergunta é se o **campo está lá**, não em que lançamento ele entrou. Número
digitado à mão em oito lugares seria oito números que ninguém mediu.

---

## 6. O que a bateria de testes provou, e como

Toda prova é nos dois sentidos: o teste falha com o defeito reposto e passa com
o conserto.

| O que se quer garantir | Teste | Defeito reposto para provar que o teste vê |
|---|---|---|
| backup corrompido não vira database | `manifesto_que_nao_confere_e_recusado_e_nada_e_escrito`, `backup_adulterado_nao_vira_database` | `if false && &confere != sha` — a comparação do SHA-256 desligada. Os dois testes falham |
| caminho do backup não escapa da pasta | `caminho_que_escapa_da_pasta_e_recusado` | a chamada a `caminho_seguro` retirada do leitor do manifesto. O teste falha |
| por cima exige o serviço parado | `por_cima_com_a_porta_de_dados_no_ar_e_recusado` | `if false && self.porta_no_ar.load(...)`. O teste falha |
| o backup do banco alheio não entra por um destino permitido | `o_backup_do_banco_alheio_nao_entra_por_um_destino_permitido` | a chamada a `poder_no_backup` retirada da operação. O teste falha |
| **o comportamento velho** | `quem_nao_usa_a_operacao_nova_nao_ve_diferenca`, `backup_antigo_sem_escopo_no_manifesto_ainda_restaura`, `zip_antigo_de_um_banco_e_deduzido_como_database` | — (é o teste que trava a regressão, não o que prova o recurso) |
| **PITR:** o corte de carimbo corta | `restaura_ate_um_instante_no_meio_do_diario` | `if false && e.carimbo > ate_ms` — a linha 3 aparece no restaurado e as três asserções caem |
| o diário do restaurado não mente sobre quando | `o_diario_do_restaurado_guarda_o_carimbo_original` | a chamada a `forcar_proximo_evento` retirada. O teste falha |
| sem imagem no diário, recusa nomeando o interruptor | `sem_imagem_no_diario_o_pitr_recusa_dizendo_o_interruptor` | `if false && !self.config.replicacao.imagem_da_linha`. O teste falha |
| backup sem carimbo recusa o PITR e **não** a restauração | `backup_sem_carimbo_recusa_o_pitr_e_nao_a_restauracao` | `conteudo.quando_ms.or(Some(0))` — o diário inteiro é reaplicado. O teste falha |
| `ate` antes da cópia recusa | `ate_antes_da_copia_e_recusado` | `if false && ate_ms < copia_ms`. O teste falha |
| por cima com `ate` recusa | `por_cima_com_ate_e_recusado` | `if false && por_cima`. O teste falha |
| o diário vivo tem de continuar o da cópia | `diario_que_nao_continua_a_copia_para_nomeando_a_tabela` (a mensagem da contagem), `diario_vivo_maior_e_diferente_tambem_para` (a comparação do evento) | `diario_vivo_continua` devolvendo sempre `Ok(())`: os dois caem. Cada metade sabotada sozinha derruba **só** o teste que a acorda |
| **o comportamento velho do PITR** | `quem_nao_manda_ate_nao_ve_diferenca` | — (sem `ate`, a resposta não ganha campo e nada é reaplicado) |

O teste do comportamento velho é o que mais importa aqui, e ele guarda três
coisas: o `restaurar` de **linha** (desfazer uma exclusão) continua sendo o que
era — o nome não mudou de dono —, o `backup`/`conferir_backup` continuam
conferindo com o manifesto novo dentro, e a raiz de dados não ganha nada
sobrando depois de uma restauração.

### O que se aprendeu medindo, e não supondo

**A leitura do ZIP precisava dos três tipos de bloco, e não do nosso.** O
compressor daqui só emite DEFLATE com Huffman **fixo**, e a tentação era ler só
isso — o leitor teria metade do tamanho. Mas quem baixa o backup, abre para
olhar e compacta de novo devolve Huffman **dinâmico**, porque é o que todo
compressor do mundo emite: `zlib`, `zip`, `7z`, o Explorer do Windows®. Um
leitor que só entende o próprio dialeto recusaria justamente o arquivo que o
operador acabou de conferir na mão. O teste do dinâmico é um **vetor** — bytes
produzidos pela zlib, com o texto que tem de sair deles —, pela mesma razão
que a criptografia se confere contra vetor oficial: ida e volta com o próprio
compressor não prova nada sobre ler o que os outros escrevem, porque os dois
lados podem estar errados juntos.

**O `/tmp` como palco morreu medido.** A primeira versão usava
`base.parent()`, e com o `base: "dados"` do `config.json` padrão o pai é
**vazio** — o palco caía em `std::env::temp_dir()`. Funciona, e a prova pelo
navegador mostrou o database substituído indo parar em
`/tmp/.phxsql-substituido-…`. Só que `/tmp` costuma ser outro sistema de
arquivos, e num `tmpfs` a troca deixaria de ser um `rename` para virar uma
cópia do database inteiro para dentro da RAM — no meio da trava. O conserto é
uma linha (pai vazio = diretório de trabalho) e tem teste próprio,
`o_vizinho_da_base_relativa_e_o_diretorio_de_trabalho`. **Achado exercitando,
não lendo:** no teste unitário o `base` é sempre absoluto, e o defeito não
aparecia.

**E a tela mentiu sobre o dado, de novo.** O título da seção do detalhe usava
a classe `.secao`, que é caixa-alta: um database chamado `Comercial` aparecia
como `COMERCIAL`, e o arquivo `dados_root_….zip` como `DADOS_ROOT_….ZIP`. É a
mesma armadilha de «Blumenau» virando «BLUMENAU» dentro da grade — quem olha
não sabe se está gravado assim. O conserto é pôr o nome dentro do `<em>`, que
é o único pedaço de `.secao` sem transformação. **Nenhuma das duas aparece
lendo o código.**

---

## 7. PITR — restaurar a um INSTANTE

Até aqui, restaurar era voltar ao instante da cópia, e nada mais. O backup das
três da manhã devolve o banco das três da manhã; o que aconteceu entre as três
e o engano das dez estava perdido junto com o engano.

O PITR fecha essa distância, e a ideia inteira cabe numa frase: **a cópia
restaurada vira réplica do diário vivo, do instante da cópia até o instante
pedido.**

```json
{"op": "restaurar_backup",
 "origem": "/backup/Comercial_ana_2026-08-29_0300.zip",
 "database": "Comercial_as_15h",
 "ate": "2026-08-29T15:00:00Z"}
```

### 7.1 Por que ele não é código novo

Reaplicar um diário sobre uma cópia é exatamente o que uma réplica faz o dia
inteiro: pega o evento, tira a imagem da linha de dentro dele, grava. O PITR
chama **o mesmo `Table::aplicar_evento`** da replicação, com a mesma marca de
`como_replica` — **aplica, não julga**.

Escrever um segundo aplicador teria sido mais fácil de ler e é a decisão
errada: *o segundo caminho é o que um dia esquece uma conferência*, e a
conferência que ele esqueceria é justamente a do rowid, que é o que impede a
reaplicação de espalhar uma divergência.

### 7.2 O começo sai da POSIÇÃO, e não do relógio

O `.log` de cada tabela **viaja dentro do backup** — medido: uma cópia tirada
com um evento no diário leva um `clientes.log` de um evento, e o `.log` vivo
passa a ter dois, com o primeiro byte a byte igual ao copiado.

Isso dá o começo de graça e sem relógio nenhum: **o número de eventos do
diário da cópia é a posição em que o mundo estava na hora da cópia.** A
reaplicação lê o diário vivo a partir dali.

Começar por «o primeiro evento cujo carimbo passou da hora da cópia» seria
pedir ao relógio a única coisa que ele não sabe responder — ver a seguir.

### 7.3 O carimbo NÃO é monotônico, e por isso o filtro é evento a evento

Medido, num diário de três eventos:

```
carimbos: [1788884516705, 1000000000000, 1788884516705]
origens:  [0,             7,             0]
```

O do meio está vinte e cinco anos atrás. Não é defeito: é o caminho
bidirecional, que carimba o evento com o instante em que a escrita **nasceu**
no outro servidor (`Table::forcar_proximo_evento`), porque é esse instante que
decide o conflito lá. E há a segunda causa, mais banal: o `agora_ms` é relógio
de parede (`SystemTime::now`), que anda para trás num acerto de NTP.

Consequência de projeto: **o corte de cima é aplicado evento a evento**
(`carimbo <= ate`), e nunca «corte a lista no primeiro que passou». Cortar por
posição jogaria fora os eventos bons que vêm depois de um carimbo torto.

E pular um evento no meio **não é silencioso**: o `aplicar_evento` confere o
rowid, então pular uma inclusão e aplicar a seguinte para na hora, com «o
source diz rowid 2 e aqui saiu 1». Isso vira o `parou_em` da resposta, em vez
de gravar a linha errada no slot errado. *A guarda que já existia para a
réplica é a mesma que segura o PITR.*

### 7.4 O que ele NÃO refaz, e por quê

- **Cascata.** A origem já cascateou quando aceitou a escrita, e os eventos
  que a cascata dela gerou estão no diário das **filhas**. Refazer aqui criaria
  evento que o original nunca teve.
- **Chave estrangeira.** Pelo mesmo portão e pelo mesmo motivo: a reaplicação
  anda por tabela, e não há ordem global entre tabelas. Filha órfã no meio da
  passada se cura quando a tabela da mãe for reaplicada; recusar travaria a
  restauração inteira por causa de uma ordem que se resolve sozinha.
- **Coluna calculada, `padrao`, `check`.** Nada é recalculado: a imagem que
  vem do diário já saiu da origem com tudo aplicado.
- **O `.tx`.** A marca de commit em curso já viaja dentro do backup de
  propósito, e quem a completa é a recuperação do arranque
  (`transacao::recuperar`). Ela é **idempotente pelo rowid**, então reaplicar o
  diário por cima não duplica inclusão nenhuma — e o PITR não a toca, porque
  dois donos para o mesmo commit seriam um a mais.

### 7.5 O carimbo reaplicado é o do ORIGINAL

O evento que a reaplicação grava no diário do restaurado leva o carimbo e a
origem do evento original, e não a hora da restauração. O diário é trilha de
auditoria: um restaurado que jurasse que tudo aconteceu na hora em que foi
restaurado destruiria justamente o que se foi buscar nele.

É o mesmo mecanismo e o mesmo motivo do bidirecional.

### 7.6 As tabelas que existem de um lado só

- **Na cópia e não mais viva** (apagada depois do backup): fica como estava na
  cópia, e sai na resposta em `sem_diario_vivo`. Não dá para saber **quando**
  ela foi apagada — apagar tabela não deixa evento em diário nenhum —, e sumir
  com ela em silêncio seria pior.
- **Viva e não na cópia** (nasceu depois do backup): **não é criada**, e sai em
  `novas_na_origem`. Refazê-la exigiria a história do esquema, que o formato
  não guarda. Dizer que a restauração é «o estado às 15h» e não avisar da
  tabela que faltou seria a mentira mais cara desta página.

### 7.7 O diário que não alcança mais a cópia

Não há rodízio de `.log` de tabela: o `rodizio.rs` desta casa é dos logs de
**texto** (`perfil.txt`, `diretivas.log`, `acessos.log`), e o único lugar do
motor que apaga um `.log` de tabela é o `excluir_tabela`, que leva a tabela
junto.

Sobra um caso, e ele é real: a tabela foi **apagada e recriada** depois do
backup. Aí o diário vivo começa do zero, a posição da cópia aponta para o meio
de uma história que não é a mesma, e reaplicar dali gravaria linhas de outra
vida.

Quem acha isso é a comparação do último evento da cópia com o evento daquela
posição no diário vivo. A conta de tamanho que vem antes dela **não pega
nenhum caso a mais** — diário mais curto que a posição devolve lista vazia, e
lista vazia já cai na recusa —, e isso foi medido sabotando: com a conta
desligada, os dois testes de continuidade continuam vermelhos pela comparação.
Ela fica só pela **mensagem**, que é o que um operador precisa ler, e o teste
que a prova confere o texto e não o veredito.

### 7.8 As recusas, e quando cada uma acontece

**Antes de tocar em disco** — porque uma restauração que criasse o database e
só então descobrisse que não consegue reaplicar deixaria o pior estado
possível: um banco novo, com o nome pedido, no instante errado, e um erro na
resposta.

| Recusa | O que dispara |
|---|---|
| `ate` ilegível | não é instante, ou traz fuso escrito à mão (`+03:00`) |
| `ate` e `ate_ms` juntos | dois campos de tempo querendo dizer coisas diferentes |
| modo `por_cima` | o por cima tira o database vivo da raiz, e é o diário **dele** que a reaplicação lê |
| backup sem carimbo | falta o `quando_ms` do manifesto e o `quando` não é instante |
| `ate` antes da cópia | o diário só sabe andar para a frente |
| imagem desligada | `replicacao.imagem_da_linha` está `false`: o evento diz que o rowid 42 mudou e não diz para quê |
| origem sumiu | o database de dentro do backup não existe mais neste servidor |

**Por tabela, na resposta** — a continuidade só se confere com o diário da
cópia na mão, ou seja, com a cópia já restaurada. Ela sai **nomeada** no
`parou_em`, dizendo que aquela tabela ficou no instante da cópia. Devolver erro
ali deixaria o pedido com um database criado e uma resposta de fracasso.

### 7.9 A resposta

```json
{"database": "Comercial_as_15h", "de": "Comercial", "…": "…",
 "pitr": {
   "de": "Comercial",
   "copia_ms": 1787972404132, "copia": "2026-08-29 03:00:04,132",
   "ate_ms": 1788015600000,   "ate":   "2026-08-29 15:00:00,000",
   "reaplicados": 2,
   "tabelas": [{"tabela": "clientes", "reaplicados": 2, "pulados": 1,
                "ultimo_carimbo_ms": 1788015000123,
                "ultimo": "2026-08-29 14:50:00,123"}],
   "sem_diario_vivo": [],
   "novas_na_origem": []}}
```

`pulados` são os eventos que existem no diário e ficaram **depois** do corte —
e é o número que prova que o corte aconteceu. `parou_em` só aparece quando
aquela tabela não foi até o fim, com o motivo escrito.

`ate` é **inclusivo**: quem pede «até as 15:00:00» quer o que aconteceu às
15:00:00,000. O instante que a tela mostra é o instante que se digita de volta.

### 7.10 A sequência que PROVA o PITR

Ela **já roda**, em `bancada/pitr/provar.py` — 22 conferências pelo soquete,
zero falhas, com o resultado datado em `bancada/pitr/resultados.json`. E a
prova real é nos dois sentidos: com o filtro de carimbo desligado no servidor,
a mesma bancada acusa **3 falhas** e mostra a linha 3 dentro do restaurado.

Fica escrita aqui também porque *roteiro que resolveu algo não pode morrer com
a sessão*, e porque é ela que a `bancada/comparativo/` precisa para trocar a
sonda de **código** do PITR por uma sonda de **efeito**. O servidor precisa
de `"replicacao": {"imagem_da_linha": true}` no `config.json` **antes de
subir** — a imagem vale para o que for gravado daí em diante, e não para o
diário que já está no disco.

```json
1  {"op":"criar_database","database":"loja"}
2  {"op":"criar_tabela","database":"loja","tabela":"clientes",
    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
               {"nome":"nome","tipo":"Str(20)"}],
    "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}
3  {"op":"inserir","database":"loja","tabela":"clientes",
    "linha":{"id":1,"nome":"um"}}
4  {"op":"backup","destino":"/backup","database":"loja","zip":true}
      -> guarde a resposta "arquivo"
5  {"op":"inserir","database":"loja","tabela":"clientes",
    "linha":{"id":2,"nome":"dois"}}
6  {"op":"atualizar","database":"loja","tabela":"clientes","rowid":1,
    "linha":{"id":1,"nome":"um alterado"}}
7  {"op":"inserir","database":"loja","tabela":"clientes",
    "linha":{"id":3,"nome":"tres"}}
8  {"op":"diario","database":"loja","tabela":"clientes","max":100}
      -> quatro eventos; CORTE = carimbo_ms do 4o menos 1
9  {"op":"restaurar_backup","origem":"<o arquivo do passo 4>",
    "database":"loja_no_meio","ate_ms":<CORTE>}
10 {"op":"varrer","database":"loja_no_meio","tabela":"clientes"}
```

O que o passo 10 tem de devolver: **id 1 com «um alterado», id 2, e nada de id
3.** E o passo 9 tem de dizer `reaplicados: 2` e `pulados: 1`.

Duas armadilhas da sonda, e as duas já pagas no teste:

- **Espere o relógio andar** entre os passos 3-4 e 5-6-7. Três escritas
  seguidas caem no mesmo milissegundo, e aí não existe instante entre a
  alteração e a inclusão da terceira linha. A sonda confere isso antes de
  cortar: o carimbo do 4º evento menos 1 tem de ser **maior ou igual** ao
  carimbo do 3º; se não for, repita com pausa.
- **O corte sai do próprio diário, medido.** Um `ate` digitado à mão seria um
  número que ninguém mediu — e é exatamente o erro que esta casa já pagou
  quatro vezes.

O controle da sonda, na mesma corrida: o mesmo backup restaurado **sem** `ate`
devolve só o id 1, e o banco `loja` continua com as três linhas. Sem esse
controle, uma sonda que devolvesse duas linhas por acaso passaria.

---

## 8. A tela

*Arquivo → Restaurar um backup…*, o botão **Restaurar** na barra de
ferramentas (ao lado do Backup), ou o botão dentro de *Backup e restauração*.

O botão na barra não é enfeite: o pedido chegou como «falta o botão restaurar»
com a tela de backup no ar há semanas. **Botão que não se acha não existe.**

A tela procura as cópias na pasta do `backup.destino`, mostra o que cada uma
tem dentro, e oferece as duas formas **lado a lado** — não uma escondida atrás
da outra, porque a escolha entre elas é a decisão inteira. Cores da casa: azul
consulta abre a cópia, verde inclui cria o banco novo, vermelho exclui de vez
substitui o que existe, sempre em contorno, com preenchimento só no `hover`.

O modo por cima só libera o botão depois de o nome do database ser **digitado**
no campo de confirmação — e não com um `confirm()`, que só sabe perguntar sim
ou não e é respondido no reflexo.
