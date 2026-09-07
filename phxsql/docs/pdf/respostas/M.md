# M) exemplo script de diretivas de usuário para o gerenciamento do banco de dados e para o gerenciamento do acesso as tabelas

> Corrida em 2026-09-07T16:37:35Z UTC · commit `a56a165` ·
> `phxsqld 0.18.0 (a56a16563355-sujo) x86_64-unknown-linux-gnu` ·
> reproduzido por `python3 bancada/diretivas/provar.py` · **46 afirmações, 0 falhas**

## Resposta curta

O «script de diretivas» é o **`config.json`**: não há operação de protocolo que
crie usuário, então a diretiva é o arquivo — e ele carrega papel (`supervisor`),
direito **por banco** (as dez atividades) e direito **por tabela** dentro do
banco, com a regra da tabela **substituindo** a da base. Os hashes saem do
próprio binário (`echo -n '<senha>' | phxsqld --senha`, PBKDF2-SHA256 com
210.000 iterações) e a senha **nunca** aparece: nem na resposta do `login`, nem
na do `usuarios`, nem no `acessos.log`, nem no erro padrão — varrido e medido
abaixo. E as três operações que escondem a tabela do portão único —
`juntar`, `unir`, `pivotar` — foram pedidas com a tabela negada e as três
recusaram com `SP000025`.

## Exemplo exercitado

### 1. O script: gerar os hashes e escrever as três diretivas

```
  echo -n '<senha de ana>' | phxsqld --senha
    -> pbkdf2-sha256$210000$329a4a45aa02fb8ff5ff810...6c48f432
  echo -n '<senha de bruno>' | phxsqld --senha
    -> pbkdf2-sha256$210000$7b171e9bfa6df104e1e6980...e32af6aa
  echo -n '<senha de carlos>' | phxsqld --senha
    -> pbkdf2-sha256$210000$4fe446b9c39397cc7e20c2a...734edede
```

O cano (`echo -n | phxsqld --senha`) e não o argumento: assim a senha não fica
no histórico do shell nem aparece num `ps`.

O `config.json` que as três diretivas produzem — colado da saída da corrida,
com o hash mascarado **na impressão**, nunca no arquivo:

```json
{
  "usuarios": [
    {
      "id": 10,
      "nome": "Ana Administradora",
      "login": "ana",
      "senha_hash": "pbkdf2-sha256$210000$<sal>$<hash>",
      "email": "ana@empresa.com.br",
      "supervisor": true,
      "ativo": true
    },
    {
      "id": 11,
      "nome": "Bruno DBA da loja",
      "login": "bruno",
      "senha_hash": "pbkdf2-sha256$210000$<sal>$<hash>",
      "email": "bruno@empresa.com.br",
      "supervisor": false,
      "ativo": true,
      "bases": {
        "loja": {
          "ler": true,
          "inserir": true,
          "alterar": true,
          "excluir": true,
          "criar": true,
          "reindexar": true,
          "diario": true,
          "verificar": true,
          "administrar": true
        }
      }
    },
    {
      "id": 12,
      "nome": "Carlos Consulta",
      "login": "carlos",
      "senha_hash": "pbkdf2-sha256$210000$<sal>$<hash>",
      "email": "carlos@empresa.com.br",
      "supervisor": false,
      "ativo": true,
      "bases": {
        "loja": {
          "ler": true,
          "inserir": true,
          "alterar": true,
          "tabelas": {
            "folha": {}
          }
        }
      }
    }
  ]
}
```

São **três diretivas** e três desenhos diferentes:

- **ana** — `"supervisor": true`. Administradora do sistema: pode tudo, em toda
  base, em toda tabela.
- **bruno** — administrador de **um banco só**. Tem as dez atividades em `loja`,
  inclusive `administrar` (que é o que `excluir_tabela` exige). Não tem `rh` e
  não tem `"*"`: base fora da diretiva **nega tudo**.
- **carlos** — usuário comum, com o direito por **tabela**. Lê e grava em `loja`,
  **menos** em `folha`: `"folha": {}` é a regra daquela tabela, e ela
  **substitui** a da base em vez de somar.

### 2. O cadastro, conferido pelo próprio binário

```
  $ phxsqld --config <o arquivo acima> --usuarios
    login          nome                     nivel      ativo    poder por base
    root           Administrador do sistema supervisor sim      (supervisor: tudo em toda base)
    ana            Ana Administradora       supervisor sim      (supervisor: tudo em toda base)
    bruno          Bruno DBA da loja        nenhum     sim      loja=ler+inserir+alterar+excluir+criar+reindexar+diario+verificar+administrar
    carlos         Carlos Consulta          nenhum     sim      loja=ler+inserir+alterar
```

