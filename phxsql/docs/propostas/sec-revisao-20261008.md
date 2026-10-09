# Revisão SEC — o que entrou em 08/10/2026 (`51d7e5cc..ed582124`)

Papel SEC, só leitura. Escopo: 7 commits (`a0325c80` … `ed582124`). A árvore suja da frente do TLS
(`tls*`, `dblink/mysql.rs`, `email.rs`) **não** foi revisada como entregue; só o HEAD.
Estado: **3 ALTO (bloqueiam), 4 MÉDIO, 5 BAIXO.** Nada consertado aqui.

## ALTO — bloqueiam

### A1. O retrato recebido escreve e apaga FORA do database (nome do fio sem `validar_nome` no schema)

- **Onde:** `phxsql-server/src/servidor/servico_diario_01.rs::refazer_por_retrato` (:535 lê `tabela`
  do fio, :569 passa adiante) → `phxsql-store/src/catalogo.rs::trocar_pelo_retrato` (:1828). Só a
  parte da TABELA passa por `validar_nome`; o SCHEMA vai direto em `self.caminho().join(sc)` (:1851).
  `Path::join` com caminho absoluto **substitui** a base.
- **Entrada → efeito:** a origem (ou quem está no meio de um link de replicação sem `cifra`/`pino_tls`)
  anuncia no `posicao` uma `base` acima da posição da réplica (força `precisa_de_retrato`) e responde
  ao `retrato_da_replica` com `{"tabela":"/srv/phxsql/base/central.produtos","arquivo":"produtos.reg",…}`.
  `separar_qualificado` dá schema `/srv/phxsql/base/central`, tabela `produtos`; conferido com `rustc`:
  `dir=/srv/phxsql/base/central`. A réplica **apaga** todo `produtos.*` (EXTENSOES_TODAS) daquele
  diretório e põe no lugar os bytes da origem.
- **No espelho do 325:** um caixa comprometido reescreve o cadastro do CENTRAL (preço, produto) ou o
  database de outro caixa — o «um escritor por database» do 677 cai inteiro. Fora da base: cria
  diretório e escreve/apaga `<nome>.{reg,ndx,bin,memo,log}` onde o processo tiver permissão.
- **Teste adverso:** origem falsa (`TcpListener`) que responde `posicao` com `base` alta e o retrato com
  `tabela` absoluta apontando para outro database da réplica → tem de recusar e nenhum arquivo fora de
  `base/<database>/` pode mudar (conferir `mtime` e SHA-256 do database vizinho).
  Variante: `tabela` que não está entre as tabelas replicadas daquela origem.

### A2. O pedaço do retrato não é amarrado ao database nem à sessão: leitura cruzada e cifra contornada

- **Onde:** `servico_diario_01.rs::pedaco_do_retrato` (:425) e `RetratoServido` (:298), que guarda só
  `id` e arquivos — sem database, sem usuário, sem o veredito do fio. O portão de permissão olha o
  `database` do PEDIDO, e o pedaço não confere que é o database do retrato. O portão do 342 (dado
  pessoal só em fio cifrado) roda só no `tirar_retrato`.
- **Entrada → efeito:** a réplica legítima tira o retrato de `rh` (fio cifrado). O usuário U, com
  `replicar` só em `loja`, por fio em claro, manda
  `{"op":"retrato_da_replica","database":"loja","id":<ms>,"indice":0,"offset":0}`. O `id` é
  `agora_ms()` (:378): adivinha-se varrendo a janela, e o erro «não há retrato {id}» distingue o acerto
  do erro sem contar violação. U recebe o `.reg`/`.memo` cru de `rh`, com coluna marcada, em claro.
  O mesmo U manda `"soltar":true` e apaga o retrato do outro (a réplica legítima nunca termina).
- **Teste adverso:** sessão A (permissão total, fio cifrado) tira o retrato de `rh` com coluna
  marcada; sessão B (`replicar` só em `loja`, fio em claro) pede o pedaço com o `id` de A → recusa.
  B manda `soltar` → o retrato de A continua. Os dois têm de falhar com o defeito de hoje.

