# Parecer do DBA (papel C) — pedido 268: a migração da cifra, `criptografar`/`descriptografar`

**Data:** 17/09/2026. **Escopo:** só leitura e provas de leitura — este parecer
não conserta nem comita; nomeia o *não* e o porquê, e deixa a decisão com o
dono. Todo número aqui foi **lido no código ou medido nesta data**; onde é
estimativa, a receita está escrita ao lado.

---

## 0. A recomendação, em uma linha

**Construir a saída (b) como uma operação com nome — `criptografar` /
`descriptografar` —, pelo molde do `acrescentar_coluna` (arquivo `*.novo` ao
lado, slot a slot na mesma ordem, `rename` com o volume 1 como ponto de
compromisso), SEM nenhuma mudança de formato do `.reg`, com o `.memo`/`.bin`
dentro do escopo; recusar (a) e (c); e a mudança de formato que entra cedo é
OUTRA — o envelope da chave (`SEGURANCA.md` §11.5) e uma marca «selado/claro»
por externo na imagem da linha do `.log`.**

---

## 1. O que o cabeçalho do `.reg` sabe hoje — medido, não lembrado

**Sabe.** A pergunta 1 do briefing partia da hipótese de que uma tabela cifrada
e uma em claro fossem indistinguíveis no disco. Não são — e isto está no
formato desde 29/08/2026 (commit `9f561c8`, «Cifra o valor da coluna marcada»).

O que o cabeçalho carrega (`docs/FORMATO.md` §1, linhas 84–119;
`crates/phxsql-store/src/reg.rs:96-129`):

| Off | Tam | Campo | O que diz sobre a cifra |
|----:|----:|---|---|
| 8 | 2 | versão do formato | **4** = em claro, **5** = com coluna cifrada. É o primeiro byte lido; decide quantos bytes de cabeçalho ler |
| 10 | 2 | tamanho do cabeçalho | **128** na 4, **192** na 5 — o segundo discriminador |
| 128 | 1 | sinais (só na 5) | bit 0: cifrado; bit 1: modo FrogCript |
| 132 | 4 | iterações do PBKDF2 (só na 5) | |
| 136 | 16 | sal, em claro (só na 5) | a chave desta tabela sai da senha **mais este sal** |
| 152 | 16 | prova da chave (só na 5) | recusa a senha errada **na abertura**, não na primeira linha |

E a leitura consulta **só isso** para decidir se decifra: `RegFile::montar`
(`reg.rs:388-452`) lê a versão no byte 8, e o material vem de
`cofre::Material::ler` quando é 5 e de `Material::EM_CLARO` quando é 4. O
`config.json` não entra nessa decisão em lugar nenhum. O `esquema` responde
`material` (`cifrado`/`em_claro`) por tabela (`servidor.rs:16332`), e o
relatório `dados_pessoais` traz o mesmo campo (`servidor.rs:10592`).

**Medido nesta data**, com um leitor de cabeçalho de 30 linhas em Python que só
lê (área de rascunho, não versionado): **66 arquivos `.reg`** fora de `target/`
no repositório, **66 na versão 4, 0 na versão 5, 0 `.reg.novo` órfãos**. O
maior deles, `bancada/phxsql/precos.reg`: versão 4, cabeçalho 128, `slot_size`
122, 10.000.000 slots, 9.980.000 vivos, `data_offset` 640, CRC do cabeçalho
conferindo. Ou seja: **não há um único `.reg` da versão 5 neste repositório**
— o que pesa na §5 («cedo» ainda é agora).

O corolário para o pedido: quando o 268 nasceu anotado como «papel C, cabeçalho
do `.reg`», a anotação apontava para o lugar errado. O cabeçalho já tem tudo o
que uma migração precisa saber. O que falta é a **operação**, não um byte.

---

## 2. O que acontece HOJE ao declarar sobre uma tabela com 100.000 linhas em claro — provado

### 2.1 `cifra.tabelas` tem um leitor só, e ele não é o motor de dados