Repare no que **não** aparece: nenhuma senha e nenhum hash.

### 3. O login, e a resposta crua de cada um

```json
{"ok":true,"op":"login","resultado":{"id":10,"nome":"Ana Administradora","login":"ana","email":"ana@empresa.com.br","telefone":"","nivel":"admin","supervisor":true,"ativo":true,"ex…
{"ok":true,"op":"login","resultado":{"id":11,"nome":"Bruno DBA da loja","login":"bruno","email":"bruno@empresa.com.br","telefone":"","nivel":"nenhum","supervisor":false,"ativo":tru…
{"ok":true,"op":"login","resultado":{"id":12,"nome":"Carlos Consulta","login":"carlos","email":"carlos@empresa.com.br","telefone":"","nivel":"nenhum","supervisor":false,"ativo":tru…
```

Sem `login`, o token sozinho não passa do portão 2 — havendo cadastro, ele é a
chave da porta da **rede**, não a identidade de ninguém:

```json
{"ok": false, "op": "varrer", "erro": "[SP000025] acesso negado: faca login antes: {\"op\":\"login\",\"usuario\":...,\"senha\":...}", "codigo": 4001, "nome": "ACESSO_NEGADO"}
```

### 4. Cada diretiva vale exatamente o que ela diz

```
  -- ana, supervisora: pode tudo, em toda base
  [OK  ] ana le loja.folha
  [OK  ] ana le rh.cargos

  -- bruno, administrador da loja e de mais nada
  [OK  ] bruno CRIA tabela em loja (tem 'criar')
  [OK  ] bruno APAGA tabela em loja (tem 'administrar')
  [OK  ] bruno NAO le rh (base fora da diretiva nega tudo)

  -- carlos, uma tabela permitida e uma negada NO MESMO BANCO
  [OK  ] carlos LE loja.clientes (a diretiva da base vale)
  [OK  ] carlos NAO le loja.folha (a regra da tabela SUBSTITUI a da base)
  [OK  ] carlos NAO grava em loja.folha
  [OK  ] carlos GRAVA em loja.clientes
     tabelas que carlos enxerga: ['clientes']
  [OK  ] a arvore de carlos ESCONDE folha
```

As recusas, cruas:

```json
{"ok": false, "op": "varrer", "erro": "[SP000025] acesso negado: bruno nao tem permissao de ler em rh.cargos", "codigo": 4001, "nome": "ACESSO_NEGADO", "classe": "acesso", "sprint": "SP000025"}
{"ok": false, "op": "varrer", "erro": "[SP000025] acesso negado: carlos nao tem permissao de ler em loja.folha", "codigo": 4001, "nome": "ACESSO_NEGADO", "classe": "acesso", "sprint": "SP000025"}
```

E a árvore **esconde** o que ela não abre: `tabelas` devolveu `['clientes']` e
não `['clientes','folha']`. Não é enfeite — o nome de uma tabela já conta parte
da história.

### 5. As três portas dos fundos, e o controle positivo delas

O portão de permissão é **um só**, e ele lê o campo `"tabela"` do pedido. Três
operações escondem a tabela dele — `juntar` a guarda em `a.tabela`/`b.tabela`,
`unir` numa **lista**, e `pivotar` põe as tabelas de consulta **dentro** de cada
item de `juntar`. As três pagam conferência própria, e a bateria pede a tabela
negada pelos três caminhos.

**Controle positivo primeiro** — uma conferência que recusasse tudo protegeria
igual e quebraria o motor:

```
  [OK  ] juntar clientes x clientes passa
  [OK  ] unir clientes+clientes passa
  [OK  ] pivotar sobre clientes passa
```

E então a tabela negada, pelos três caminhos e mais o SQL:

```json
juntar  {"ok": false, "op": "juntar",  "erro": "[SP000025] acesso negado: carlos nao tem permissao de ler em loja.folha", "codigo": 4001, "nome": "ACESSO_NEGADO"}
unir    {"ok": false, "op": "unir",    "erro": "[SP000025] acesso negado: carlos nao tem permissao de ler em loja.folha", "codigo": 4001, "nome": "ACESSO_NEGADO"}
pivotar {"ok": false, "op": "pivotar", "erro": "[SP000025] acesso negado: carlos nao tem permissao de ler em loja.folha", "codigo": 4001, "nome": "ACESSO_NEGADO"}
sql     {"ok": false, "op": "sql",     "erro": "[SP000025] acesso negado: carlos nao tem permissao de ler em loja.folha", "codigo": 4001, "nome": "ACESSO_NEGADO"}
```

