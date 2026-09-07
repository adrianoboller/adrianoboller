# S) exemplo de Tabela particionada por faixa de qtde de registros máximo ex 1000.000 de registros mas funcionando como se fosse uma tabela única para ficar leve o cadastro e na hora de usar é transparente para o select, insert, update, softdelete e delete

> Corrida em 2026-09-07 16:32:40 UTC · commit `a56a165` ·
> `target/release/phxsqld` · reproduzido por
> `python3 bancada/particao-por-faixa/sonda.py`

## Resposta curta

**Existe e responde.** `ModoParticao::PorQuantidade` (`docs/FORMATO.md` §8) já
era o modo **padrão** de paginação — volumes `Tabela_001.reg`…`Tabela_NNN.reg`,
endereço por **divisão** (`volume = (rowid-1)/registros_por_arquivo + 1`), sem
precisar de coluna de referência nenhuma. O pedido falava em 1.000.000 num
volume só; esta sonda gravou **1.000.000 de linhas em dez volumes de 100.000**
e exercitou as cinco operações em cima disso.

**O número que decide**: ler uma linha do **volume 10** custou **0,3417 ms**
contra **0,4882 ms** do **volume 1** (razão **0,70×** — não cresce, dentro do
ruído de medir microssegundos por rede+JSON). E uma diferença real em relação
à partição por letra (item R): aqui o **`INSERT` dentro de transação é
ACEITO**, porque o rowid alvo continua sendo `slots()+1` — a mesma conta de
uma tabela sem partição nenhuma —, e por isso **não existe** nenhuma recusa
análoga à do `UPDATE` que muda a letra: nada aqui decide o volume pelo
*conteúdo* da linha.

## Exemplo exercitado

### Criação e carga: 1.000.000 de linhas, 100.000 por volume

```json
{"op":"criar_tabela","database":"carga","tabela":"grande",
 "colunas":[{"nome":"id","tipo":"Int8"},
            {"nome":"nome","tipo":"Str(30)","obrigatoria":true}],
 "indices":[{"nome":"porId","colunas":["id"],"unico":true}],
 "registros_por_arquivo":100000,"max_arquivos":25,
 "particao":"faixa","softdelete":true}
```

```
=== 1. criar_tabela particao=faixa, registros_por_arquivo=100000

  [OK  ] criar_tabela particao=faixa registros_por_arquivo=100000

  carregando 1000000 linhas em lotes de 5000 (10 volumes previstos)...
  gravadas 1000000 linhas em 17.7s (56562 linhas/s)
```

### 2. Os dez volumes, no disco

```
=== 2. os arquivos que nasceram no disco

    grande_001.reg            7200512 bytes
    grande_002.reg            7200512 bytes
    grande_003.reg            7200512 bytes
    grande_004.reg            7200512 bytes
    grande_005.reg            7200512 bytes
    grande_006.reg            7200512 bytes
    grande_007.reg            7200512 bytes
    grande_008.reg            7200512 bytes
    grande_009.reg            7200512 bytes
    grande_010.reg            7200512 bytes
  total: 10 volumes de dados
```

### 3. `varrer` atravessa a fronteira de arquivo como se fosse uma tabela só, `buscar` acha em qualquer volume

```
=== 3. `varrer` atravessando a fronteira volume 1 / volume 2

  pedidos os rowids 99998..100003, devolvidos 6:
    rowid    99998  id=99998     volume_esperado=1
    rowid    99999  id=99999     volume_esperado=1
    rowid   100000  id=100000    volume_esperado=1
    rowid   100001  id=100001    volume_esperado=2
    rowid   100002  id=100002    volume_esperado=2
    rowid   100003  id=100003    volume_esperado=2
  -> rowids seguidos através da fronteira de arquivo: True. Uma UNICA chamada, um UNICO resultado; o `varrer` nao sabe que cruzou de arquivo.

  buscar por indice em tres volumes diferentes:
    id=6         (volume  1) -> achou=True volume_do_rowid=1
    id=400006    (volume  5) -> achou=True volume_do_rowid=5
    id=900006    (volume 10) -> achou=True volume_do_rowid=10
```