Varrido em `crates/` nesta data: a lista `cifra.tabelas` é lida por
`config.rs` (só para **validar** o formato `"banco.tabela"` e recusar
duplicata, linhas 1414–1437), é entregue ao **Profiler** (`servidor.rs:4588`
a quente e `21519` no arranque), e o Profiler é quem decide `sigiloso`
(`profiler.rs:725,745`) para **não gravar o texto do pedido** no `perfil.txt`.
**Nenhuma linha de `phxsql-store` lê essa lista.** O método
`Cifra::tabela_sigilosa` (`config.rs:1499`) não tem chamador em `crates/`;
quem responde de fato é `Profiler::tabela_e_sigilosa`.

Então a resposta à pergunta 2 é dupla:

- **Nada acontece com as 100.000 linhas.** Não viram lixo, não são recusadas,
  não são recifradas. Continuam em claro, legíveis, e as gravações seguintes
  **continuam em claro**, porque o `.reg` decide pelo cabeçalho dele.
- **O que muda é só o Profiler**: pára de gravar o texto dos pedidos que tocam
  a tabela. Foi para isso que a lista nasceu (`SEGURANCA.md` §13, 05/09/2026),
  e o rodapé da tela já diz «declarar não cifra» nos seis idiomas (pedido 196).

### 2.2 O que de fato decide a cifra, e quando

`RegFile::criar` (`reg.rs:274-286`): o material nasce cifrado quando
`esquema.tem_dado_pessoal()` **e** `cofre::ligado()` — e `Material::novo()`
devolve `EM_CLARO` quando o cofre está desligado (`cofre.rs:347-350`). A
decisão é **na criação e nunca depois** (`reg.rs:257-260`). A «declaração» que
leva à cifra é a **marca de dado pessoal por coluna** (`DadoPessoal`,
`marcar_lgpd`), não a lista por tabela do `config.json`. Uma tabela nomeada em
`cifra.tabelas` **sem coluna marcada nasce em claro mesmo nova** — o briefing
dizia «não cifra o que já está gravado»; é menos do que isso: a lista não
cifra o que ainda vai ser gravado tampouco.

E o caminho irmão, `remarcar_dado_pessoal` (`reg.rs:1081-1130`): numa tabela
da versão 4 com dado, marcar uma coluna com o cofre ligado é **aceito**, o
`slot_size` não muda (rabo zero em `EM_CLARO`), o esquema é regravado e a
tabela **continua em claro** — a recusa «reselar não existe» só dispara quando
`material.cifrado()` já é verdadeiro (linha 1103). O
`acrescentar_coluna` idem (`reg.rs:1260-1267`): «o material é o da tabela e
não se rederiva aqui».

### 2.3 A prova, rodada nesta data sem subir servidor

O teste que já existe para exatamente isso —
`crates/phxsql-store/tests/cifra-dos-dados.rs:139`,
`tabela_escrita_antes_da_cifra_continua_abrindo` — grava 30 linhas em claro
(versão 4), **liga o cofre**, reabre a mesma tabela, lê a linha 7 em claro,
insere a 31 e altera a 3, e confere que a versão **continua 4**. Rodado num
`target` isolado na área de rascunho (`CARGO_TARGET_DIR=…/scratchpad/target
cargo test --offline -p phxsql-store --test cifra-dos-dados
tabela_escrita_antes_da_cifra_continua_abrindo`), para não tocar no `target`
do time: compilação **12,24 s**, corrida **1,06 s**, **1 passou, 0 falhou**.

### 2.4 O que isto diz sobre a pétrea invocada no briefing

