# PhxZip na web — o contrato HTTP

Pedidos **454** e **455** (`docs/PENDENCIAS.md`). O PhxZip é o 7-Zip em Rust da
casa (`docs/7ZIP.md`, crate `crates/phxzip`); este documento é o contrato entre a
**tela** (`crates/phxzip-web/ui/`, papel E) e o **servidor web** (frente seguinte,
sobre a API pública da biblioteca). A tela foi escrita e exercitada contra um
servidor falso que segue este contrato à letra (`testes-web/phxzip/`); o servidor
de verdade segue o mesmo contrato, e onde os dois discordarem **o contrato manda**.

O que o contrato não diz, o servidor não inventa: rota nova, campo novo ou erro
novo entram **aqui primeiro**.

## 1. O servidor

| | |
|---|---|
| endereço | **só `127.0.0.1`**. Não há opção de escutar em outra interface: a porta extrai arquivos e aceita senha, e ela nasce presa à máquina (pedido 454) |
| porta | constante única no servidor, `PORTA_PADRAO` (sugestão: **7700** — livre das quatro do PhxSql: 5000, 5001, 6000, 7000), configurável na linha de comando. A tela não sabe a porta: usa caminho relativo |
| HTTP | 1.1, uma resposta por pedido, `Content-Length` sempre (sem `chunked` na ida nem na volta) |
| disco | **o servidor não lê nem escreve caminho escolhido pelo navegador.** Nenhuma rota recebe caminho de disco. O que entra chega no corpo; o que sai, sai como download |
| estado | **nenhum entre pedidos.** Cada pedido traz o pacote inteiro e a senha, e ao responder o servidor não guarda nada — nem o pacote, nem a chave derivada. O preço está dito em §7 |
| HTTP vem de onde | do **mesmo motor** do PhxSql (`http.rs`/`fio.rs`), nunca de uma cópia (pétrea «função e comando vêm do mesmo motor»; pedido 454). Se reusar pedir extração, extrai-se |

## 2. As rotas

| método | rota | corpo que entra | o que sai |
|---|---|---|---|
| `GET` | `/` | — | `index.html` |
| `GET` | `/phxzip.css`, `/phxzip.js`, `/fonte/exo2-latin.woff2` | — | os estáticos |
| `GET` | `/api/estado` | — | JSON: versão, limites, níveis, formatos, idiomas (§4) |
| `GET` | `/api/idiomas?idioma=<Coluna>` | — | JSON: os textos da tela já resolvidos (§5) |
| `POST` | `/api/compactar` | envelope: cabeça + conteúdos (§3) | o pacote, como download |
| `POST` | `/api/listar` | envelope: cabeça + pacote | JSON: entradas e blocos |
| `POST` | `/api/testar` | envelope: cabeça + pacote | JSON: veredito da integridade |
| `POST` | `/api/extrair` | envelope: cabeça + pacote | uma entrada crua, ou `.tar` com várias |

**A Exo 2 vem desta porta, e não de uma CDN** (a tela do PhxSql a pede ao
Google Fonts): a tela abre sem rede, o CSP fica em `'self'`, e ninguém fora da
máquina fica sabendo que ela abriu. É a face latina variável (400–700, 40.896
bytes, que cobre os seis idiomas), sob a SIL OFL 1.1 — o `fonte/OFL.txt` vai
junto no pacote do pedido 455, como a licença pede.

**Os estáticos são uma lista fechada**, embutida no binário por `include_bytes!` —
qualquer outro caminho é `404 ROTA_INEXISTENTE`. Não existe servir «a pasta
`ui/`»: é assim que não existe `../` para pedir. O `textos.json` **não** é
servido cru (§5). Método errado numa rota que existe é `405 METODO_HTTP`, com
`Allow`.

### 2.1 `GET /api/estado`

```json
{
  "ok": true,
  "produto": "PhxZip",
  "versao": "0.19.0",
  "limites": {
    "envio": 268435456,
    "cabeca": 1048576,
    "entrada": 268435456,
    "bloco": 268435456,
    "cabecalho": 16777216,
    "espiar": 262144,
    "avaliar_json": 16777216,
    "simultaneas": 2
  },
  "niveis": ["armazenar", "lzma2"],
  "formatos": ["7z", "phz"],
  "idiomas": ["Portugues", "Frances", "Ingles", "Italiano", "Alemao", "Espanhol"]
}
```