### 4. O custo: volume 1 × volume 10 (300 leituras cada, média)

```
=== 4. o custo de ler UMA linha no volume 1 x no ULTIMO volume

  rowid    50001 (volume 1)              : 0.4882 ms/leitura (media de 300)
  rowid   950001 (volume 10)             : 0.3417 ms/leitura (media de 300)
  razao volume_10 / volume_1 = 0.70x
  -> tem de ficar perto de 1,0x: o endereco e uma DIVISAO, nao uma busca que anda pelos volumes anteriores.
```

### 5. As cinco operações, no meio do último volume

```
=== 5. as cinco operacoes, direto no meio do arquivo maior

  [OK  ] SELECT (varrer) uma linha do ULTIMO volume
  [OK  ] UPDATE linha inteira, sem restricao de particao
  [OK  ] SOFTDELETE (excluir suave)
  [OK  ] DELETE fisico (excluir de vez)
  [OK  ] INSERT normal (fora de transacao) -- abre um volume novo alem do previsto
```

### 6. `INSERT` dentro de transação — ao contrário da letra (item R), é ACEITO

```
=== 6. INSERT dentro de uma transacao -- e ACEITO aqui

  [OK  ] begin
  [OK  ] inserir DENTRO da transacao
  [OK  ] commit
    -> apos o COMMIT a linha aparece pelo indice: True
```

### 7. O campo `esquema.volumes` vem vazio neste modo — achado exercitando, não lendo

```
=== 7. o `esquema`: paginacao aparece, `volumes` vem vazio

  paginacao: {"registros_por_arquivo": 100000, "max_arquivos": 25, "capacidade": 2500000, "digitos": 3, "modo": "quantidade", "baldes": null, "coluna": null, "bytes_por_arquivo": 1073741824}
  volumes (campo do protocolo): []
```

Isto **não é defeito**: `Table::reler_fronteiras` só lê o cabeçalho de cada
volume na partição por **período**, porque ali o corte depende do calendário.
Na partição por quantidade o volume sai de uma divisão — não há fronteira
nenhuma para ler do disco —, e por isso o protocolo não a calcula de graça. A
lista de volumes que existem de verdade vem do sistema de arquivos (parte 2
acima), não do `esquema`.

## O que NÃO existe, e é dispensa registrada

- **Nenhuma restrição análoga à do `UPDATE` que muda a letra.** Não existe
  porque não há como existir: nada aqui decide o volume pelo *valor* de uma
  coluna, então não há "mudar de balde" para recusar. É uma consequência
  estrutural do desenho, não uma lacuna.
- **O campo `esquema.volumes` não lista os volumes da partição por
  quantidade** (fica vazio — ver item 7 acima). Quem quer as fronteiras faz a
  MESMA conta que o motor faz (`primeiro_rowid(N) = (N-1) × registros_por_arquivo + 1`);
  não há operação do protocolo que devolva essa lista pronta para este modo.
- **Nenhuma tela mostra a divisão em volumes** desta partição — como no item
  R, é engenharia que responde sem interface por cima.

## Como se refaz

```bash
flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server
python3 bancada/particao-por-faixa/sonda.py
```

`PHX_SONDA_PORTA` (padrão 6710), `PHX_SONDA_LINHAS` (padrão 1.000.000) e
`PHX_SONDA_POR_VOLUME` (padrão 100.000). A carga completa levou 17,7 s nesta
máquina — bem dentro do teto de 5 minutos que o pedido original previu para
reduzir a 300.000, então o número aqui é o pedido inteiro (1.000.000), sem
redução. Detalhe em `bancada/particao-por-faixa/LEIA-ME.md`.