O briefing chamou isso de «o dado mentindo sobre si mesmo». **Não é.** O disco
diz a verdade — versão 4, `material: em_claro` —, e diz em três lugares
(`esquema`, `dados_pessoais`, `--example tabela-marcada-nasceu-em-claro`, que
lista **toda** tabela com coluna marcada e material em claro, não só as do
pedido 210 como o texto dele sugere). O que promete mais do que cumpre é o
**nome** de um campo de configuração: `cifra.tabelas` mora na seção `cifra` e
se chama `tabelas`, e é uma lista de redação do Profiler. A pétrea que alcança
isso é «configuração que não é lida mente», na variante «lida por menos do que
o nome promete». É achado de configuração, não de formato — e por isso fica
nomeado aqui e não resolvido: renomear ou não é decisão fora do meu domínio.

---

## 3. As três saídas, com o tradeoff de cada uma

Para cada uma: ordem de digitação, replicação (o que a réplica recebe), queda
no meio, custo.

### 3.1 (a) Recusar a declaração quando a tabela já tem dados — **não**

Há três «declarações» onde a recusa poderia morar, e nenhuma serve:

- **Recusar em `cifra.tabelas`** uma tabela que já tem dado: recusaria a
  redação do Profiler justamente na tabela que **tem** dado a proteger — e a
  §13.4 já decidiu que a lista nomeando tabela inexistente **sobe** o servidor,
  porque declarar antes de criar é ordem legítima de modelagem.
- **Recusar `marcar_lgpd`** numa tabela da 4 com dado e cofre ligado:
  bloquearia o inventário LGPD, que tem leitores além da cifra (relatório
  `dados_pessoais`, retenção, redação); e não alcança o caso comum, que é a
  coluna marcada **antes** de o cofre ser ligado — nesse instante não há
  declaração nenhuma a recusar.
- **Recusar o arranque** com cofre ligado e tabelas marcadas na 4: é guarda
  **imposta**, que é o que «guarda nova entra pedida» proíbe.

A analogia com a chave conferida não segura: lá a declaração (`declarar_fk`) e
a garantia (conferir) eram o **mesmo ato**, e o dono pôde mandar o ato nascer
conferido. Aqui a separação no tempo entre marcar a coluna e cifrar o disco é
**desenho** («ligar a cifra não cifra o que já existe», §11.6), não descuido.
Ordem, replicação e queda: não se aplicam. O que sobra de (a) e vale — o choque
**aparecer** por tabela — já existe pelos três leitores do `material`.

### 3.2 (b) Migrar, slot a slot — **sim, mas não «no lugar»**

**«No lugar» é impossível pelo formato e foi recusado pela casa.** Da 4 para a
5 o slot cresce 16 bytes (a etiqueta) e o cabeçalho cresce de 128 para 192, o
que empurra o `data_offset` — escrever o slot 1 alargado comeria o começo do
slot 2. E mesmo onde caberia, a casa já recusou reescrever no próprio arquivo
pela queda: «deixaria exatamente esse meio-termo se a máquina caísse»
(`reg.rs:2299-2301`). A migração é **ao lado, e troca por `rename`**.

**O molde já está escrito, e foi escrito para isto.** `acrescentar_coluna`
(`reg.rs:1208-1402`) usa `escrever_volume_alargado` (`reg.rs:2409-2464`): o
i-ésimo slot lido é o i-ésimo escrito, **inclusive os livres**, e por isso
`rowid = primeiro + i` vale dos dois lados e o `.ndx` não é tocado. O
`transformar` recebe **material, faixas e `slot_size` como parâmetros**
justamente porque «a reescrita sela com o esquema NOVO enquanto ainda abre com
o velho» (`reg.rs:1710-1712`). `criptografar` = abrir cada slot com
`EM_CLARO` e selar com `Material::novo()`; `descriptografar` = o inverso. A
mecânica **não depende de o slot crescer** — o laço lê `quantos` slots de
`slot_velho` e escreve `slot_novo` cada (`reg.rs:2450-2456`); só o nome da
função diz «alargado».

- **Ordem de digitação:** preservada **por construção** — posição é rowid,
  livre continua livre, `.ndx` intocado. É o mesmo argumento do `FORMATO.md`
  §1.1: compactar os buracos na passagem renumeraria tudo, e não se faz.