### A3. HEAD: dblink MySQL aceita `tls`/`tls_ca`/`pino_tls` e conecta em CLARO

- **Onde:** `dblink/mod.rs::Motor::fala_tls` diz `Postgres | MySql`; `tls_de_saida()` valida até o
  PEM na declaração; mas no HEAD, `conectar_com` (:1148) chama `mysql::Conexao::abrir` **sem** o TLS.
- **Entrada → efeito:** quem cadastra `{"motor":"mysql","tls":"verificar","tls_ca":"…"}` recebe
  aceite, e a consulta e o resultado vão em claro. A tela mostra `tls: verificar`. É configuração que
  não é lida. (A senha não vaza: o nativo é desafio e o `caching_sha2` completo já recusa sem TLS.)
- **Estado:** a árvore da frente já passa o TLS e recusa servidor sem `CLIENT_SSL`. **Não integrar o
  HEAD sozinho.** O teste adverso que fecha: MySQL falso sem `CLIENT_SSL` + ligação `tls:"exigir"` →
  recusa antes da credencial; com `CLIENT_SSL` e `S` grudado em dado claro → recusa (o buffer do
  CVE-2021-23222).

## MÉDIO

### M1. O expurgo acredita no `consumidor` que o pedido diz ser

- **Onde:** `servico_diario_01.rs::anotar_confirmacao_do_diario` (:63). O nome sai de
  `consumidor`/`para` do pedido, e `desde`/`duravel` também. Nada amarra o nome ao login nem ao IP.
- **Entrada → efeito:** qualquer sessão com `replicar` no database manda
  `{"op":"replicar",…,"consumidor":"central","desde":<ponta>}`. A confirmação é `max`, e não volta
  até o reinício. O expurgo tira volumes que o central não puxou. Na réplica fiel isso força um
  retrato; no **bidirecional, que não se refaz por retrato**, o par para em `erro.diario_expurgado`
  com venda que não chegou.
- **Teste adverso:** com `diario.consumidores=["central"]`, uma sessão de outro login confirma como
  `central` → a passada não pode tirar volume que o central de verdade não confirmou.

### M2. O retrato é UM slot global, e quem tem `replicar` congela o servidor

- **Onde:** `tirar_retrato` (:347) chama `soltar_retrato_servido` e copia o database inteiro com a
  trava global de dados na mão.
- **Entrada → efeito:** pedidos repetidos de `retrato_da_replica` (sem `id`) param toda escrita
  enquanto a cópia roda, e põem uma cópia inteira do database na raiz. Num caixa, isso trava a venda
  ou enche o disco. Sem atacante: duas réplicas atrasadas ao mesmo tempo apagam o retrato uma da
  outra e nenhuma termina.
- **Teste adverso:** duas réplicas pedem o retrato do mesmo database ao mesmo tempo → as duas têm de
  terminar. Um laço de 50 pedidos de retrato tem de ter teto ou recusa.

### M3. Restrição de nome excluída não segura o SAN curinga

- **Onde:** `phxsql-core/src/cadeia.rs::dns_cabe` (:541) e `Restricoes::aceita`. A exclusão
  `secreto.exemplo.com` não alcança o SAN `*.exemplo.com`, e `dns_casa` (:939) aceita esse curinga
  para `secreto.exemplo.com`. É a classe do CVE-2025-61727 do Go.
- **Entrada → efeito:** uma autoridade intermediária com a exclusão emite `*.exemplo.com`; o dblink
  em `tls:"verificar"` para `secreto.exemplo.com` aceita.
- **Teste adverso:** raiz → intermediária com `excludedSubtrees dNSName secreto.exemplo.com` → folha
  com SAN `*.exemplo.com`; `validar_servidor_tls(…, "secreto.exemplo.com")` → recusa.
  `outro.exemplo.com` → aceita (o irmão, para o portão não recusar tudo).

