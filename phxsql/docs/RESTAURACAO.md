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
  é o que restaurar quer dizer.
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

## 5. O manifesto ganhou dois campos

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

## 7. A tela

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