- **Queda no meio:** a fase A escreve todos os `*.novo` sincronizados sem
  trocar nada; a fase B troca com `rename`, volume 1 primeiro;
  `terminar_troca_interrompida` termina para a frente e
  `conferir_volumes_uniformes` recusa conjunto misturado (`reg.rs:539-629`).
  **Sem byte novo no cabeçalho:** `geometria_do_volume` compara
  `(slot_size, data_offset, CRC do esquema)`, e o `data_offset` da 5 é
  **sempre** o da 4 mais 64 (cabeçalho 128→192 alinhado a 64), então o
  mecanismo já distingue os dois lados — inclusive na tabela só de externas,
  cujo `slot_size` não muda. Tabela de um volume: um `rename`, atômico.
  **A conferir antes de reaproveitar o molde**, e isto é herança e não
  novidade: numa tabela de **um** volume com espelho a fase B faz dois
  `rename` (`.reg` e `.bkp`) e `trocas_por_terminar` devolve vazio sem
  paginação (`reg.rs:561`); não achei, por leitura, quem confere a geometria
  do espelho contra a do principal numa queda entre os dois. Vale para o
  `acrescentar_coluna` de hoje tanto quanto para (b).
- **Replicação:** a operação é **local e não replica a si mesma**, como o
  `ALTER` (`FORMATO.md` §1.1: «uma réplica só volta a aplicar eventos depois
  de receber a mesma alteração»). A réplica **não recebe slot**: recebe a
  **imagem da linha**, decodifica em valores e chama o `inserir`/`atualizar`
  dela (`table.rs:4021-4034`), selando com o material do **próprio**
  cabeçalho — que saiu do cofre **dela** quando a tabela **dela** nasceu. Na
  imagem, a faixa **inline** marcada viaja **em claro** (protegida só pela
  cifra do corpo do `.log`, versão 3 — §11.7), e a coluna **externa** viaja
  **como está no bloco**, selada quando a origem é cifrada
  (`table.rs:3814-3818`) — e **sem nenhuma marca dizendo se está selada**.
  Consequência que a migração cria e que hoje só existe como frase da §11.8:
  origem migrada e réplica não (ou o inverso) passam a divergir **no meio do
  fluxo** — a réplica em claro recebe um `Memo` selado e pára com «memo nao e
  UTF-8»; recebe um `Bin` selado e **guarda o texto cifrado como se fosse o
  dado, sem erro** (`table.rs:4066-4068`, `abrir_externo` devolve os bytes
  como vieram quando o material local é `EM_CLARO`). Logo: **migrar os dois
  lados no mesmo passo**, e a resposta do `criptografar` tem de dizer isso.
- **O que o §13.6 não nomeou: `.memo`/`.bin`.** Ele nomeou `.log`/`.trash`/
  `.reason` (histórico, ficam) e `.ndx` (em claro, 200/200 pares). Não nomeou
  que cifrar coluna **externa** marcada é selar o **conteúdo dos blocos**, e
  os blobs são «pilha de blocos append-only… o espaço morto volta com a
  compactação» (`blob.rs:3,13`) — e «sem compactação implementada»
  (`FORMATO.md:2286`). Um `criptografar` que selasse só as faixas do `.reg`
  deixaria o memo sigiloso **legível no `.memo`**. Desenho que fecha isso sem
  um segundo ponto de compromisso: (1) anexar ao `.memo`/`.bin` a cópia selada
  de cada bloco vivo — offsets antigos ficam válidos; (2) escrever o
  `.reg.novo` com as faixas seladas **e os ponteiros novos**; (3) `rename`,
  volume 1 primeiro; (4) **zerar** o conteúdo dos blocos velhos, já mortos —
  passo idempotente e retomável («todo bloco morto deste arquivo que não é
  zero»). Queda antes do (3): estado velho inteiro, mais blocos selados mortos.
  Queda entre (3) e (4): dado consistente, e o (4) se retoma. É decisão a
  confirmar (zerar bloco morto num append-only), não afirmação minha de que
  já se faz.
