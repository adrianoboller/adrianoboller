# 0.4) Diretivas do banco e do servidor — o HFSQL e o `ALTER … SET` do PhxSql

*Medido em 2026-09-07 17:56 UTC, commit `b463b1d` + esta frente,
`phxsqld 0.18.0`. 61 afirmações, 0 falhas.*

## Resposta curta

O HFSQL espalha a configuração por **oito portas** — `HSetServer`,
`HSetTransaction`, `HSetLog`, `HSetIntegrity`, `HSetDuplicates`, `HSetTrigger`,
as propriedades da `Connection` e `HManageTask`. Você pediu para centralizar em
`SHOW`, `ALTER … SET` e `TRUE`/`FALSE`, e é o que existe agora: **quatro
escopos** (servidor, banco, tabela, conexão), um verbo para ver, um verbo para
mudar, e toda alteração no **diário administrativo** com os nove campos que
você listou.

O mapa saiu medido, diretiva por diretiva, contra o motor vivo: **39 diretivas
mapeadas**, das quais **22 têm equivalente** (11 com ressalva registrada — existem
no `config.json` e não se mudam pela rede — e 4 parciais), **17 não existem e são
dispensa com o motivo técnico**, e **1 entrou nesta rodada**:
`comandos_proibidos` por banco, que é o pedido 220. O `ALTER SERVER SET` não
tem caminho próprio: ele desemboca no mesmo
`config_gravar` que a tela usa, com o mesmo portão, a mesma validação de tipo e
a mesma gravação atômica — *portão de permissão é UM só*. São **49 diretivas de
servidor** graváveis, 17 delas a quente.

## Exemplo exercitado

**Os quatro `SHOW`.** O de tabela é o que mostra as três diretivas do HFSQL
lidas na declaração — `duplicate_check` é o `unico` do índice,
`referential_integrity` é o `verificar` da chave (e ela **nasce conferida**), e
o journal é o `.log` que toda tabela tem:

```json
{"escopo":"tabela","servidor":"127.0.0.1:7300","banco":"erp","tabela":"pedidos",
 "duplicidade":[{"indice":"ix_id","duplicate_check":true,"unico":true},
                {"indice":"ix_cli","duplicate_check":false,"unico":false}],
 "integridade_referencial":[{"chave":"fk_ped_cli","tabela_ref":"clientes",
                             "referential_integrity":true,
                             "ao_excluir":"Restringir","ao_alterar":"Cascata"}],
 "triggers":0,"journal":true,"motivo_obrigatorio":false,
 "gravavel_por_diretiva":[]}
```

**O de conexão diz, com todas as letras, o que não existe** — em vez de omitir
o campo e deixar quem pergunta achar que a versão é velha:

```json
{"escopo":"conexao","servidor":"127.0.0.1:7300","ip_origem":"127.0.0.1",
 "ligacao":1,"encryption":true,"encryption_exigida":false,"compression":false,
 "max_linhas":1000,"timeout_s":30,"somente_leitura":false,
 "gravavel_por_diretiva":[]}
```

**`ALTER SERVER SET`, com o `MOTIVO`, e o que ele ensina sobre reinício.**
`max_linhas` vale na hora; `timeout_s` fica gravado e espera:

```text
SQL> ALTER SERVER SET max_linhas = 500 MOTIVO 'pico de exportacao';
     gravado; o config.json em disco traz "max_linhas": 500 e o comentario
     "_nota" sobreviveu; a op `config` ja responde 500, sem reiniciar

SQL> ALTER SERVER SET timeout_s = 45 MOTIVO 'suporte';
     gravado; "exigem_reinicio": ["timeout_s"]

SQL> SHOW SERVER SETTINGS;
     {"recurso":"timeout_s","valor":30,"tipo":"inteiro",
      "aplica":"exige reinicio","a_quente":false,"escopo":"servidor",
      "no_arquivo":45,"esperando_reinicio":true}
     {"recurso":"max_linhas","valor":500,"tipo":"inteiro",
      "aplica":"a quente","a_quente":true,"escopo":"servidor"}
```

O par `no_arquivo`/`esperando_reinicio` só apareceu **exercitando**: o `SHOW`
respondia o valor vivo (30) depois de gravar 45, calado — o mesmo defeito que a
tela de Configurações já tinha pago uma vez, reaparecido pela porta nova.

**A diretiva por banco — o pedido 220 —, com o controle positivo que faz a
prova valer:**

```text
antes:  reindexar em erp  -> ok  {"ix_id":0,"ix_cli":0}

SQL> ALTER DATABASE erp SET comandos_proibidos = (reindexar) MOTIVO 'auditoria interna';
{"gravado":true,"escopo":"database","banco":"erp","recurso":"comandos_proibidos",
 "acrescentados":["reindexar"],"ja_existiam":[],
 "comandos_proibidos_da_base":["reindexar"],"aplica":"a quente",
 "aviso":"esta diretiva so ACRESCENTA; para retirar, edite
          seguranca.comandos_proibidos no config.json"}

depois: reindexar em erp  -> {"ok":false,"codigo":4001,
          "erro":"[SP000025] acesso negado: operacao reindexar esta proibida no banco erp"}
        reindexar em loja -> ok  {"ix":0}      <- a regra NAO vazou de banco
        excluir_tabela em erp e em loja -> "[SP000025] … proibida neste servidor"
                                              <- o global continua valendo em todos
```

