# Parecer do DBA (papel C) — pedidos 372 e 255

**Data:** 23/09/2026 · **Papel:** C, DBA sênior · **Alcance:** formato em disco,
garantias de dado, migração. **Não** escrevi código de produção, **não** comitei,
**não** rodei `cargo build/test` (disco em 2,3 GB livres, 94% cheio — `df -h /`).
Tudo abaixo é leitura de fonte com `sed`/`grep`, a corrida de bancada já gravada,
e **uma** medição própria de syscall, declarada como teto.

**Parte do que já estava escrito:** `docs/propostas/revisao-sec-saidas-de-segredo.md`
(achados A2, A4 e o item «`config.json` e `dblink.json` com segredo em claro, com
a alternativa `_env` para cada um», linhas 30-33, 270-307, 430-431) e
`docs/propostas/segredo-newtype-vs-conferidor.md:155,197-198`. Não recomeço: o que
falta ali é a decisão de **chave**, de **formato** e de **migração**, que é este
documento.

---

## PARECER 1 — pedido 372 · o `dblink.json` grava a senha do destino em claro

### 1.0 O que o pedido diz, e duas correções de citação

O pedido (`docs/PENDENCIAS.md:393`) aponta `dblink/mod.rs:299`. **A citação
envelheceu**: a linha 299 hoje é a *leitura* (`de_json`), e a escrita está em
**377**. E o pedido nomeia **um** segredo; o arquivo grava **dois**.

Os sítios que põem segredo no `dblink.json`, hoje:

| # | onde | o quê |
|---|---|---|
| 1 | `crates/phxsql-server/src/dblink/mod.rs:376-377` | `campos.push(("senha", Json::texto_de(&self.senha)))` — a senha do outro banco |
| 2 | `crates/phxsql-server/src/dblink/mod.rs:383-384` | `campos.push(("token_remoto", Json::texto_de(&self.token)))` — **o portão 1 do outro PhxSql** |
| 3 | `crates/phxsql-server/src/dblink/mod.rs:808-814` | `Registro::gravar` serializa **todas** as ligações e chama `config::gravar_privado` — é por onde os dois vão ao disco |

O sítio 2 é o que um `grep senha` não acha, e o motivo está escrito no próprio
campo: ele se chama `token_remoto` **de propósito** (`dblink/mod.rs:151-158`),
porque `token` já é o portão 1 *deste* servidor. É o mesmo nome escolhido de
propósito que ficou dois dias fora da lista de segredos do profiler
(`crates/phxsql-server/src/segredos.rs:5-16`). **E ele é o pior dos dois**: quem
tem o token alcança a porta de dados do outro servidor **sem usuário nenhum**
(`dblink/mod.rs:146-150`).

O que **já está certo** e não se mexe: o arquivo nasce `0600` desde o primeiro
byte, por troca atômica (`config.rs:2052-2070`), com prova contra o sistema
operacional (`dblink/mod.rs:1421-1443`); `para_json` nunca devolve a senha
(`:412`, `:448-458`); o `Debug` é escrito à mão e desestrutura sem `..`, de modo
que campo novo **para de compilar** (`:209-265`); e o job recusa pedido com
segredo (`segredos.rs:50-80`).

### 1.1 (c) — `senha_env` resolve o caso sozinho? **Resolve o ARQUIVO. Não resolve o PEDIDO.**

Resolve o arquivo, e isso é fato: com `senha_env` preenchido, `para_disco` grava
o **nome da variável** e não o valor (`dblink/mod.rs:376-379`), e o mesmo vale
para o token (`:381-384`). A tela já oferece o campo, com o texto explicando
(`crates/phxsql-server/ui/index.html:10836-10840`, `:10913`, `:10950`).

Não resolve o pedido, porque **372 é sobre o padrão**, e o padrão continua sendo
o claro. E há três buracos medidos que qualquer decisão tem de levar junto:

- **C1 · Variável ausente vira senha VAZIA, em silêncio.**
  `dblink/mod.rs:305` — `std::env::var(&senha_env).unwrap_or_default()`; o irmão
  do token em `:311`; os irmãos no `config.rs:1343` (relé de e-mail) e
  `config.rs:1887` (chave privada do fio). Um erro de digitação no nome da
  variável produz senha vazia; o erro que aparece vem do **outro banco**
  («access denied»), e manda procurar credencial errada — exatamente a família
  de erro que o batismo de `token_remoto` existe para evitar. Pior: contra um
  destino cujo usuário tem senha vazia, **autentica**.
- **C2 · A tela oferece meia credencial.** O formulário de ligação tem
  `fSenha` e `fSenhaEnv` (`ui/index.html:10911-10914`) e **não tem** campo de
  `token_remoto` nem de `token_remoto_env` — as únicas ocorrências de
  `token_remoto` no `index.html` são da tela de **replicação** (`:12888`,
  `:13066`, `:13123`, `:13320`). Ou seja: o segredo mais forte dos dois só entra
  por pedido direto ou editando o arquivo à mão, e só sai do arquivo por lá.
- **C3 · `senha_env` não apaga o que já foi gravado.** `gravar_privado`
  (`config.rs:2052-2070`) escreve um `.tmp` novo e **renomeia**: os blocos do
  arquivo antigo são desligados, nunca sobrescritos. A senha em claro que já
  esteve no disco continua recuperável no espaço livre — e em toda cópia de
  backup já tirada. **O que tira a senha do disco é trocar a senha no outro
  banco**, e isso vale igual para a proposta de cifrar.

**Veredito (c):** `senha_env` é hoje a **única** coisa que tira o segredo do
arquivo com ganho real, e é recusa medida da proposta de cifrar «para resolver o
mesmo problema». Mas ela é escolha de quem opera, e o pedido cobra o padrão.

### 1.2 (a) — Qual chave cifra o `dblink.json`?

**NÃO à estática X25519 do fio** (`config.rs:1813-1900`), por dois motivos, e o
primeiro sozinho já basta:

1. **Ela é identidade, não chave de repouso.** É o pino que todo cliente e toda
   réplica esperam (`dblink/mod.rs:186-198`, `config.rs:1926-1963`), e trocá-la é
   procedimento previsto — o próprio código avisa que uma estática que muda
   «quebra o pino de todo cliente» (`config.rs:1952-1958`). Se ela também abrir o
   `dblink.json`, **rotacionar a identidade passa a perder credencial**. Chave de
   identidade e chave de repouso têm ciclos de vida diferentes; amarrá-las é
   criar uma migração onde hoje há um procedimento.
2. **Ela mora no mesmo diretório, com a mesma permissão.** `chave-do-fio.hex`
   resolve ao lado do `config.json` (`config.rs:1872` + `caminho_da_chave`
   `:1918-1920`), 0600 (`config.rs:2013-2035`), e o `dblink.json` resolve ao lado
   também (`config.rs:3599`, `:3640`). É a chave debaixo do capacho.

**NÃO à `cifra.senha` do cofre como ela está** (`config.rs:1525-1527`): ela mesma
está em claro no `config.json` ao lado, e o cofre **escreve isso** no próprio
cabeçalho — «Nao protege contra quem le o `config.json` desta maquina: quem le o
`config.json` tem a senha» (`crates/phxsql-store/src/cofre.rs:29-34`). E
`cifra.ligada` nasce `false` (`config.rs:1532`): amarrar o dblink a ela deixa o
padrão em claro do mesmo jeito.

**O que serve, e já existe inteiro — zero dependência nova:**

| peça | onde | o que resolve |
|---|---|---|
| PBKDF2-SHA256 | `crates/phxsql-core/src/cifra.rs:440` (`chave_de_senha`) | senha mestra → chave de 32 bytes |
| XChaCha20-Poly1305 (RFC 8439, com vetores) | `crates/phxsql-core/src/cifra.rs:391`, `:410` | selar/abrir com AAD |
| `cofre::Material` — sal 16 B + iterações + prova 16 B | `crates/phxsql-store/src/cofre.rs:319`, `:475`, `:521`, `:545`; `SAL_LEN` `:52`, `ITERACOES_PADRAO=210_000` `:56`, `ITERACOES_MINIMAS=10_000` `:57` | recusa a senha errada **na abertura**, e não na primeira ligação |