- **Histórico e diários:** `.log`, `.trash`, `.reason` ficam como estão
  (§13.6). E há um irmão a nomear: o volume **corrente** do `.log` e do
  `.trash`, se nasceu em claro (versão 2), **continua recebendo imagens em
  claro** depois da migração — é o «buraco real» da §11.7. O diário só abre
  volume novo por tamanho (`log.rs:498-504`); o `criptografar` deveria
  **virar o volume** dos três diários no mesmo ato, para as imagens novas
  nascerem na versão 3. Sem isso a migração protege o `.reg` e o diário
  seguinte a desprotege.
- **`.ndx`:** continua em claro — medido no pedido 194, 200 de 200 pares. A
  resposta da operação diz.
- **Pré-condições da operação:** cofre ligado; ao menos uma coluna marcada
  (senão «nada a cifrar: marque a coluna antes»); `descriptografar` exige a
  chave, porque a 5 só abre com ela; **nunca automática** — nem no arranque,
  nem no `config` a quente, nem por estar em `cifra.tabelas`. A lista continua
  sendo do Profiler; a operação tem o próprio nome.
- **Custo — estimativa com receita explícita, composta de partes medidas, não
  medida ponta a ponta:**
  - reescrita slot a slot: **0,553 µs/linha** a 10 milhões, 427 MiB/s lido +
    escrito, sequencial (`DESEMPENHO.md` §4.14, `--example custo-do-alter`);
  - selagem AEAD: **0,10 µs por valor** de 22 B e **0,585 µs** por payload de
    128 B, 330 MB/s (`SEGURANCA.md` §11.1 e §11.4, `--example
    custo-da-cifra`);
  - PBKDF2 **uma vez** por material novo: **298 ms** a 210.000 iterações
    (`SEGURANCA.md:1496`; a §13.7 mediu 290,3 ms);
  - 100.000 linhas (o cenário do briefing): 55 ms + 10 ms + 298 ms ≈ **0,36
    s**, mais ≈ 65 ms se houver espelho — **o PBKDF2 domina**;
  - 10.000.000 linhas: 5,53 s + 1,0 s + 0,3 s ≈ **6,8 s**;
  - disco: pico **2× o `.reg`** (velho e `*.novo` convivem), mais 2× o `.bkp`
    — 1,1 GiB viram 2,3 GiB no pico para dez milhões (§4.14);
  - modo FrogCript: rabo de `largura marcada + 167` por linha em vez de 16
    (§11.4), mesma mecânica, arquivo maior.
  - O medidor `--example custo-do-criptografar`, no molde do `custo-do-alter`,
    é o **primeiro** entregável da frente — número citado é número que não se
    mede.

**Veredito:** é a saída. **Nenhuma mudança de formato do `.reg`.** E, escrita
como «reselar a tabela com o material X», a mesma primitiva serve para
`criptografar`, `descriptografar`, rotação de senha sem envelope, e a migração
da versão 5 para a 6 quando o envelope entrar (§5).

### 3.3 (c) Cifra por linha com marca por slot — **não**

O bit está disponível de graça: o byte 1 do slot (`flags`) **nunca é escrito**
— `montar_slot_com` preenche só o status (byte 0), a versão (8..16) e o
tempero (16..24) (`reg.rs:1722-1738`). Mas a marca não é o problema: a
**etiqueta** é. Cada slot cifrado precisa dos 16 bytes de rabo, e o rabo **só
existe no slot da versão 5**. Então (c) **não resolve o 268**: as 66 tabelas da
4 deste repositório, e toda tabela nascida sem cofre, não têm rabo — e dar o
rabo é reescrever, que é (b). O que (c) «resolveria» é só a tabela nascida já
com rabo — o que exige cobrar 16 bytes por linha de quem não cifra, recusado
com o motivo escrito na §11.6 («carimbá-la de cifrada custaria 16 bytes por
linha e um cabeçalho maior para não proteger nada»).

