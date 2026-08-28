# Changelog

Tudo que mudou no PhxSql, do mais novo para o mais antigo.

Formato: cada versão traz **Corrigido** primeiro — defeito é o que o leitor
precisa achar rápido —, depois **Adicionado**, **Mudado** e **Sabido**. A
seção *Sabido* lista o que ainda não funciona, para ninguém descobrir sozinho.

Os números são **medidos**, nunca estimados.

---

## 0.4.1 — 2026-08-28

Rodada de revisão: nada de recurso novo, só o que a leitura do próprio projeto
achou de errado.

### Corrigido

- **A bancada media coisas diferentes dos dois lados.** Na varredura por faixa
  o MySQL(R) recebia `COUNT(*) + SUM(valor)` sobre 1.250.000 linhas enquanto o
  PhxSql lia 20.000 — mesma pergunta, 1,6% do trabalho. O «5× mais rápido» que
  saía dali não era o motor: era o serviço menor. A fase `varrer` de
  `examples/carga.rs` passou a ler a faixa inteira e somar o valor, e a
  medição de dez milhões foi **refeita do zero**.

  É o segundo erro deste tipo — o primeiro favorecia o MySQL(R), este
  favorecia o PhxSql. Por isso a bancada ganhou uma quarta regra: *mesma
  quantidade de trabalho*, não só mesma forma de pergunta.

  A prova de que agora está igual não é a promessa, é a soma: os dois motores
  devolvem 1.250.000 linhas e **5.576.201.000,00**, o mesmo total até o
  centavo, por dois códigos sem uma linha em comum.

  E o resultado sobreviveu ao conserto — a varredura continua a favor do
  PhxSql, por **3,3×** em vez dos 5× que a montagem errada prometia. A nova
  medição: inserção 20,7× mais devagar (4.039 linhas/s contra 83.492), busca
  pontual 2,6× mais devagar, exclusão 2,0×, atualização empatada, varredura
  3,3× mais rápida. Escreve 2,29 GiB onde o MySQL(R) escreve 32,03; ocupa
  2,27 GiB onde ele ocupa 0,88.

- **Campo com nome errado no `config.json` era silencioso.** Quem quisesse
  trocar a porta escreveria `"porta": 5001`, e o campo se chama `bind`: o
  servidor subia na 5000 sem uma palavra. O arranque agora lista os campos que
  não reconheceu e diz que o valor foi ignorado. Não vira erro — config antigo
  continua subindo.

- **Seis marcas de terceiros sem o `(R)`**: `MySQL` em `docs/REPLICACAO.md` e
  no dossiê, `HFSQL` em dois módulos, `SQLite` e `Clarion` no `docs/PLANO.md`.

- **O painel tem sete gráficos, não nove.** O README e o dossiê diziam nove.
  Contados: um de área, um de anel e cinco de barras.

- **A versão que o servidor anunciava estava errada.** O `Cargo.toml` do
  workspace ainda dizia `0.1.0` enquanto este changelog ia em 0.4.0 e os
  pacotes saíam com 0.4.0 no nome. Como `VERSAO` é `env!("CARGO_PKG_VERSION")`,
  o `ping`, o `quem_sou` e o rodapé do Centro de Controle respondiam `0.1.0` a
  quem perguntasse. Cliente que decide compatibilidade pela versão estava
  recebendo a resposta errada há três lançamentos.

- **Números velhos no dossiê.** A capa dizia 276 testes (são 280) e 3.184
  linhas de doc (são 3.261); o rodapé ainda dizia *PhxSql 0.3.0 · 19.242
  linhas · 69 KB de interface*, três números defasados de uma vez. Remedidos:
  20.224 linhas de Rust, 158 KiB de interface, 280 testes. A regra do projeto é
  medir, e ela vale para o documento que apresenta o projeto.

- **A bancada não estava no dossiê.** A comparação com o MySQL(R) em dez
  milhões de registros — a maior medição já feita aqui — existia só em
  `bancada/` e no roteiro, como uma linha marcada «pronto». Virou a seção 16,
  com a figura, a tabela dos oito números e o diagnóstico da inserção.

