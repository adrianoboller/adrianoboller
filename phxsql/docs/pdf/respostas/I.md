# I) exemplo de script replicação

> Corrida em 2026-09-07T16:24Z–16:27Z UTC · commit `a56a165` ·
> `target/release/phxsqld` · reproduzido por
> `python3 bancada/replicacao/montar.py` + `python3 bancada/replicacao/medir.py`

## Resposta curta

Não há um "script de replicação" separado do motor: a replicação é **dois
blocos `config.json`** — um no lado que grava (papel `source`, imagem da
linha ligada) e um no lado que copia (papel `replica`, `somente_leitura`,
lista `origens` com host/porta/token/usuário/`senha_hash`) — e um laço que já
mora dentro do próprio `phxsqld`. A réplica **procura**; o master nunca
empurra nada — é o mesmo desenho do MySQL(R), pelo mesmo motivo de firewall.
`bancada/replicacao/montar.py` escreve os quatro `config.json` (um master,
três réplicas) e sobe os quatro processos; `medir.py` carrega, mata e retoma
uma réplica, e compara.

**Números desta corrida** (100.000 linhas, quatro `phxsqld` em
`127.0.0.1`, máquina **não** ocupada por outra medição —
`maquina_ocupada:false` no `resultados.json`): master grava a **33.883
linhas/s** com a imagem no diário; as três réplicas aplicam a **37.311
eventos/s** cada, em paralelo; atraso de uma escrita até as três (dominado
pelo `reconectar_em: 2s` do laço — sono, não trabalho): **1.134–2.012 ms**;
derrubada a `slave03`, ela volta a responder em **335 ms** e alcança 4.000
eventos em **0,3 s**; os **quatro retratos SHA-256 batem** no fim
(105.001 linhas). E a prova que decide: `posicao` nos quatro lados devolve
**exatamente 105.006 eventos / 105.001 registros**, os quatro iguais.

## Exemplo exercitado

### 1. O `config.json` do lado que grava (source), como o `montar.py` escreveu

```json
{
  "base": "base",
  "bind": "127.0.0.1:5800",
  "token": "espelho",
  "web": { "ligado": false },
  "replicacao": {
    "papel": "source",
    "imagem_da_linha": true,
    "id_servidor": "master"
  },
  "usuarios": [
    { "login": "adm", "nome": "Adriano", "id": 10,
      "senha_hash": "pbkdf2-sha256$210000$a37bfee7...$0243bc85...",
      "bases": { "*": { "ler": true, "inserir": true, "alterar": true,
        "excluir": true, "criar": true, "administrar": true,
        "diario": true, "verificar": true, "replicar": true } } }
  ]
}
```

### 2. O `config.json` do lado que copia (réplica `slave01`)

```json
{
  "base": "base",
  "bind": "127.0.0.1:5801",
  "token": "espelho",
  "web": { "ligado": false },
  "somente_leitura": true,
  "replicacao": {
    "papel": "replica",
    "id_servidor": "slave01",
    "imagem_da_linha": true,
    "origens": [
      { "nome": "master", "host": "127.0.0.1", "porta": 5800,
        "token": "espelho", "usuario": "adm",
        "senha_hash": "pbkdf2-sha256$210000$a37bfee7...$0243bc85...",
        "databases": ["loja"], "reconectar_em": 2 }
    ]
  },
  "usuarios": [ /* ... a mesma tríade admin do master ... */ ]
}
```

`slave02` e `slave03` são idênticos, só muda `bind`/`bind` da porta
(5802/5803) e `id_servidor`. A senha nunca aparece em claro em nenhum dos
quatro arquivos — `senha_hash` sai de `phxsqld --senha`.

### 3. Passo a passo, contra o motor vivo

```bash
$ python3 bancada/replicacao/montar.py /tmp/phx-f3-4084/replicacao
quatro servidores no ar em /tmp/phx-f3-4084/replicacao
  master  127.0.0.1:5800
  slave01 127.0.0.1:5801  puxando de master
  slave02 127.0.0.1:5802  puxando de master
  slave03 127.0.0.1:5803  puxando de master

$ PHX_REPLICACAO=/tmp/phx-f3-4084/replicacao python3 bancada/replicacao/medir.py 100000
carga inicial: 100000 linhas no master
  33,883 linhas/s no master (com a imagem no diario)
  as tres replicas alcancaram 2.7s depois do fim da carga
  37,311 eventos/s por replica, as tres em paralelo

operacao no master                 diario     ate as 3   resultado
------------------------------------------------------------------------------
1 insercao                         100001      2012 ms   iguais
1.000 insercoes em lote            101001      1134 ms   iguais
1 alteracao                        101002      1527 ms   iguais
1 exclusao suave                   101003      1523 ms   iguais
1 restauracao                      101004      1368 ms   iguais
1 exclusao fisica                  101005      1463 ms   iguais
1 linha com memo de 200 KB         101006      1557 ms   iguais

QUEDA E RETOMADA do slave03
  master gravou 4000 linhas com o slave03 derrubado (105006 eventos)
  voltou a atender em 335 ms e alcancou 4000 eventos em 0.3s desde o arranque

servidor      linhas  retrato
master        105001  72554b753253cd5d
slave01       105001  72554b753253cd5d
slave02       105001  72554b753253cd5d
slave03       105001  72554b753253cd5d
```