**O que falta não é criptografia — é um lugar para a chave.** A chave mestra tem
de vir de **fora do conjunto que a cópia carrega**: variável de ambiente, ou
arquivo em caminho/montagem declarados fora da pasta do banco. É o modelo de
ameaça que o próprio cofre escreve (`cofre.rs:31-34`): o que se protege é **o
arquivo copiado** — disco levado, backup vazado, cópia noutra máquina. Cifrar com
chave da mesma pasta protege contra **ninguém**.

E uma exigência de formato que não é detalhe: o envelope tem de amarrar o **nome
da ligação** no AAD (`Material::selar(nonce, aad, claro)`, `cofre.rs:521`). Sem
isso, mover o envelope da ligação A para a linha da ligação B dentro do mesmo
arquivo é edição de texto — e o arquivo é editável à mão por desenho.

### 1.3 (b) — É mudança de formato? **É, e por isso entra CEDO.**

Hoje o `dblink.json` **não tem versão nenhuma**: `Registro::abrir` aceita
`{"dblink":[…]}` ou uma lista crua (`dblink/mod.rs:725-751`), e `Definicao::de_json`
ignora campo desconhecido (`:296-357`). Mudar isso custa pouco hoje e vira
migração depois — é a pétrea de sempre, e é o motivo de o parecer sair agora.

**Forma recomendada — material UMA vez no arquivo, envelope por ligação:**

- nível de arquivo: `"formato": 2` e
  `"cifra_do_cadastro": {"sal": <hex 16 B>, "iteracoes": 210000, "modo": "aead", "prova": <hex 16 B>}`;
- por ligação: `"senha_cifrada"` e `"token_remoto_cifrado"` (hex), cada um com
  nonce próprio, **mutuamente exclusivos** com `"senha"`/`"token_remoto"`, AAD =
  nome da ligação + nome do campo.

**Por que um material só, com número:** uma derivação PBKDF2-SHA256 de 210.000
iterações custa **290,3 ms medidos** nesta casa (`docs/SEGURANCA.md:2708`; 298 ms
em §11.4, `:1862`). `Registro::gravar` reescreve **todas** as ligações a cada
`salvar` (`dblink/mod.rs:808-814`) e `Registro::abrir` lê todas a cada arranque.
Sal por ligação faria **10 ligações custarem 2,9 s por arranque e 2,9 s por
salvar**. Com material único é uma derivação; com a chave entregue já derivada
(hex de 32 bytes na variável), é zero.

**Como um `dblink.json` escrito ANTES continua sendo lido:** sem `formato` e sem
`senha_cifrada`, o leitor cai no caminho de hoje (`:301-306`, `:307-311`) e **nada
muda**. A decisão mora no **campo presente**, não numa versão que o leitor velho
precise entender — é o mesmo contrato do byte 52 do `.ndx` («arquivo escrito
antes tem zero ali, e zero quer dizer limpo; não há migração», `ndx.rs:659-662`,
`docs/FORMATO.md:1027`) e do `PSCH`, que volta com o que foi gravado nele.
**Compatibilidade para trás é de graça aqui, e por isso não se negocia.**

Migração para a frente: acontece **na primeira gravação com chave disponível** —
pedida, não imposta.

### 1.4 (d) — O que quebra para quem já usa

1. **Binário antigo lendo arquivo novo: senha VAZIA em silêncio.** `de_json` faz
   `j.texto_ou("senha", "")` (`:303`); um `senha_cifrada` que ele não conhece
   simplesmente some, e a ligação passa a apresentar senha vazia ao outro banco.
   O erro aparece **do outro lado** e manda procurar no lugar errado.
   **Downgrade deixa de ser suportado no instante da primeira migração**, e isso
   tem de estar no CHANGELOG e no MANUAL, não só no código.
2. **Chave ausente não pode derrubar o servidor — e hoje derrubaria.**
   `crates/phxsql-server/src/servidor.rs:1265`:
   `let dblink = crate::dblink::Registro::abrir(&config.dblink)?;` — com `?`. Um
   `dblink.json` ilegível **aborta o arranque** hoje. Se a leitura passar a
   depender de uma variável de ambiente, esquecer a variável deixa de ser «o
   DbLink não funciona» e vira «o motor de dados não sobe». **NÃO.** A ligação
   que não abre nasce **trancada**, com o motivo nomeado, e recusa só as
   operações dela. O DbLink é acessório; o motor não é.