- **Três pedidos não estavam nem registrados.** Triggers, stored procedures e
  jobs foram pedidos e não constavam do roteiro do dossiê — nem como «a fazer».
  Ausência que não está escrita é ausência que se esquece.

### Adicionado

- **`docs/PENDENCIAS.md`.** A revisão do que falta, em um lugar só: o que foi
  pedido e não existe, o que depende de decisão do Adriano, o que está travado
  de fora, o checklist das perguntas já respondidas, e o único buraco que a
  medição apontou sem ninguém pedir.

- **`empacotar.sh`.** Monta os pacotes de Linux e Windows e o zip de fontes.
  Os das rodadas anteriores foram feitos à mão — pacote que ninguém consegue
  refazer é pacote em que não se deve confiar. O zip de fontes sai de
  `git archive`, que respeita o `.gitignore` de graça.

- **`docs/dossie/numeros-da-bancada.py`.** A figura, a tabela e o diagnóstico
  da seção 16 passam a ser **gerados** de `bancada/resultados.json`. Número
  digitado envelhece calado; número gerado não tem como divergir da medição.

- **`.gitignore`** para os 2,4 GB que a bancada cria em `bancada/phxsql/`.

---

## 0.4.0 — 2026-08-27

### Adicionado

- **Painel.** A primeira tela depois do login: o servidor inteiro em gráficos
  — bancos, registros, usuários, conexões, acessos, recusados, IPs bloqueados
  e tabelas em RAM nos números do topo; operações por hora nas últimas 24 h;
  operações mais pedidas; usuários por nível; maiores tabelas; de onde vêm os
  acessos; e quem mais usou.

  Tudo de **uma** chamada — a operação `painel` agrega no servidor. Dez
  chamadas deixariam a tela dez vezes mais lenta só pela ida e volta. E o
  painel conta **só o que quem está olhando poderia abrir**: base sem
  permissão de leitura não entra na conta.

  Os gráficos são SVG escrito à mão — barras, área e anel —, como o resto do
  projeto. Usam `currentColor` e os tokens do tema, então trocam de cor com o
  sol/lua sem uma linha a mais.

- **O phx-grid v0.8.0 na aba Conteúdo.** O grid do ecossistema Phoenix, ES5
  estrito e sem dependência. Arrastar um cabeçalho para a faixa de cima
  **agrupa**, com contagem e agregados por grupo; vários níveis empilham e as
  pastilhas reordenam arrastando. Vieram junto a busca global e a paginação.

  As colunas saem do **esquema** da tabela, não de uma lista escrita à mão —
  tabela nova aparece certa sem tocar na página. E o grid segue o tema do
  console.

- **Comparação medida com o MySQL(R)**, 10.000.000 de registros, em
  `bancada/`. Tudo para ser refeito: `python3 bancada/medir.py 10000000`.

- **Espelho `.bkp`** (`"espelho": true`): toda escrita no `.reg` vai também
  para um irmão, e a leitura tenta o espelho quando o CRC falha. `phxsql
  reparar` conserta nos dois sentidos e **conta** o que não teve salvação.

- **Três portas de replicação:** `envio` e `retorno` separadas, validadas
  contra a porta de dados, a da web e uma contra a outra.

- `phxsqld --pagina` escreve o Centro de Controle num arquivo — da **mesma**
  função que serve o navegador.

### Corrigido

- **Ligar o espelho apagava a cópia boa.** `espelhar()` copiava o `.reg` por
  cima do `.bkp` existente: estragar o principal e religar o espelho destruía
  a segunda chance. Um teste pegou. Agora só semeia o que ainda não existe.