Sem reiniciar o processo. No `config.json`, a entrada por banco entra na
**mesma lista** que já existia, como objeto ao lado das strings:

```json
"comandos_proibidos": ["excluir_tabela", {"comando":"reindexar","database":"erp"}]
```

**O diário, com os nove campos que você pediu**, gravado pelas duas portas — o
`ALTER … SET` e o `config_gravar` da tela:

```json
{"data_hora":"2026-09-07 17:56:16,911","quando_ms":1788803776911,
 "servidor":"127.0.0.1:7300","banco":"","recurso":"max_linhas",
 "valor_anterior":1000,"valor_novo":500,"usuario":"(token de servico)",
 "ip_origem":"127.0.0.1","motivo":"pico de exportacao"}
{"data_hora":"2026-09-07 17:56:16,912","quando_ms":1788803776912,
 "servidor":"127.0.0.1:7300","banco":"","recurso":"timeout_s",
 "valor_anterior":30,"valor_novo":45,"usuario":"(token de servico)",
 "ip_origem":"127.0.0.1","motivo":"suporte"}
```

**E o portão, com uma operadora de verdade** (senha com hash gerado pelo próprio
`phxsqld --senha`), recusada nas três portas — e o controle positivo de que ela
continua podendo ler, senão a prova só diria que o portão recusa tudo:

```text
SHOW SERVER SETTINGS                         -> [SP000025] acesso negado: olivia nao tem
ALTER SERVER SET max_linhas = 7              ->   permissao de administrar: as diretivas
ALTER DATABASE erp SET comandos_proibidos=…  ->   do servidor exigem esse poder
op "bancos"                                  -> ok
```

## O que NÃO existe, e é dispensa registrada

**17 das 39 diretivas do HFSQL não têm equivalente**, e cada uma com o motivo
técnico (o mapa completo, com a conta refeita por script, está em
`docs/DIRETIVAS.md` §2). As seis que mais importam:

- **`hActiveDirectory`** — não é um campo, é um protocolo (LDAP/Kerberos)
  contra um servidor de fora, e este motor tem **zero dependências externas**.
  O que existe é o cadastro próprio com PBKDF2-SHA256 e desafio-resposta.
- **`hlbActive` e os sete pesos `hlb*`** — o PhxSql tem **cluster com eleição e
  promoção automática**, não balanceador. O cluster resolve *quem é o master*
  quando um nó cai; o balanceador reparte carga de leitura entre réplicas. São
  problemas diferentes, e a nossa réplica não serve consulta a cliente.
- **`hMode2GB`** — é um limite do formato deles. Aqui o `.reg` é paginado por
  volume, e a partição já passou de 1.000.000 de linhas em dez volumes sem
  interruptor nenhum. Não há limite a destravar.
- **`hWindowsDiskCacheSize`** — é do cache de disco do Windows, e este servidor
  não fala com ele: a durabilidade se decide por `recursos.durabilidade` e pelo
  `fsync` da janela, que é portátil.
- **`Compression` da `Connection`** — não há compressão no fio, **e a premissa
  foi medida**: uma resposta real de `varrer` com 5.000 linhas tem **535.870
  bytes**, e o `deflate` desta casa (escrito à mão, sem crate) a leva a
  **55.284 — 9,69×**, em 4,02 ms. Ela **valeria** numa rede lenta (429 ms
  contra 44 ms a 10 Mbit/s) e seria puro custo no soquete local, onde a mesma
  varredura leva 3,9 ms. O que falta não é o compressor: é a **negociação** — o
  protocolo é uma linha JSON por pedido, e comprimir sem combinar quebra todo
  cliente antigo. Virou pedido, com o número na mão.
- **`ALTER TABLE … SET`** — não grava nada, e as duas recusas são decisão.
  `duplicate_check` ligado depois exige **provar que não há duplicata** varrendo
  a tabela inteira: isso é migração, não diretiva. E
  `SET referential_integrity = FALSE` desligaria a conferência de uma chave que
  **já aceitou linhas sob a garantia**, sem que nada no arquivo diga quais
  entraram com ela — a pétrea diz que *chave declarada nasce conferida*, e o
  caminho que existe é o `declarar_fk` com `"verificar": false`, escrito na
  declaração, quando ainda não há dado confiando nela.

E uma dispensa de texto: a mensagem *«o campo … não se grava pela tela»* ficou
como está, embora agora haja duas portas. Mudá-la envelheceria três saídas já
coladas (a resposta M deste PDF, o `PENDENCIAS.md` e a página dos pedidos), e o
texto continua verdadeiro — o caminho é o `config.json`.

## Como se refaz

```bash
cargo build --release -p phxsql-server --bin phxsqld
python3 bancada/diretivas/sql.py
```

Sobe dois `phxsqld` de verdade (7300 e 7301), exercita os quatro `SHOW` e os
quatro `ALTER`, abre o `config.json` em disco depois de cada gravação, lê o
`diretivas.log` linha a linha, mede a compressão que não existe e prova o
portão com uma operadora sem `administrar`. Os números vão para
`bancada/diretivas/resultados-sql.json`; o mapa está em `docs/DIRETIVAS.md`.