3. **Quem já usa `senha_env`:** nada muda — o arquivo já não tem segredo.
4. **Quem edita o `dblink.json` à mão perde o caminho** para os campos cifrados —
   e, por C2, **é o único caminho** para pôr o `token_remoto`. Cifra sem
   reposição de caminho (tela ou comando) troca um vazamento por uma porta
   fechada na cara de quem opera.
5. **O claro que já está no disco continua lá** (C3). Quem ligar a cifra e não
   trocar a credencial no destino trocou um vazamento por uma crença.

### 1.5 O **NÃO**, o tradeoff, e o que sobe ao dono

> **NÃO a cifrar o `dblink.json` com chave que mora no mesmo diretório.** Isso não
> protege contra ninguém e **anuncia** proteção — é a família do
> `encryption_exigida` e do `recursos.cache_paginas`, já paga aqui
> (`PENDENCIAS.md:390`, pedido 366; `servidor.rs:5886`). Cifra do cadastro só
> entra com a chave vinda **de fora do conjunto copiado**.

**Tradeoff, nos dois sentidos:**

| caminho | compra | custa |
|---|---|---|
| **Cifrar com chave mestra externa** | protege a **cópia** (disco levado, backup vazado) — o modelo de ameaça escrito em `cofre.rs:31-34` | mudança de formato; downgrade quebrado em silêncio; o arranque passa a depender de uma variável; edição à mão perdida; 290,3 ms de derivação; **e não apaga o claro já gravado** |
| **`senha_env`/`token_remoto_env` + aviso de arranque** | a **mesma** proteção contra a cópia, porque o segredo não está no arquivo; zero formato, zero migração, não quebra ninguém | depende de quem opera **escrever a decisão** — não vira «por padrão» |

**O que fecha 372 como pétrea sem inventar chave**, e é o que eu recomendo entrar
primeiro (não escrevo o conserto; nomeio a forma):

1. **Aviso de arranque no molde já provado das saídas em claro**
   (`config.rs:3936-3960`): fala com quem **não registrou decisão**, e **cala**
   para quem escreveu `senha_env`. Aviso perpétuo em instalação decidida gasta a
   confiança do aviso verdadeiro — a doutrina já está escrita ali e vale igual.
2. **A tela oferece `token_remoto` e `token_remoto_env`** (C2). Enquanto não
   oferecer, metade da credencial não tem caminho de saída do arquivo.
3. **Conserto do `unwrap_or_default` silencioso** (C1), nos quatro sítios —
   `dblink/mod.rs:305`, `:311`, `config.rs:1343`, `config.rs:1887`. Variável
   declarada e ausente é **erro nomeado**, nunca senha vazia. Este é o irmão que
   a pétrea manda procurar, e ele está fora do arquivo que motivou o pedido.

**Sobe ao dono** (e é o único ponto deste parecer que sobe): escolher entre
(i) aviso + `_env` e (ii) cifra com chave mestra externa. Não é comportamento de
banco que o help dos quatro responda — é **implantação e produto**: muda o
procedimento de instalação, o de backup e o de recuperação de desastre. Os dois
desenhos estão acima; o meio-termo (cifrar com chave ao lado) está **recusado**.

---

## PARECER 2 — pedido 255 · tomada no meio de `BULKINSERT`/`reindexar`

### 2.1 (a) — Qual garantia a recusa protege? **Não é precaução cega.**

A marca de sujo (byte 52) sobe **antes** de a primeira página suja existir e só
cai depois de **todas** irem ao disco (`crates/phxsql-store/src/ndx.rs:957-964`,
`:835-838`; leitura em `:659-662`; a ordem está escrita em
`docs/FORMATO.md:1012-1017`). Quem abre com ela levantada sabe que a árvore pode
ter **chave faltando**.