- **Erro de medição na bancada.** A primeira versão mandava ao MySQL(R) um
  único `WHERE id IN (…)` e ao PhxSql vinte mil buscas separadas — 41× a
  favor do MySQL(R) pela *forma da pergunta*, não pelo motor. Corrigido para
  uma instrução por operação dos dois lados; o SELECT pontual passou de 41×
  perdendo para 3,4×, e o UPDATE de perdendo para empate.

- **Gráficos desproporcionais.** O `viewBox` de 620 dentro de cartões de
  ~370 px encolhia o desenho inteiro em 0,6 — texto de 12 px virava 7 px.
  Cada gráfico passa a nascer com a largura do cartão que o recebe.

- **Colisão de nome entre o relay e o backup**: o campo do servidor remoto se
  chamava `destino`, e `destino` já era o diretório do backup. Renomeado.

### Sabido

O que a medição diz e ninguém deve esconder: **a inserção é o nosso buraco**
— 3.685 linhas/s contra 95.301 do MySQL(R), e é CPU, não disco. Continuam
faltando triggers, stored procedures, jobs, transporte de replicação,
start/stop pela interface, transações e TLS.

**280 testes**, clippy limpo, zero dependências externas.

---

## 0.3.0 — 2026-08-27

### Corrigido

- **O nível de usuário quase afrouxou todo `config.json` existente.** O padrão
  do campo novo `nivel` era `leitor`, e isso mudava o comportamento de quem já
  tinha config: base sem regra explícita passava de *nega tudo* para *lê tudo*.
  Um teste antigo (`sem_curinga_e_sem_base_nega_tudo`) quebrou e apontou o
  problema. Existe agora `Nivel::Nenhum`, que é o padrão, e o teste antigo
  passa sem alteração — que é a prova de que nada mudou para quem já tem
  config.

- **`phxsqld --usuarios` mentia sobre quem podia o quê.** Escrevia
  `(nenhuma)` para usuário sem regra de base, mesmo quando o nível dava poder,
  e mostrava `supervisor` numa coluna em vez do nível. Agora mostra o nível e
  o que ele concede.

### Adicionado

- **Nível de usuário:** `nenhum`, `leitor`, `operador`, `dono`, `admin`. Cada
  um contém o anterior, e há teste que percorre as dez atividades para
  garantir. A regra de uma base específica ganha do nível, inclusive para
  **tirar** poder — dá para dar `admin` a alguém e ainda assim fechar uma base.

- **Backup em ZIP**, com o DEFLATE (RFC 1951) escrito neste projeto — Huffman
  fixo mais casamento LZ77. Nome
  `BancoNome_Admin_Data_HoraMin.zip`, com o manifesto dentro.

  A prova não é o teste de ida e volta com o próprio código; é o mundo abrir:
  `unzip -t` passa todos os CRC, e o `zipfile` do Python extrai e confere byte
  a byte contra o original. **18.311 → 2.406 bytes, 87% menor.**

- **Backup agendado**, seção `backup` no `config.json`, desligada por padrão.
  `hora` (uma vez por dia) ou `cada_horas`, com `manter` para a retenção. O
  relógio confere de minuto em minuto em vez de dormir até a hora — dormir
  horas seguidas é frágil. A faxina só apaga arquivo com a cara dos nossos.
  Todo backup agendado entra no `acessos.log`.

### Sabido

Continua tudo da 0.2.0: replicação sem transporte, sem start/stop pela
interface, sem transações, sem TLS, sem compactação, sem SQL, sem MCP, sem
ODBC.

**276 testes**, clippy limpo, zero dependências externas.

---

## 0.2.0 — 2026-08-27

### Corrigido

- **Sondagem de travessia de diretório não contava violação.** Nome de
  database, tabela ou schema com `..` ou barra já era recusado pelo motor,
  mas era recusado **calado**: não contava tentativa e não gerava bloqueio.
  Auditado com seis sondagens seguidas (`../../../etc`, `/etc`, `C:\dados`,
  byte nulo, quebra de linha): seis recusas, seis linhas no `acessos.log`,
  **zero bloqueios**. Quem sondasse podia tentar a noite inteira.

  Agora é violação grave, na mesma classe de comando proibido: **bloqueia na
  primeira tentativa** e cria a regra de firewall. Conferido contra servidor
  de verdade — uma sondagem, um bloqueio, uma regra.

  A separação está em `catalogo::nome_hostil`, deliberadamente distinta de
  `validar_nome`: `"minha tabela!"` é um nome ruim (alguém errou, recusar
  basta); `"../../etc/passwd"` não é nome nenhum.