### M4. A cópia do retrato sobrevive ao retrato (resíduo de dado pessoal)

- **Onde:** os arquivos `.retrato-servido-*` (origem) e `.retrato-recebido-*` (réplica) em
  `config.base`. Só saem no `soltar` ou no próximo retrato. Não há limpeza no arranque nem prazo.
- **Efeito:** quando a réplica cai no meio, sobra uma cópia inteira de tabelas com coluna marcada
  fora do ciclo da tabela. Um esquecimento ou uma exclusão feita depois não alcança essa cópia.
  **Não medi** se o backup varre a raiz; se varrer, ela também vai para o backup.
- **Teste adverso:** retrato tirado, réplica morta sem `soltar`, servidor reiniciado → nenhum
  `.retrato-*` na raiz.

## BAIXO

- **B1.** `servico_web_01.rs:1525`: o caminho remoto emite um id de sessão para uma op que não é
  `login`/`desafio` **sem** `conferir_a_emissao_da_sessao`. O id nasce anônimo e o login seguinte gira
  (o 719 vale), então a fixação está coberta. Mesmo assim é um segundo caminho de emissão fora do
  portão único.
  Teste: `{"op":"ping","servidor":"<remoto>"}` por HTTP claro e fora do loopback → nenhum `sessao` na
  resposta.
- **B2.** CSP: `integracao_claude` nasce `true`, e com `script-src 'unsafe-inline'` um XSS exfiltra
  dados para `api.anthropic.com` com a chave **do atacante** (a folga do `connect-src` é o canal).
  Isso já existia antes de hoje; o 339(a) deu o interruptor. E está latente:
  `http.rs::montar_resposta` (:396) dá a folga a todo `text/html` **sem ler a config**. Hoje ninguém
  o chama com HTML; no dia em que chamar, o interruptor mente.
- **B3.** `pg/scram.rs::ponta_do_servidor` devolve `None` para certificado RSA-PSS ou Ed25519. O
  cliente manda então `y,,` a um PG que oferece o `-PLUS`, e o PG recusa. É disponibilidade, não
  vazamento. O modo `exigir` não protege de quem está no meio, como diz a documentação.
- **B4.** `tls_ca` no cadastro do dblink: a mensagem de erro diz se o arquivo existe (só quem tem
  `administrar` chega lá).
- **B5.** `PHXSQL_TESTE_*`: todos estão sob `#[cfg(debug_assertions)]`. `gancho_de_teste` e
  `gancho_de_teste_em_texto` nem existem em release, então o compilador garante, e o `empacotar.sh`
  só gera `--release`. **Nenhum é lido em release.** O `sigkill_de_teste` chama `kill` pelo `PATH`,
  mas isso só existe em debug.

## Conferido e que se sustenta

- 719: os dois caminhos de login (local e remoto) giram o id e matam o velho.
- 674: o portão cobre `desafio`/`login` local (`servico_rede_01.rs:1341/1415`) e o remoto.
- 339(a): nenhum `setItem` de segredo; a migração apaga as duas gavetas velhas; com `false`, o
  `connect-src 'self'` vem do servidor.
- PG: o `N` ao `SSLRequest` recusa (não cai para o claro). `R 0` sem SCRAM com senha recusa.
- Cadeia: o CN não conta; extensão repetida recusa; extensão crítica desconhecida recusa;
  `serverAuth` vale no caminho; `pathLen` e `keyCertSign` conferidos; o curinga segue a RFC 9525.
- O retrato está em `OPS_DE_REPLICACAO`, `Atividade::Replicar` e `PorColuna::Recusa`.
  `tem_dado_pessoal` conta a coluna externa marcada.

## Para a frente do TLS (árvore ainda suja)

Antes de integrar, conferir que o SMTP passa ao `passar_a_tls` o **host sem a porta** (se
`cfg.servidor` traz `:587`, a verificação nunca casa) e que o `STARTTLS` recusa linha grudada no
`220`.