E o que ela custa além disso:

- **estado misto permanente, por desenho** — a «metade cifrada e metade em
  claro que abre sem erro nenhum» que o §13.6 nomeia como o perigo vira a
  norma; `material` passaria a precisar de um terceiro valor (`parcial`) e de
  um **contador no cabeçalho** mantido em **todo** `inserir`/`atualizar` — no
  laço quente, para servir a uma operação rara;
- **a frase «a tabela está cifrada» deixa de ser verdadeira** por tabela; vira
  verdadeira por linha, e quem lê o painel não sabe qual;
- ordem de digitação: preservada (nada se move); queda: por slot (CRC e
  `.bkp`, como hoje); replicação: indiferente, a réplica sela por conta
  própria.

**Veredito:** não. Muda o formato do futuro para tornar preguiçosa uma migração
que continuaria sendo necessária para o passado.

---

## 4. Os «nãos» deste parecer, em lista

1. **Não** à migração automática — no arranque, no `config` a quente, ou por
   presença em `cifra.tabelas`. Guarda nova entra pedida.
2. **Não** ao «no lugar». O formato não deixa (slot e cabeçalho crescem) e a
   casa já recusou pela queda.
3. **Não** a (a): não há declaração onde a recusa caiba sem quebrar um leitor
   legítimo ou impor guarda.
4. **Não** a (c): não alcança o dado que já existe e institucionaliza o estado
   misto.
5. **Não** a um byte novo no cabeçalho do `.reg` para (b): `versão` +
   `material` + a geometria já respondem tudo.
6. **Não** a um `criptografar` que não sele o `.memo`/`.bin` nem vire o volume
   dos diários: protegeria a faixa e deixaria o memo e a imagem seguinte em
   claro.

---

## 5. Qual mudança de formato entra CEDO — e não é o 268

**Para (b): nenhuma.** É o achado mais barato deste parecer.

Duas entram cedo, e as duas ficaram visíveis ao desenhar (b):

**5.1 O envelope da chave (`SEGURANCA.md` §11.5) — versão 6 do `.reg`.** Hoje
a chave da tabela **é** a derivada da senha (mais o sal do arquivo). Com isso,
`descriptografar` + `criptografar` seria a **única rotação de senha** que
existe — uma reescrita inteira por tabela —, e a réplica de tabela com externa
marcada precisa da senha **e do sal** da origem (§11.8). Com uma chave de
tabela sorteada e envelopada pela mestra (`AEAD(mestra, chave_da_tabela)`, 48
bytes), rotacionar é regravar **48 bytes por arquivo**, e a chave da tabela
pode ir à réplica sem a senha mestra. O custo de formato: o material passa de
40 para ≈ 88 bytes; os bytes 168..188 têm só **20** livres; o cabeçalho vai de
192 para **256**, e o `data_offset` anda 64. **Impacto de migração:** cada
tabela da 5 precisa de **uma cópia sequencial** (`reescrever_volume`, slots
byte a byte — 427 MiB/s, 5,53 s por 1,1 GiB, o número da §4.14), porque a
folga de 63 bytes do `regravar_esquema` nunca comporta +64. **Hoje esse custo
é zero**: 0 arquivos da 5 no repositório. Cada tabela da 5 criada antes disso
paga uma cópia depois — e a primitiva que paga é a mesma (b).

**5.2 Uma marca «selado/claro» por externo na imagem da linha (`.log`).** A
imagem carrega o externo «como está no bloco» sem dizer se está selado
(`table.rs:3814-3818`, `3843-3855`). Sem a marca, uma réplica cujo material
diverge do da origem guarda cifra como dado (`Bin`) ou pára com um erro que
fala de UTF-8 quando o problema é chave. É **um byte por externo** no formato
do evento do `.log` — append-only, logo o `.log` velho não se reescreve e o
leitor tem de aceitar os dois. Entra cedo pelo mesmo motivo do `PSCH`: depois
vira leitor de dois formatos para sempre.