Os pedidos exatos que produziram as três recusas:

```json
{"op":"juntar","database":"loja","a":{"tabela":"clientes","chave":"id"},"b":{"tabela":"folha","chave":"id"}}
{"op":"unir","database":"loja","tabelas":["clientes","folha"]}
{"op":"pivotar","database":"loja","tabela":"clientes","juntar":[{"tabela":"folha","coluna":"id","prefixo":"f","chave":"id"}],"linhas":[{"campo":"f.nome"}],"colunas":[{"campo":"nome"}],"agregador":"contagem"}
```

### 6. Senha nunca em texto puro — a varredura, e o controle dela

A resposta crua do `usuarios`, primeiros 300 bytes:

```json
{"ok":true,"op":"usuarios","resultado":[{"id":1,"nome":"Administrador do sistema","login":"root","email":"root@empresa.com.br","telefone":"","nivel":"admin","supervisor":true,"ativo":true,"exige_chave":false,"bases":{},"tabelas":{}},{"id":10,"nome":"Ana Administradora","login":"ana","email":"ana@emp…
```

```
  [OK  ] controle: o varredor ACHA a senha quando ela esta no texto
  [OK  ] nenhuma senha em texto puro em a resposta do usuarios
  [OK  ] nenhuma senha em texto puro em o acessos.log
  [OK  ] nenhuma senha em texto puro em o erro padrao do servidor
  [OK  ] nenhum hash pbkdf2 na resposta do usuarios
```

O **controle** vem primeiro por lei da casa: uma varredura que dá zero prova
primeiro que acha o que existe, senão o zero não vale.

O `acessos.log` guarda o login e registra **toda** tentativa, inclusive as
negadas — e nenhuma senha:

```json
{"quando":"2026-09-07 16:37:37,289","quando_ms":1788799057289,"ip":"127.0.0.1","porta_origem":56512,"op":"login","usuario":"carlos","autenticado":true,"ok":true,"ms":274}
```

## O que NÃO existe, e é dispensa registrada

- **Não há operação de protocolo que crie, altere ou apague usuário.** O
  `config_gravar` só aceita a lista fechada de `CAMPOS_EDITAVEIS`, e `usuarios`
  não está nela. Medido nesta rodada, 2026-09-07 16:47 UTC:

  ```json
  {"token":"t","op":"config_gravar","campos":{"usuarios":[{"login":"novo"}]}}
  {"ok":false,"op":"config_gravar","erro":"[SP000025] acesso negado: o campo \"usuarios\" nao se grava pela tela; edite o config.json","codigo":4001,"nome":"ACESSO_NEGADO","classe":"acesso","sprint":"SP000025","repetir":false,"ms":0}
  ```

  O «script de diretivas» é, por isso, um script que
  **escreve o `config.json`** e reinicia o servidor, e não uma sequência de
  `CREATE USER` / `GRANT`. É a decisão que está no `docs/USUARIOS.md`, e ela
  tem preço: mudar direito exige reinício.
- **Sem troca de senha pelo protocolo**, pela mesma razão.
- **Sem grupos nem papéis nomeados.** O poder é por usuário. Com vinte
  operadores iguais, são vinte blocos iguais no `config.json`. `supervisor` é a
  única coisa parecida com um papel.
- **Sem direito por COLUNA.** O direito desce até a tabela e para aí: esconder
  a coluna de salário dentro de uma tabela que a pessoa pode ler não existe.
- **A senha viaja em claro no pedido de `login`**, como todo o resto do
  protocolo — medido nesta corrida e registrado aqui, não descoberto depois. A
  porta 5000 pertence dentro de VPN ou IPSec.
- **`bruno` administra `loja`, mas `administrar` sem base é outra coisa.**
  Operações que não falam de banco — `usuarios`, `config`, `ips` — caem na
  regra da base vazia, e bruno não tem `"*"`: ele não as alcança. Quem precisa
  disso pede `"*": {"administrar": true}` explicitamente.

## Como se refaz

```bash
flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server
python3 bancada/diretivas/provar.py
```

Sobe um `phxsqld` na porta 6505 (troque com `PHX_F5_PORTA_DIRETIVAS`), escreve
tudo em `/tmp/phx-f5d-<pid>` e apaga no fim. Os números vão para
`bancada/diretivas/resultados.json`.