- `limites.entrada`, `bloco` e `cabecalho` saem de `phxzip::Limites::default()`;
  `idiomas` sai de `IDIOMAS` (§5). **Nenhum número desta resposta se digita no
  servidor**: cada um vem da constante que o motor já usa, para a tela nunca
  anunciar um teto que o motor não confere.
- `niveis` é **o que o motor faz, e nada além**. Hoje o codificador do PhxZip tem
  um esforço só (`PROFUNDIDADE = 48`, `lzma_compressor.rs`), então são dois:
  `armazenar` (Copy) e `lzma2`. «Rápido» e «máximo» entram no dia em que a
  biblioteca expuser a profundidade — oferecer três rótulos para um codificador
  só seria configuração que ninguém lê, e **configuração que não é lida mente**.
  A tela desenha os níveis que esta lista traz, na ordem dela.

## 3. O envelope dos `POST`

Todo `POST` de `/api/*` leva `Content-Type: application/octet-stream` e o mesmo
envelope:

| deslocamento | tamanho | o quê |
|---|---|---|
| 0 | 4 | `PZW1` em ASCII — assinatura e versão do envelope |
| 4 | 4 | `N`, inteiro sem sinal, *little-endian*: o tamanho da cabeça |
| 8 | `N` | a **cabeça**: JSON em UTF-8 (`N` ≤ `limites.cabeca`) |
| 8+`N` | o resto | a **carga**: bytes crus |

**Por que envelope, e não `multipart/form-data`.** O nome de arquivo é **dado**, e
tem de chegar byte a byte. No `multipart` o nome viaja no `filename="…"`, e o
navegador o reescreve. Medido no Chromium 141 (`FormData` contra um eco local,
24/09/2026):

| nome no disco | o que chegou no `filename=` |
|---|---|
| `a"b.txt` | `a%22b.txt` |
| `c⏎d.txt` (quebra de linha) | `c%0Ad.txt` |
| `e%22f.txt` | `e%22f.txt` |
| `ação.txt` | `ação.txt` |

A terceira linha é a que decide: um arquivo chamado `e"f.txt` e outro chamado
`e%22f.txt` chegam **iguais**. São dois nomes, uma forma só no fio, e o servidor
não tem como separá-los. No envelope o nome vai dentro de um JSON, que tem um
escape só e reversível, e o servidor lê com o analisador JSON que já tem, em
vez de escrever um de fronteiras.

**A senha vai na cabeça, e só nela.** Nunca em URL, nunca em *query*, nunca em
cabeçalho HTTP — um lugar só, e o que não é corpo acaba em log de alguém. O
servidor **nunca a devolve** (nem em erro), **nunca a registra**, e não guarda a
chave derivada depois de responder.

### 3.1 `POST /api/compactar`

Cabeça:

```json
{
  "formato": "7z",
  "nivel": "lzma2",
  "senha": "…",
  "cifrar_nomes": true,
  "nome": "relatorios.7z",
  "itens": [
    {"nome": "relatorios", "pasta": true, "modificado": null},
    {"nome": "relatorios/jan.json", "pasta": false, "tamanho": 5120, "modificado": 1790000000}
  ]
}
```

- A carga é a **concatenação** dos conteúdos dos itens que não são pasta, na
  ordem de `itens`. A soma dos `tamanho` tem de ser igual ao comprimento da
  carga, senão `PEDIDO_MALFORMADO`.
- `modificado`: segundos Unix (UTC), ou `null`. O servidor converte com
  `phxzip::unix_para_filetime`.
- Cada `nome` passa por `phxzip::conferir_nome` **antes** de qualquer byte ser
  comprimido; nome repetido é `NOME_REPETIDO` (o `Escritor` já recusa).
- `senha` ausente ou vazia = sem cifra. Com senha, `cifrar_nomes` escolhe o
  `Opcoes::cifrar_cabecalho` (padrão `true`).