### 4. `posicao` dos dois lados — a prova de que a réplica ALCANÇOU o source

Pedido mandado a cada uma das quatro portas (login, depois `{"op":"posicao",
"database":"loja","com_esquema":false}`), saída real colada, uma linha por
servidor:

```
master  {"ok": true, "op": "posicao", "resultado": {"database": "loja", "papel": "source", "id_servidor": "master", "imagem_da_linha": true, "tabelas": {"clientes": {"eventos": 105006, "registros": 105001, "chave": "id"}}, "usuario": 10}, "ms": 0}
slave01 {"ok": true, "op": "posicao", "resultado": {"database": "loja", "papel": "replica", "id_servidor": "slave01", "imagem_da_linha": true, "tabelas": {"clientes": {"eventos": 105006, "registros": 105001, "chave": "id"}}, "usuario": 10}, "ms": 0}
slave02 {"ok": true, "op": "posicao", "resultado": {"database": "loja", "papel": "replica", "id_servidor": "slave02", "imagem_da_linha": true, "tabelas": {"clientes": {"eventos": 105006, "registros": 105001, "chave": "id"}}, "usuario": 10}, "ms": 0}
slave03 {"ok": true, "op": "posicao", "resultado": {"database": "loja", "papel": "replica", "id_servidor": "slave03", "imagem_da_linha": true, "tabelas": {"clientes": {"eventos": 105006, "registros": 105001, "chave": "id"}}, "usuario": 10}, "ms": 0}
```

Os quatro `eventos`/`registros` batem, byte a byte do mesmo número — é o que
prova convergência, não a contagem de linhas por si (duas tabelas podem ter o
mesmo total e conteúdo diferente; por isso a bancada também confere o SHA-256
de cada linha, acima).

## O que NÃO existe, e é dispensa registrada

- **Não existe um comando/script standalone de configuração** (tipo
  `CHANGE MASTER TO` do MySQL(R)). Montar a replicação é escrever o bloco
  `replicacao` no `config.json` de cada lado e (re)iniciar o `phxsqld` — não
  há operação de rede que grave essa configuração no arquivo do
  administrador (`docs/REPLICACAO.md` §13, item ☐ "escrever a configuração
  pela tela"). O assistente da tela (`docs/ASSISTENTE-REPLICACAO.md`)
  **entrega o bloco pronto para colar**, não aplica sozinho.
- **Não existe replicação síncrona nem quórum de escrita.** O `inserir` no
  master responde sem esperar réplica nenhuma; o commit não sabe se alguém
  copiou o dado. É o assunto inteiro do item T deste PDF.
- **Não existe ordem global entre tabelas.** A posição (`desde`/`ate`) é por
  tabela, porque não há transação que amarre tabelas diferentes — quando
  houver, entra um número de sequência do database inteiro (o campo
  reservado do cabeçalho do evento já está guardado para isso).
- **Não existe promoção automática nesta topologia simples** (master fixo +
  réplicas fixas, sem bloco `cluster`). Promover uma réplica aqui é manual
  (`spare_promover`) ou exige o bloco `cluster` — assunto do item J.
- **Não existe espera crescente na reconexão nem long-poll no source**
  (`docs/REPLICACAO.md` §13): o laço dorme um intervalo **fixo**
  (`reconectar_em`), e é esse sono — não o transporte — que domina o atraso
  medido acima (1.134–2.012 ms contra um transporte de menos de 1 ms, medido
  à parte no item T).
- **Uma réplica não pode ser escrita pela aplicação.** `somente_leitura`
  existe para isso: gravar nela quebraria a numeração do `rowid`, que é o que
  torna a réplica fiel sem transmitir nem negociar identidade nenhuma.

## Como se refaz

```bash
python3 bancada/replicacao/montar.py /tmp/algum-diretorio
PHX_REPLICACAO=/tmp/algum-diretorio python3 bancada/replicacao/medir.py 100000
# conferir os dois lados a mao, como no bloco 4 acima:
#   {"op":"login",...} depois {"op":"posicao","database":"loja"} nas portas 5800-5803
python3 bancada/replicacao/montar.py /tmp/algum-diretorio --derrubar
```