- **Colisão de nome entre o relay e o backup.** O campo que escolhe o servidor
  remoto se chamava `destino` — e `destino` já era o diretório do backup.
  Resultado: todo pedido de backup ia parar no relay e voltava com "esta
  interface não fala com outro servidor". Renomeado para `servidor`. Achado
  ligando as duas peças, não lendo o código.

- **`fe_de_bytes` do Ed25519 lia sete bytes onde precisa de oito.** O pedaço
  do meio perdia o bit 152. Passou despercebido no teste do ponto base — que
  tem esse bit em zero — e só apareceu quando os vetores da RFC 8032 rodaram.
  É exatamente por isso que a regra "criptografia se confere contra vetor
  oficial" existe.

- **Duas cores presas ao tema escuro.** O gradiente da tela de entrada e a
  tinta do botão eram literais. No tema claro o botão ficava com tinta quase
  preta sobre vermelho escuro. Viraram token.

### Adicionado

- **Tabela em memória e `SelectMemory`.** A tabela inteira em RAM, com
  consulta que não toca em disco. Filtros (`=`, `!=`, `<`, `<=`, `>`, `>=`,
  `contem`, `comeca`, `termina`, `nulo`, `nao_nulo`), ordenação múltipla,
  projeção de colunas, `pular` e teto. Filtro de igualdade numa coluna
  mapeada evita a varredura, e a resposta diz qual mapa usou.

  **Medido** (`cargo run --release --example memoria`, 50.000 linhas, a mesma
  pergunta pelos dois caminhos):

  | caminho | tempo | linhas examinadas |
  |---|---:|---:|
  | varrendo o `.reg` | 55.878 µs | 50.000 |
  | `SelectMemory` | 641 µs | 8.333 |

  **87×.** Carga para a RAM: 53 ms, 2.205 KB de valores. O exemplo confere as
  duas respostas linha por linha antes de imprimir o número.

  Nada entra em memória sozinho, e toda escrita atualiza a cópia residente
  **dentro da mesma trava** do disco — não existe janela em que os dois
  discordem.

- **Chave assimétrica Ed25519 como segundo fator.** Escrito do zero, mais o
  SHA-512 que ele exige. Conferido contra os quatro vetores da RFC 8032
  seção 7.1, o vetor de 1023 bytes, e os quatro do FIPS 180-4 para o SHA-512.

  E a prova que vale mais: um cliente de teste que assina com a implementação
  **de referência** da RFC (Python puro, independente desta) gerou a mesma
  chave pública e teve a assinatura aceita pelo servidor.

  `phxsqld --gerar-chave` imprime o par uma vez. `"chave_publica"` no usuário
  do `config.json` passa a exigir assinatura no login, sobre o **mesmo**
  desafio da senha — então a assinatura também vale uma vez só.

- **Sistema de backup com manifesto conferível.** Cópia mais um `backup.json`
  com o SHA-256 de cada arquivo, e um comando que lê tudo de volta e confere.
  Acha arquivo que sumiu, arquivo que mudou (mesmo do mesmo tamanho) e
  arquivo que apareceu sem estar no manifesto.

  ```
  phxsql backup <base> <destino>        com o servidor parado
  phxsql conferir-backup <destino>      sai com erro se não bater
  {"op":"backup","destino":"..."}       com o servidor no ar, sob a trava
  ```

- **Alternador de tema, sol ☀️ e lua 🌙.** Paleta clara completa, começando no
  que o sistema pede e lembrando a escolha por navegador. O vermelhão
  escurece para `#c63c0a` no claro, por contraste — a mesma adaptação que o
  dossiê já fazia.