- **`formato: "phz"` passa pelo `phxzip::empacotar`**, e não por uma segunda
  regra na web: um `.phz` é UMA entrada, sempre LZMA2, sempre com cabeçalho
  cifrado (pedido 450). `nivel` e `cifrar_nomes` são **ignorados** nele, e a tela
  diz isso. Sem senha → `SENHA_AUSENTE`; zero itens → `SEM_ENTRADA`; mais de um
  → `MAIS_DE_UMA_ENTRADA`; o item é pasta → `ENTRADA_E_PASTA`.

Resposta `200`: o pacote, com

```
Content-Type: application/x-7z-compressed
Content-Disposition: attachment; filename="relatorios.7z"; filename*=UTF-8''relatorios.7z
X-PhxZip-Tamanho-Original: 5120
```

### 3.2 `POST /api/listar`

Cabeça: `{"senha": "…"}` (ou `{}`). Carga: o pacote.

```json
{
  "ok": true,
  "tamanho_do_pacote": 1893,
  "cabecalho_cifrado": true,
  "entradas": [
    {"indice": 0, "nome": "relatorios", "pasta": true, "tamanho": 0,
     "modificado": null, "cifrada": false, "crc": null, "bloco": null},
    {"indice": 1, "nome": "relatorios/jan.json", "pasta": false, "tamanho": 5120,
     "modificado": 1790000000, "cifrada": true, "crc": "9ae0daaf", "bloco": 0}
  ],
  "blocos": [
    {"indice": 0, "compactado": 1712, "tamanho": 5120, "entradas": 1,
     "metodos": ["LZMA2", "7zAES"]}
  ]
}
```

- `entradas` na **ordem do arquivo** — a tela não reordena o que o pacote guarda.
- `modificado` sai de `filetime_para_unix`; `crc` em hexadecimal minúsculo.
- **`compactado` é do BLOCO, não da entrada.** Num 7z sólido várias entradas
  dividem um bloco comprimido junto, e o tamanho comprimido de cada uma **não
  existe** — dividir o do bloco entre elas seria inventar número. A tela mostra o
  compactado na linha quando o bloco tem uma entrada só, e diz «sólido» quando
  tem mais.
- **Pendente na biblioteca:** `bloco` e `blocos` pedem um acessor público que a
  API ainda não tem (`Entrada::lugar` é `pub(crate)` e `Arquivo::blocos` é
  privado). Enquanto não houver, o servidor manda `"bloco": null` e
  `"blocos": []`, e a tela cai no que é medível sem eles: o tamanho do pacote
  contra a soma das entradas.
- Cabeçalho cifrado sem senha → `SENHA_AUSENTE`. Só o conteúdo cifrado (nomes à
  vista) → lista sem senha, com `"cifrada": true`; a senha passa a ser pedida
  ao extrair.

### 3.3 `POST /api/testar`

Cabeça: `{"senha": "…"}`. Carga: o pacote. O servidor decodifica **todos** os
blocos e confere todos os CRCs (`Arquivo::percorrer`), sem devolver conteúdo:

```json
{"ok": true, "entradas": 12, "pastas": 2, "bytes": 1048576, "blocos": 1}
```

A falha é um erro nomeado (§6), igual às outras rotas.

### 3.4 `POST /api/extrair`

Cabeça:

```json
{"senha": "…", "indices": [3]}
{"senha": "…", "indices": [3], "ate": 262144, "avaliar_json": true}
{"senha": "…", "todas": true}
```

- **Um índice** → a entrada crua, `Content-Type: application/octet-stream`,
  `Content-Disposition: attachment` com o nome da entrada (último segmento), e
  `X-PhxZip-Tamanho: <tamanho inteiro>`.
- **`ate`** (≤ `limites.espiar`) → só os primeiros `ate` bytes, e
  `X-PhxZip-Cortado: 1` quando a entrada é maior. **Espiar é esta mesma rota com
  `ate`, e não uma rota à parte** — a função não se duplica.
