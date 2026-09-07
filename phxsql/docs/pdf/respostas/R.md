# R) exemplo de Tabela particionada por campo texto chave ex nome pega a primeira letra e cria uma paginação de A Z para ficar leve o cadastro e na hora de usar é transparente para o select, insert, update, softdelete e delete

> Corrida em 2026-09-07 16:44:54 UTC · commit `a56a165` ·
> `target/release/phxsqld` · reproduzido por
> `python3 bancada/alfanumerica/sonda.py`

## Resposta curta

**Existe e responde.** `ModoParticao::PorLetra` (`docs/FORMATO.md` §8) parte a
tabela em **37 volumes fixos** — `_A`.._Z`, `_0`.._9`, `_Outros` — pela
primeira letra de uma coluna de referência, com acento dobrado na letra sem
acento (`Ávila` → `_A`). As cinco operações do pedido passam pela mesma porta
de sempre — `select`/`insert`/`update`/`softdelete`/`delete` não sabem que a
tabela é particionada — com **duas recusas de propósito**: `UPDATE` que
mudaria a letra, e `INSERT` dentro de transação.

**O número que responde "para ficar leve"**: ler a letra `Z` toca **200**
linhas de **5.200**, e custa **12,0 ms** contra **9,4 ms** da letra `A` — não
cresce com a posição no alfabeto. **O que falta**: o paginador A–Z **na tela**
— a canalização (`esquema` devolve `primeiro_rowid`/`registros` por balde,
`varrer` aceita `depois`+`max`) está pronta, nenhuma tela oferece as 26 letras
para clicar.

## Exemplo exercitado

### 1. As cinco operações, numa tabela `clientes` particionada por `nome`

```json
{"op":"criar_tabela","database":"loja","tabela":"clientes",
 "colunas":[{"nome":"id","tipo":"Int8"},
            {"nome":"nome","tipo":"Str(60)","obrigatoria":true},
            {"nome":"cidade","tipo":"Str(40)"}],
 "indices":[{"nome":"porId","colunas":["id"],"unico":true}],
 "registros_por_arquivo":1000,"particao":"letra","particao_coluna":"nome",
 "softdelete":true}
```

Saída real:

```
=== 1. As cinco operacoes numa tabela particionada por letra

  [OK  ] criar_tabela particao=letra particao_coluna=nome

  [OK  ] inserir 'Alves'              rowid=1
  [OK  ] inserir 'Silva'              rowid=18001
  [OK  ] inserir 'Ávila'              rowid=2
  [OK  ] inserir '9 de Julho Ltda'    rowid=35001
  [OK  ] inserir '@estranho'          rowid=36001
  [OK  ] inserir 'Andrade'            rowid=3

  os arquivos que nasceram:
    clientes_9.reg               718 bytes
    clientes_A.reg              1002 bytes
    clientes_Outros.reg          718 bytes
    clientes_S.reg               718 bytes

  varrer devolve as 6 linhas como UMA tabela so:
    rowid      1  rownum  1  Alves
    rowid      2  rownum  3  Ávila
    rowid      3  rownum  6  Andrade
    rowid  18001  rownum  2  Silva
    rowid  35001  rownum  4  9 de Julho Ltda
    rowid  36001  rownum  5  @estranho

  [OK  ] atualizar linha INTEIRA, mesma letra
  [ERRO] atualizar mudando a letra (A -> Z) -- [SP000018] esquema invalido: a alteracao mudaria o balde de A para Z, e o balde e o endereco fisico da linha em clientes. Exclua e insira de novo: a l
  [OK  ] excluir SUAVE
  [OK  ] excluir DE VEZ
  depois das duas exclusoes: ['Alves', 'Andrade', '9 de Julho Ltda', '@estranho']

  [OK  ] begin
  [ERRO] inserir DENTRO da transacao -- [SP000018] esquema invalido: loja.clientes e particionada por letra, e ali o slot da linha depende do balde: o rowid alvo nao e previsivel fora do motor, e sem ele a marca de recuperacao nao e idempotente. Insira nesta t
```