---

## 6. O que o pedido 268 e o §13.6 dizem, e o formato desmente ou completa

| dizia | o formato/código diz |
|---|---|
| «papel C, **cabeçalho** do `.reg`» | o cabeçalho já sabe (versão 4/5, sinais, sal, prova); falta a operação |
| «declarar em `cifra.tabelas` não cifra o que já está gravado» | a lista não cifra **nada**, nem o novo: só o Profiler a lê; quem cifra é a marca por coluna + cofre, na criação |
| «reescrever o `.reg` slot a slot» (sem dizer onde) | ao lado e com `rename`; no lugar é impossível (slot +16, cabeçalho +64) e recusado pela queda |
| «a queda no meio precisa da mesma resposta do `ALTER`» | e a resposta já serve sem byte novo: `data_offset` da 5 = da 4 + 64, a geometria distingue |
| `.log`/`.trash`/`.reason` e `.ndx` nomeados | faltou o `.memo`/`.bin` (bloco claro fica, sem compactação) e o volume corrente dos diários (continua recebendo imagem em claro) |
| «o dado mentindo sobre si mesmo» (briefing) | o disco diz a verdade em três leitores; o que promete mais é o **nome** `cifra.tabelas` |

---

## 7. Fontes (todas lidas nesta data)

- `/home/user/adrianoboller/phxsql/docs/FORMATO.md` — §1 (linhas 84–193), §1.1
  (195–293), limites (2280–2307).
- `/home/user/adrianoboller/phxsql/docs/SEGURANCA.md` — §11.1 (1325–1345),
  §11.4 (1485–1503), §11.5–11.8 (1581–1719), §13 (2023–2152).
- `/home/user/adrianoboller/phxsql/docs/DESEMPENHO.md` — §4.14 (1680–1761).
- `/home/user/adrianoboller/phxsql/docs/PENDENCIAS.md` — linha 292 (pedido 268).
- `/home/user/adrianoboller/phxsql/crates/phxsql-store/src/reg.rs` — 1–135,
  254–479, 539–629, 883–933, 1081–1130, 1208–1402, 1644–1823, 2253–2259,
  2296–2464.
- `/home/user/adrianoboller/phxsql/crates/phxsql-store/src/cofre.rs` — 1–140,
  229–252, 331–394.
- `/home/user/adrianoboller/phxsql/crates/phxsql-store/src/table.rs` —
  3804–3855, 3921–3935, 3988–4083.
- `/home/user/adrianoboller/phxsql/crates/phxsql-store/src/log.rs` — 20–64,
  448–504. `blob.rs` — 1–15, 296.
- `/home/user/adrianoboller/phxsql/crates/phxsql-store/tests/cifra-dos-dados.rs`
  — 133–179 (rodado), 264–298.
- `/home/user/adrianoboller/phxsql/crates/phxsql-server/src/config.rs` —
  1315–1339 (a marca `DIVIDA: #268` está na **1338**), 1406–1437, 1494–1509.
- `/home/user/adrianoboller/phxsql/crates/phxsql-server/src/profiler.rs` —
  15–20, 257–263, 464–471, 722–754.
- `/home/user/adrianoboller/phxsql/crates/phxsql-server/src/servidor.rs` —
  4584–4588, 10592, 16332, 21515–21519.
- `/home/user/adrianoboller/phxsql/crates/phxsql-server/src/replica.rs` —
  1–57.
- `/home/user/adrianoboller/phxsql/crates/phxsql-store/examples/tabela-marcada-nasceu-em-claro.rs`
  — 192–207.
- Medições desta data: contagem de `.reg` por versão (66/66/0) e cabeçalho do
  `precos.reg`, por leitor Python na área de rascunho; teste isolado
  (12,24 s compilar, 1,06 s rodar, 1 ok).