- **`avaliar_json: true`** → o servidor julga o conteúdo INTEIRO (não o trecho)
  com o **analisador JSON da casa** (`phxsql_core::json::Json::analisar`), que é
  quem lê o `config.json` de verdade — a tela não tem analisador próprio, porque
  um segundo juiz de «JSON válido» diria sim onde o motor diz não. Responde em
  `X-PhxZip-Json: valido | invalido | nao_e_texto | nao_avaliado` (este último
  acima de `limites.avaliar_json`), e `X-PhxZip-Json-Posicao: <byte>` quando
  `invalido`. **Pendente no `phxsql-core`:** hoje a posição só existe dentro da
  frase do erro («JSON invalido na posicao N: …»); a rota precisa dela como
  número, e recortar a frase é decidir por texto. Até lá, o servidor manda o
  veredito sem a posição, e a tela diz «inválido» sem linha.
- **Vários índices, ou `todas`** → um `.tar` (POSIX ustar; nome que não cabe no
  ustar vai em cabeçalho pax, como GNU tar e bsdtar leem), com as pastas e as
  datas. Tar e não ZIP: o ZIP é recusa nomeada do PhxZip (pedido 454), e o tar
  não comprime — não há segunda compressão para decidir.

## 4. Limites

Todo limite é **declarado** em `/api/estado` e **conferido** no servidor. A tela
confere antes de enviar só para poupar a pessoa de mandar 2 GB para ouvir «não»;
quem protege é o servidor.

| limite | onde se confere | o que acontece |
|---|---|---|
| `envio` | no `Content-Length`, **antes de ler o corpo** | `413 GRANDE_DEMAIS` `{"oque":"envio"}` |
| `cabeca` | no `N` do envelope, antes de alocar | `413 GRANDE_DEMAIS` `{"oque":"cabeca"}` |
| `entrada`, `bloco`, `cabecalho` | no motor (`Limites`), antes de alocar | `413 GRANDE_DEMAIS` com o `oque` do motor |
| `espiar` | no `ate` | `ate` maior é rebaixado ao teto, sem erro |
| `simultaneas` | na entrada da rota `POST` | `503 OCUPADO`; a memória de pico é ≈ `envio` × `simultaneas` × 3 (pacote, bloco decodificado, resposta) |

**Achado do exercício, e regra para o servidor:** responder `413` sem ler o corpo
e fechar a conexão faz o navegador ver **erro de rede**, não o `413` — ele ainda
está enviando quando o soquete fecha (medido no Chromium, `testes-web/phxzip/`).
Por isso a tela confere o teto antes, e o servidor, depois de responder o `413`,
**drena** o corpo até um teto de descarte (sugestão: 2 × `envio`) antes de fechar;
acima do descarte, fecha — e aí a tela mostra «a conexão caiu», não uma frase que
finge saber o motivo.

## 5. Os textos da tela e a fábrica de idiomas

A tela **não tem texto cravado**: todo rótulo sai por chave (`data-txt`,
`data-txt-ph`, `data-txt-tt`, `data-txt-al` e a função `t("chave")` no JS) — a
mesma convenção da tela do PhxSql. O **dado** (nome de arquivo, tamanho, data,
nome de método) entra por `textContent` e nunca passa pela fábrica.

`GET /api/idiomas?idioma=Alemao` responde **no mesmo formato** do `/idiomas` do
PhxSql (`idiomas.rs::textos_para_a_pagina`):

```json
{"ok": true, "idioma": "Alemao", "idiomas": ["Portugues", "…"], "textos": {"zip.aba_compactar": "Komprimieren", "…": "…"}}
```

Idioma desconhecido cai no `Portugues`, e célula vazia cai na coluna `Portugues`
— os degraus 2 e 3 do `idiomas.rs`. A página recebe o texto **já resolvido**, e
nunca as seis colunas.

**Como servir sem duplicar a fábrica do PhxSql** (pétrea «função e comando vêm do
mesmo motor»):

1. **A máquina é uma só.** `IDIOMAS`, o `TextoDeFabrica`, a macro `texto!` e a
   resolução em degraus moram hoje em `phxsql-server` (`mensagens.rs` e
   `idiomas.rs`). O PhxZip é produto à parte (pedido 455: pacote só dele) e **não
   pode** depender do `phxsql-server` inteiro. Então o que é máquina — a lista
   `IDIOMAS`, o tipo e a resolução — **sai para o `phxsql-core`** (de que o
   `phxzip` já depende), e os dois servidores a chamam. É extração, não cópia.