O `.reg` **não** está em dúvida, e isso está medido: na corrida `bulk#1.5` de
`bancada/tomada/resultados.json` (16/09/2026 07:36 UTC) o desfecho foi
`conf=1712 reg=1713` e, depois do `reindexar`, `achadas=1713`. O dado estava
inteiro; quem estava atrasado era **só** o índice.

Logo, o que a recusa protege é a **resposta do índice** — e ela é o chão de duas
pétreas:

- **Unicidade.** `NdxFile::inserir` decide duplicata perguntando ao próprio
  índice (`ndx.rs:1162-1163`: `if self.descritor(idx)?.unico && self.existe(idx, chave)?`).
  Índice com chave faltando responde «não existe» e **deixa entrar a repetida** —
  e o dano vai para o `.reg`, que não se reescreve.
- **Integridade referencial, nos dois lados.** `conferir_fks` responde «existe
  este pai?» com `mae.buscar(...)` (`crates/phxsql-store/src/table.rs:1797-1799`).
  Índice atrasado responde «não existe» para pai que existe — recusa gravação
  legítima — e, do outro lado, responde «ninguém aponta para esta linha» para mãe
  que **tem** filha. Isso é **matar o pai que tem filhos, em silêncio**: a pétrea
  primordial.

E o portão está no lugar certo, de propósito: mora no `descritor`
(`ndx.rs:1145-1156`), e não espalhado por `inserir`/`buscar`/`varrer`/`remover` —
o mesmo raciocínio do portão de permissão, porque a que alguém esquecesse viraria
uma busca respondendo errado calada.

> **A recusa está CERTA e não se mexe nela.** O pedido 255 é sobre a
> **RECUPERAÇÃO e o AVISO** — não sobre parar de recusar. O título dele
> («deixa a tabela recusando até um `reindexar` manual») lê como se a recusa
> fosse o defeito; **não é**, e isso tem de ser corrigido no texto do pedido,
> senão o conserto errado fica autorizado pelo enunciado.

### 2.2 (b) — A marca alcança a tabela que estava no meio do `BULKINSERT`? **NÃO.**

- `recuperar` varre o diretório de cada base atrás de `transacao_*.tx`
  (`crates/phxsql-server/src/transacao.rs:1429-1452`) e só completa o que está
  **nomeado numa marca** (`completar`, `:1482-1490`; o `reindexar` da recuperação
  em `:1507-1518`).
- **A única chamada de produção de `gravar_marca` está dentro do COMMIT**
  (`crates/phxsql-server/src/servidor.rs:15572`). Portanto `BULKINSERT` fora de
  transação, `inserir_lote` e `reindexar` **não criam marca nenhuma**, e a
  recuperação nunca chega a eles. É o mesmo alcance do pedido 172 (a filha da
  cascata, `transacao.rs:1520-1533`) por outro caminho: a máquina existe e não
  alcança a tabela que ia consertar.

**Medido**, `bancada/tomada/resultados.json`, corrida de 16/09/2026 07:36 UTC:

| ponto | quedas | `.ndx` sujo | % |
|---|---:|---:|---:|
| `tx_aberta` (transação aberta, sem COMMIT) | 72 | 0 | 0% |
| `bulk` (BULKINSERT linha a linha) | 72 | 1 | 1,4% |
| `bulk_lote` (`inserir_lote` de 500) | 72 | 35 | 48,6% |
| `reindexar` (10.000 linhas) | 120 | 111 | 92,5% |

**147 de 264 quedas nesses três pontos (55,7%) deixam índice sujo que ninguém
conserta sozinho.** E a prova de que o caminho da marca funciona está na mesma
corrida: as duas quedas de `tx_em_bulk` com byte 52 = 1 (`tx_em_bulk#3.22`,
`#3.23`) saíram com `"indices_reconstruidos": 1` no relatório — tinham marca.

**O silêncio também está medido.** O gancho da saúde do disco dispara por
`codigo == 5001` (`crates/phxsql-server/src/saude_do_disco.rs:20-24`;
`servidor.rs:221`, `CODIGO_DE_ES`), e o erro do índice atrasado é
`PhxError::Corrompido` = **1001 / SP000010**
(`crates/phxsql-core/src/error.rs:146`, `:251`). A fila e o carteiro do #249
existem e **este erro não os alcança**.

