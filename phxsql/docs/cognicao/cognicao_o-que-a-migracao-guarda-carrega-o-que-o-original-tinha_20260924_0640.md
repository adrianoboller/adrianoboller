# O que a migração guarda carrega o que o original tinha — a permissão, o link e até o nome do temporário

**Descoberto em 24/09/2026, ~06:40**, quando a revisão de segurança da etapa 2
do pedido 450 (`config.json` → `config.phz`) bloqueou o commit, com cada caso
provado pelo binário.

## 1. O que aconteceu

A migração `--empacotar-config` tinha a ordem certa (grava o `.phz`, relê,
compara, e só então tira o `.json` do caminho) e a promessa certa («nunca
apagado»). E deixava a configuração inteira legível de três jeitos:

- **A permissão viajava com o nome.** `config.json` em `644` virava
  `config.json.migrado-para-phz` em `644`. O `rename` não muda o modo, e ninguém
  mais regrava a cópia: o token ficava aberto para sempre.
- **O link viajava no lugar do arquivo.** Com `run/config.json` → `etc/config.json`,
  a troca renomeava o LINK, a saída dizia «o original foi renomeado», e o
  `etc/config.json` real ficava onde estava, `644`.
- **O temporário era o próprio config.** O `gravar_privado` fazia
  `with_extension("tmp")`. Com `--config servidor.tmp`, o temporário do
  `servidor.phz` era o `servidor.tmp` que se migrava: a gravação começava
  apagando-o, e o desfazer apagava o `.phz` — pasta **vazia**, e a mensagem
  dizendo «servidor.tmp continua valendo».

## 2. O que eu concluí primeiro, e estava errado

Que guardar por renomeação era neutro — «o arquivo continua o mesmo, só muda de
nome». É verdade sobre o CONTEÚDO, e foi só o conteúdo que eu conferi (byte a
byte, inclusive). A permissão e o tipo (arquivo ou link) também são do arquivo,
e também viajam — e eram justamente o que a etapa existia para proteger, porque
o parecer SEC já tinha dito que a autenticidade do config vem da permissão.

E que o `gravar_privado` era seguro por ser o molde provado da casa (achado A4,
17/09). Ele foi provado sobre a PERMISSÃO do temporário — `0600` desde o
primeiro byte —, não sobre o NOME. O nome saía de `with_extension`, que troca a
extensão: um nome derivado por troca de um nome que o usuário escolhe pode ser
qualquer outro nome, inclusive o dele.

## 3. O que a medição disse

- Pelo binário (revisão SEC): cópia em `644`; o alvo do link intocado e aberto;
  `--config meu.conf` sem aviso nenhum (o aviso procurava
  `meu.json.migrado-para-phz`); `--config servidor.tmp` com a pasta vazia.
- Depois do conserto, pelo `stat`: a cópia em `0600` nos dois sentidos da troca.
- Cada conserto com o defeito reposto: 7 vermelhos e 7 verdes, e 7 guardas
  novas no catálogo (`SEGURANCA.md` §23).

## 4. A regra

**Nome derivado de nome que o usuário escolhe se deriva por ACRÉSCIMO, nunca
por troca. E o que se guarda por renomeação carrega a permissão e o tipo do
original — confira os dois, não só o conteúdo.**

## 5. Como está guardado hoje

Pelas guardas `config-phz-copia-guardada-fica-aberta`, `config-phz-migra-o-link`,
`config-phz-aviso-procura-a-copia-pelo-lido`,
`gravar-privado-temporario-e-o-proprio-config` e
`config-phz-desfazer-apaga-a-unica-copia`. O buraco que fica: nenhuma régua
procura `with_extension` aplicado a caminho que vem do usuário. Procurado à mão
no `phxsql-server` depois do conserto, sobram três: as duas do `par` do
`config_phz.rs` são de propósito (o par `.json`/`.phz` É a troca da extensão),
e a terceira é um irmão do defeito, **anotado e não consertado nesta frente**:
`jobs.rs:574` deriva o log dos jobs por `with_extension("log")`, e com
`"jobs": "agenda.log"` no config o log seria o próprio cadastro.