2. **Os textos são dois conjuntos, de perguntas diferentes.** Os da tela do
   PhxZip (`zip.*`) não são os do Centro de Controle (`tela.*`): nenhum rótulo
   daqui é pergunta que a outra tela faz. Cada produto tem a sua tabela; a
   máquina que a resolve é a mesma.
3. **O conjunto do PhxZip é `crates/phxzip-web/ui/textos.json`**, chaveado pelo
   NOME da coluna (`"Portugues"`, `"Frances"`, …) e nunca pela posição. O servidor
   o embute por `include_str!`, confere ao subir que cada coluna existe em
   `IDIOMAS` (e recusa subir se não), e o serve resolvido. Quando a máquina estiver
   no `phxsql-core`, o arquivo vira a entrada dela — ou vira `texto!` em Rust, se
   a frente do servidor preferir; o que não pode é existir nos dois.
4. **Não há degrau 1 (a tabela `phxsys.mensagens`)**: o PhxZip não tem banco.
   Editar texto do PhxZip é editar o `textos.json`.
5. **O laço entre a tela e o dicionário tem de virar teste Rust** no servidor,
   como o do PhxSql: toda chave que a tela pede existe, e toda chave do
   dicionário alguém pede (chave morta é pior que chave faltando). Hoje ele roda
   no roteiro `testes-web/phxzip/exercitar.mjs`. E o conferidor
   `textos-fora-da-fabrica` precisa passar a medir `crates/phxzip-web/ui/` — com
   `Zip`/`PhxZip` na lista de isentos, como `Sql`/`PhxSql`.

## 6. Erros

Toda resposta de erro é JSON, e a tela decide **pelo nome, nunca pela frase**:

```json
{"ok": false, "erro": "METODO_LEGADO", "detalhe": {"metodo": "BZip2"}}
```

**Os nomes do motor são os de `phxzip::Erro::nome()`**, sem tradução nem
renomeação no servidor — a web e o terminal decidem pelo mesmo nome. O
`detalhe` leva os campos da variante:

| `erro` | HTTP | `detalhe` | o que a tela pede |
|---|---|---|---|
| `NAO_E_7Z` | 422 | — | outro arquivo |
| `VERSAO_NAO_SUPORTADA` | 422 | `versao` | regravar com o 7-Zip atual |
| `ESTRUTURA` | 422 | `onde` | o backup |
| `CORROMPIDO` | 422 | `onde` | o backup |
| `SENHA_AUSENTE` | 422 | — | a senha |
| `SENHA_ERRADA` | 422 | — | a senha certa |
| `SENHA_ERRADA_OU_CORROMPIDO` | 422 | — | conferir a senha; se estiver certa, o backup |
| `METODO_LEGADO` | 422 | `metodo` | regravar com LZMA2 |
| `METODO_DESCONHECIDO` | 422 | `metodo` | regravar com LZMA2 |
| `NOME_PERIGOSO` | 422 | `nome` | **não extrair**; nada foi gravado |
| `GRANDE_DEMAIS` | 413 | `oque`, `declarado`, `teto` | dividir o pacote |
| `NAO_CABE` | 422 | `oque`, `valor` | abrir numa plataforma de 64 bits |
| `CICLOS_DEMAIS` | 422 | `pedidos`, `teto` | regravar com menos rodadas |
| `ENTRADA_INEXISTENTE` | 422 | `indice` | listar de novo |
| `SEM_ENTRADA`, `MAIS_DE_UMA_ENTRADA`, `SEM_CIFRA`, `ENTRADA_E_PASTA` | 422 | `quantas` / `nome` | a regra do `.phz` |
| `NOME_REPETIDO` | 422 | `nome` | tirar o repetido |

E os da web, que o motor não conhece:

| `erro` | HTTP | quando |
|---|---|---|
| `PEDIDO_MALFORMADO` | 400 | envelope sem `PZW1`, cabeça que não é JSON, campo faltando, soma dos tamanhos ≠ carga. `detalhe.campo` diz qual |
| `TIPO_DE_CONTEUDO` | 415 | `POST` sem `application/octet-stream` |
| `TAMANHO_AUSENTE` | 411 | `POST` sem `Content-Length` |
| `HOST_RECUSADO` | 403 | `Host` que não é `127.0.0.1:<porta>` nem `localhost:<porta>` (religação de DNS) |
| `ORIGEM_RECUSADA` | 403 | `Origin` presente e diferente de `http://127.0.0.1:<porta>` / `http://localhost:<porta>`, ou `Sec-Fetch-Site` presente e diferente de `same-origin` |
| `ROTA_INEXISTENTE` | 404 | |
| `METODO_HTTP` | 405 | |
| `OCUPADO` | 503 | `limites.simultaneas` atingido |
| `INTERNO` | 500 | pânico ou falha de E/S — **sem** o texto do pânico |

**`onde` é texto do motor, em português, e não se traduz** — o mesmo naipe das
três mensagens que o PhxSql deixa sem tradução de propósito. A tela o mostra
como «detalhe técnico», fechado, e nunca como a frase principal.

## 7. Segurança, em uma tabela

| regra | por quê |
|---|---|
| só `127.0.0.1` | a porta extrai arquivos e aceita senha |
| `Host` conferido | um site de fora não religa o próprio nome para `127.0.0.1` e conversa com a porta |
| `Origin`/`Sec-Fetch-Site` conferidos, `Content-Type` exigido | `application/octet-stream` não é tipo «simples»: o `POST` de outra origem cai no *preflight*, e o servidor não responde CORS |
| senha só na cabeça do envelope | URL e cabeçalho acabam em log |
| nada de caminho de disco | não há o que um pedido malicioso escolha no disco |
| `conferir_nome` no motor, na ida e na volta | zip-slip se recusa onde o nome nasce, não na tela |
| estáticos por lista fechada | não existe `../` para pedir |
| sem estado entre pedidos | nada fica em memória esperando ser achado |

Cabeçalhos em **toda** resposta:

```
Content-Security-Policy: default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'
X-Content-Type-Options: nosniff
Referrer-Policy: no-referrer
Cache-Control: no-store
```

O CSP não é enfeite: sem `'unsafe-inline'`, um nome de entrada como
`<img src=x onerror=…>` que escapasse do `textContent` ainda não rodaria. A tela
foi exercitada **sob este CSP** e não viola nenhuma diretiva.

**O custo do «sem estado», dito:** listar, espiar e baixar uma entrada reenviam o
pacote inteiro, e extrair uma entrada de um bloco sólido decodifica o bloco
inteiro (é o que o `Arquivo::extrair` faz). Em `127.0.0.1` o reenvio de 256 MiB
é memória para memória; o que pesa é a decodificação, e ela pesaria igual com
sessão. A sessão compraria pouco e deixaria pacote e chave em memória entre
pedidos.

## 8. O que a tela garante do lado dela

- A senha fica numa variável da página enquanto o pacote está aberto, e sai de
  lá ao fechar. Nunca em `localStorage`, nunca em URL, nunca em `<form>` — um
  `Enter` num formulário sem `action` manda `GET ?senha=…` para a própria página,
  e o roteiro confere que isso não acontece.
- «Limpar a lista» limpa também a senha. Achado exercitando: o bloco de opções
  some com a lista vazia, a senha ficava no campo escondido, e a lista seguinte
  sairia cifrada com uma senha que ninguém via mais.
- Todo dado entra por `textContent`/`createElement`; `innerHTML` só recebe
  ícone constante.
- Caractere de controle e de direção (U+202E e irmãos) num nome de entrada
  aparece **visível**, como marca, em vez de reordenar o nome na tela:
  `fatura‮txt.exe` desenhado cru mostra «fatura**exe.txt**». Mostrar o nome
  reordenado seria mentir sobre o dado; mostrar a marca é dizer o que está
  gravado.

## 9. O servidor falso

`testes-web/phxzip/servidor_falso.py` — Python da biblioteca padrão, responde este
contrato com dados de exemplo, **inclusive os erros**, e decide o cenário pelo
**conteúdo** do pacote (como o de verdade), nunca pelo nome do arquivo. Não
comprime em 7z: o «pacote» que ele devolve começa com a assinatura do 7z e guarda
os itens num formato só dele, para que compactar → abrir → espiar → baixar feche
a volta inteira no navegador. Como rodar: `testes-web/phxzip/LEIA-ME.md`.