**É aí que o conserto entra:** na varredura que a recuperação **já faz**.
`recuperar` já chama `read_dir` no diretório de cada base
(`transacao.rs:1440-1452`) e descarta tudo que não é `.tx`. Notar os `.ndx`/`.fts`
do **mesmo** `read_dir` custa zero de travessia de diretório — e é o caminho que
motivou, não um irmão.

### 2.3 (c) — Como a recuperação honra a ordem de digitação

**Honra por construção, e é preciso dizer por quê, não só afirmar:**

- `Table::reindexar` (`crates/phxsql-store/src/table.rs:5858-5892`) **não toca no
  `.reg`**. Recria o `.ndx` (`NdxFile::criar` trunca, `:5861`) varrendo
  `self.reg.proximo_ativo(rowid)` em **ordem crescente de rowid** (`:5872-5880`) e
  reconstrói o `.fts` junto (`:5886`). O rowid **é** o slot; o slot nunca é
  reaproveitado; a reconstrução **deriva** do `.reg` e não o edita.
- É a mesma direção única que a recuperação de transação já segue, e pelo mesmo
  motivo escrito lá: «desfazer exigiria devolver slots já gravados, e o `.reg`
  nunca reaproveita slot — a regra que decide tudo neste desenho»
  (`transacao.rs:1417-1427`).
- **E fica escrito o NÃO preventivo:** nenhuma recuperação de índice pode ser
  «aparar o `.reg` pelo índice». Na corrida `bulk#1.5` a linha 1713 estava no
  `.reg` e não no `.ndx`; o certo é o índice **passar a apontar** para ela (foi o
  que o `reindexar` fez: `achadas=1713`), e nunca o `.reg` perder o slot para
  «casar» com o índice. Proposta futura que sugira truncar o `.reg` para fechar a
  conta **é recusada aqui, com este caso**.
- E a marca não se limpa por atalho: `fechar` recusa baixá-la num arquivo aberto
  sujo (`ndx.rs:1101-1108`) — senão bastaria abrir e fechar para o defeito ficar
  invisível.

### 2.4 (d) — Custo: o que medi, o que não medi, e com qual comando

**O que a varredura pagaria:** `open` + `pread(128)` + `close` por `.ndx` **e** por
`.fts` — os dois carregam a marca (`table.rs:5842-5848`; perguntar só ao `.ndx`
deixaria a queda que suja o índice de texto passar calada, que é o irmão já pago).

**Correção do pedido:** ele diz «um `open`+`read` de 64 bytes por `.ndx`»
(`PENDENCIAS.md:279`). O cabeçalho tem **128** (`ndx.rs:67`, `CAB_LEN`), e o CRC
cobre `cab[..124]` com o valor em 124 (`ndx.rs:647-650`). Ler 64 pega o byte 52 e
**não permite conferir o CRC** — a varredura acreditaria num cabeçalho que pode
estar corrompido. **São 128 bytes.**

**Medido por mim, 23/09/2026 — e é TETO, não o número em Rust:** 1.850 arquivos
reais da árvore, cinco passadas de `open(O_RDONLY)+pread(128,0)+close`, mediana
**5,86 ms** → **3,17 µs por arquivo**. O laço vazio equivalente custa 0,006 µs por
item e três chamadas triviais 0,036 µs, então o número é **syscall**, não
interpretador. Extrapolado: **10.000 tabelas = 32 ms; com `.fts` em todas, 63 ms.**
Cache quente, ext4, contêiner com 2,3 GB livres.

**O que esse número NÃO cobre, e é justamente o caso do pedido: cache frio** — o
arranque depois de uma queda da máquina. O comando existe e eu sou root aqui
(`/proc/sys/vm/drop_caches` presente, `--w------- root`), e **não rodei de
propósito**: derrubar o cache do host castiga toda frente que esteja compilando
agora, e a lei do zelador é não estragar trabalho vivo de outro processo. O
roteiro honesto é o que o próprio pedido pede:

```bash
# 1) um diretório com milhares de tabelas
python3 bancada/carga/...            # criar N tabelas vazias (N = 1.000, 10.000)
# 2) cache frio, com a máquina ociosa (conferir bancada/esta-medindo.sh antes)
sync; echo 3 > /proc/sys/vm/drop_caches
# 3) o arranque, contando syscall por syscall
strace -c -f -e trace=openat,pread64,read,close ./target/release/phxsqld --base <dir>
```

**O custo do conserto quando a marca está levantada:** `reindexar` custa
**0,31 s por milhão de chaves** desde a construção em lote
(`docs/FORMATO.md:1021-1022`). Tabela de 10 milhões ≈ 3,1 s.

**E por isso reconstruir no ARRANQUE é mais barato do que parece:**
`op_reindexar` toma a **trava global de dados** (`servidor.rs:24584-24585`), e a
recuperação roda **antes de a porta abrir** (`servidor.rs:1235`). Reconstruir ali
atrasa a subida; reconstruir tarde, no primeiro toque, segura a trava global
**com clientes conectados**. A escolha medida é atrasar a subida.

### 2.5 O desenho que eu aprovo, graduado — e o que recuso

O dono parou o item porque «reconstruir sozinho no arranque muda o tempo de
subida» (`PENDENCIAS.md:279`). A graduação respeita isso:

1. **Varrer e DIZER, sempre.** Custo medido acima (32–63 ms em 10.000 tabelas,
   cache quente; falta o frio). O relatório do arranque já existe e já sabe calar
   quando não há nada (`transacao.rs:1365-1412`, e o portão `houve()` em `:1360`) — acrescentar «N índices ficaram
   para trás numa queda: …» é o mesmo molde. Para chegar ao carteiro do #249, o
   gancho precisa passar a ver **1001**, e não só 5001 (`saude_do_disco.rs:20-24`).
2. **Reconstruir sozinho: pedido, não imposto.** Interruptor no `config.json`
   nascendo **desligado** — mil tabelas grandes virariam uma subida de minutos
   sem ninguém ter pedido. É o mesmo padrão do `"verificar": false` e da cifra do
   cofre: guarda nova entra pedida. E o campo tem de ter **leitor no dia em que
   nasce**, senão é `recursos.cache_paginas` de novo.
3. **Recuso: reconstruir em silêncio.** Recuperação que não conta o que fez é a
   mesma doença do índice atrasado em silêncio — e o relatório já tem o contador
   certo (`indices_reconstruidos`, `transacao.rs:1380-1386`).

**Migração de formato para o 255: NENHUMA.** A varredura só **lê** o byte que
existe desde a 0.18.0 (`ndx.rs:659-662`), e arquivo anterior tem zero ali, que
quer dizer limpo (`docs/FORMATO.md:1027`). O interruptor é campo novo no
`config.json`, ausente = desligado. **Zero migração** — e é por isso que este
pedido é barato e o 372 não é.

---

## Resumo — os dois NÃOs, em uma linha cada

- **372:** **NÃO** a cifrar o `dblink.json` com chave que mora no mesmo
  diretório — protege contra ninguém e anuncia proteção. Se cifrar, o formato é
  `"formato": 2` + material único + envelope por ligação com AAD no nome, e a
  chave vem de fora do conjunto copiado; e a chave ausente **não pode** derrubar
  o arranque (`servidor.rs:1265`). O caminho barato e que não quebra ninguém é
  `_env` + aviso no molde de `config.rs:3936-3960` + os dois irmãos (tela sem
  `token_remoto_env`; `unwrap_or_default` silencioso em quatro sítios).
- **255:** **NÃO** a «parar de recusar» — a recusa protege unicidade e a pétrea
  primordial, e está medida como certa (`bulk#1.5`: `.reg` íntegro, índice
  atrasado). O defeito é a **recuperação não alcançar** o que não tem marca
  (147/264 quedas = 55,7%) e o operador **não ser avisado** (erro 1001 não chega
  ao gancho 5001). Varrer 128 bytes por `.ndx`/`.fts` no `read_dir` que a
  recuperação já faz; avisar sempre; reconstruir só se pedido; e a reconstrução
  deriva do `.reg` em ordem de rowid, sem jamais editá-lo.