`rowid = (balde-1) × registros_por_arquivo + slot`: Alves (balde A=1) → rowid
1; Silva (balde S=19) → 18001 = 18×1000+1; `9 de Julho Ltda` (balde `9`=27,
dígitos ficam no bloco `0..9`) → 35001; `@estranho` (balde `Outros`=37) →
36001. **Ávila cai no MESMO balde `_A` que Alves** — a tabela de dobra de
acento provou-se ao vivo, não só no papel.

### 2. O controle: o `atualizar` parcial já é recusado numa tabela SEM partição

```
=== 2. CONTROLE -- o `atualizar` parcial na tabela SEM particao

  [ERRO] atualizar SO a cidade, tabela normal -- [SP000018] tipo invalido: coluna nome e obrigatoria e recebeu NULL
    -> se isto tambem recusa, `atualizar` e linha INTEIRA em toda tabela,
       e a recusa na particionada nao e defeito dela.
```

Sem este controle, a leitura correta acima ("update parcial recusa") pareceria
defeito da partição — e não é: `atualizar` é linha inteira no PhxSql inteiro.

### 3. A paginação A–Z: 5.200 linhas, 26 letras, custo por balde

```
=== 3. A paginacao A-Z: 200 linhas por letra, 26 letras

  gravadas 5200 linhas em 0.15s
  baldes com linha: 26  (modo=letra, coluna=nome)

  UM balde so, pela composicao que o protocolo JA permite:
    esquema -> primeiro_rowid do balde; varrer(depois=primeiro_rowid-1, max=registros)
    letra A: devolvidas= 200 examinadas= 200 so_desta_letra=True     9.4 ms
    letra M: devolvidas= 200 examinadas= 200 so_desta_letra=True    14.3 ms
    letra Z: devolvidas= 200 examinadas= 200 so_desta_letra=True    12.0 ms
    -> o custo da ULTIMA letra tem de ser igual ao da primeira. Se crescer,
       a leitura esta varrendo o cadastro inteiro e a particao nao pagou nada.

  campo desconhecido -- recusa, ou engole calado?
    varrer + {'balde': 'A'}: ok=True devolvidas=5
    varrer + {'letra': 'A'}: ok=True devolvidas=5
    varrer + {'xyzzy': 1}: ok=True devolvidas=5
    -> `xyzzy` esta aqui como CONTROLE: se ele tambem passa, a tolerancia
       e do protocolo inteiro e nao um buraco da particao.
```

`xyzzy` passa igual a `balde`/`letra` — a tolerância a campo desconhecido é do
protocolo inteiro, não um buraco da partição.

## O que NÃO existe, e é dispensa registrada

- **O paginador A–Z na tela.** A canalização já entrega tudo que uma tela
  precisaria — `esquema` devolve `primeiro_rowid`/`registros` de cada um dos
  37 baldes, e `varrer(depois, max)` já lê um balde só —, mas nenhuma tela do
  PhxSql oferece as 26 letras (mais dígitos/Outros) para clicar. É trabalho de
  interface sobre uma engenharia que já responde, não um item de motor aberto.
- **`INSERT` numa tabela por letra dentro de transação é recusado de
  propósito**, e não é lacuna a fechar: o balde (e por consequência o rowid)
  só se descobre executando a regra de partição, e a marca `.tx` de
  recuperação (`docs/TRANSACOES.md` §5.1) precisa do rowid alvo **antes** da
  passada de commit para ser idempotente. A mensagem já diz o caminho: insira
  fora de transação.
- **`UPDATE` que mudaria a letra de referência é recusado de propósito**, pelo
  mesmo motivo da regra primordial de integridade em espírito: mover a linha
  de arquivo trocaria seu endereço físico (rowid), que é a identidade dela em
  todo índice. O caminho é excluir e inserir de novo — a linha nova nasce no
  balde certo, com rowid novo.

## Como se refaz

```bash
flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server
python3 bancada/alfanumerica/sonda.py
```

`PHX_SONDA_PORTA` (padrão 5479) e `PHX_SONDA_POR_LETRA` (padrão 200) mudam
porta e volume da carga. Detalhe em `bancada/alfanumerica/LEIA-ME.md`.