- **Campos de conexão no login:** servidor (IP ou DNS), porta, usuário, senha,
  chave privada e database. A porta que aparece é a que o servidor
  **realmente** escuta, lida do `/saude`.

- **Console para mais de um servidor.** Apontar o login para outro endereço
  abre uma conexão para ele, mantida viva pela sessão. `web.servidores`
  começa **vazio** — interface que fala com qualquer endereço é proxy aberto
  de saída.

- **`replicacao.escuta`:** o socket onde o *source* serve os eventos, separado
  da porta de dados. O config recusa colisão com a porta de dados e com a
  da web.

### Mudado

- Nomes de bancos de terceiros na documentação passam a levar **(R)**.
  Exceções deliberadas: nomes de pacote (`rusqlite` é identificador, não
  marca) e citações literais de texto alheio.
- `memoria_carregar`, `memoria` e `SelectMemory` pedem permissão de **ler**,
  não de administrar: é o mesmo dado do disco por outro caminho.
- O arranque avisa alto quando o papel de replicação não é `isolado`, porque
  o transporte de eventos ainda não existe.

### Sabido — o que ainda não funciona

- **Replicação não transporta evento.** A configuração entra e valida; o
  desenho está em `docs/REPLICACAO.md`; o `.log` v2 com imagem da linha é o
  próximo passo. Hoje o papel é só um rótulo.
- **Start/stop do serviço de dados pela interface** não existe. Parar a porta
  5000 sem derrubar o processo exige mexer no laço de aceitação, e prefiro
  fazer isso inteiro a fazer pela metade.
- **Sem transações**, logo sem o A nem o I do ACID.
- **Sem TLS.** O tráfego vai em claro; a credencial não, quando se usa
  desafio-resposta ou chave.
- **Sem compactação**, sem camada SQL, sem MCP, sem ODBC.
- `crypto.subtle` com Ed25519 é recente; navegador sem suporte não assina, e
  a página diz isso em vez de fingir.

**254 testes**, clippy limpo, zero dependências externas.

---

## 0.1.0 — 2026-08-27

Primeira versão que roda ponta a ponta.

### Adicionado

- **Os cinco arquivos:** `.reg` (registros na ordem de digitação, CRC por
  registro, esquema embutido), `.ndx` (B+tree com divisão de páginas, chave
  composta, ASC/DESC/NOCASE/único), `.bin` e `.memo` (blocos com CRC e
  contabilidade de espaço morto), `.log` (diário datado das três operações).
- **Ordem de digitação como garantia:** slot excluído nunca é reaproveitado.
- **Paginação em volumes** `_001`, `_002`, … com abertura preguiçosa. O
  volume sai da aritmética do rowid, então o índice não paga nada por ela.
- **Hierarquia** database → schema → tabela, em diretórios.
- **Chave estrangeira** no esquema, com CASCADE / RESTRICT / SET NULL.
- **Reindex:** recria o `.ndx` do zero a partir do `.reg`.
- **Servidor TCP na porta 5000**, protocolo JSON Lines, `config.json`.
- **Log de acessos** por IP, com data e hora ao milissegundo — inclusive das
  tentativas recusadas.
- **Cadastro de usuários** com senha em PBKDF2-HMAC-SHA256 de 210.000
  iterações, e permissão por base em dez atividades.
- **Login por desafio-resposta** (a senha não trafega) e por Base64.
- **Política, blacklist e gancho de firewall:** comando proibido bloqueia o
  IP na hora; token e senha errados contam tentativa.
- **Centro de Controle:** interface web embutida no binário, servida pelo
  próprio `phxsqld`.
- **Linha de comando** com nove comandos, e compilados para Linux e Windows.

### Sabido

Zero dependências externas — só a `std`. JSON, CRC-32, SHA-256, HMAC, PBKDF2
e Base64 escritos aqui, cada um conferido contra vetor oficial.
