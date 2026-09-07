# 0.2) Muitos bancos tem o sequence tipo o PostgreSQL e como é isso no phxsql?

*Medido em 2026-09-07 16:27 UTC, commit `a56a165`, contra o motor vivo.*

## Resposta curta

Existe, e é **da tabela, não do banco**: uma coluna do tipo `Sequence` — no
máximo **uma por tabela** — e um contador único que vive no cabeçalho do volume
1 do `.reg` (byte 36, 8 bytes). Inserir sem informar o valor numera sozinho;
inserir informando faz o contador **acompanhar** (o próximo sai depois do
maior gravado, o que o PostgreSQL não faz por padrão); a op `sequencias` lista
os contadores do banco e a `ajustar_sequencia` zera ou pula — só para quem
tem `administrar`, porque ajustar para trás faz a próxima inserção repetir
uma chave, e quem recusa a repetição é o **índice único**, não o contador.

O que **não** há: `CREATE SEQUENCE` solto, compartilhado entre tabelas, com
`nextval()`/`currval()` — a sequência do PhxSql é sempre a coluna de uma
tabela. É decisão de formato, e a §«rownum» do `docs/FORMATO.md` diz por quê:
o contador do `.reg` é único, e reservar essa vaga para o motor tiraria do
usuário um tipo que é dele.

## Exemplo exercitado

```text
sequencias -- 2026-09-07 16:27:08 UTC -- commit a56a165

=== 1. Declarar: a coluna e do tipo `Sequence` (uma so por tabela)
  [OK  ] criar_tabela com id Sequence
  [ERRO] segunda coluna Sequence na mesma tabela (tem de recusar) -- [SP000018] esquema invalido: a tabela tem 2 colunas Sequence (a, b), e so pode ter uma: o contador do `.reg` e unico

=== 2. Inserir SEM informar o id: o motor numera
  inserir Alves    -> ok=True rowid=1
  inserir Silva    -> ok=True rowid=2
  inserir Andrade  -> ok=True rowid=3
  linhas: [(1, 'Alves'), (2, 'Silva'), (3, 'Andrade')]

=== 3. Inserir INFORMANDO o id (o contador acompanha, como no PostgreSQL nao acompanha)
  [OK  ] inserir id=100
  o proximo automatico saiu: 101

=== 4. Listar os contadores: op `sequencias`
   {'tabela': 'pedidos', 'coluna': 'id', 'proxima': 102, 'registros': 5, 'tem_sequencia': True}

=== 5. Ajustar: op `ajustar_sequencia` (exige administrar)
  [OK  ] zerar (proxima=0)
  [OK  ] pular para 5000
  o proximo automatico saiu: 5000

=== 6. Ajustar para TRAS de uma chave ja gravada: o indice unico e quem recusa a repeticao
  [OK  ] ajustar para 1 (ja existe)
  [ERRO] inserir depois do ajuste (o id 1 ja existe: tem de recusar) -- [SP000020] chave duplicada: indice unico porId ja tem essa chave

=== 7. Onde o contador mora: byte 36 do cabecalho do volume 1 (o 92 e o do rownum)
  byte 36 proxima_sequencia = 1   byte 92 proximo_rownum = 7
  (o cabecalho vai ao disco no `sincronizar`, no fecho da janela de durabilidade;
   o valor em memoria e o que a op `sequencias` devolve)

=== 8. Reabrir e conferir que o contador sobreviveu
  (a persistencia do contador entre reaberturas e provada nos testes do phxsql-store)
```

O que cada bloco prova:

- **1** — a coluna `Sequence` se declara como qualquer outra, e a segunda na
  mesma tabela é recusada na declaração, com o motivo escrito.
- **2** — o motor numera 1, 2, 3 sem que o cliente mande o `id`.
- **3** — gravar `id=100` à mão faz o próximo automático sair **101**: o
  contador acompanha o maior gravado. No PostgreSQL um `INSERT` com valor
  explícito **não** avança a sequência, e a próxima automática colide.
- **4** — `sequencias` devolve tabela, coluna, o próximo número e quantos
  registros há.
- **5** — `ajustar_sequencia` com `proxima: 0` zera (o primeiro sai 1) e
  com `5000` pula.
- **6** — ajustar para trás de uma chave gravada é aceito (é ordem de
  administrador), e é o **índice único** que recusa a repetição na hora de
  inserir, com `SP000020 chave duplicada`.
- **7** — o contador está no byte **36** do cabeçalho; o byte **92** é outro
  contador, o do `rownum`. A primeira versão desta sonda leu o 92 e imprimiu
  o número errado — o instrumento antes do veredito.

## O que NÃO existe, e é dispensa registrada

- **Sequência solta** (`CREATE SEQUENCE`, `nextval`, uma sequência para
  várias tabelas) — não existe, por formato: o contador é do `.reg`.
- **`RETURNING id`** no SQL — o protocolo devolve o `rowid`; para saber o
  `id` gerado lê-se a linha (`ler` com o `rowid`), como a sonda faz.
- **Persistência entre reaberturas** — o cabeçalho vai ao disco no
  `sincronizar` (fecho da janela de durabilidade), e a sonda mostra o valor
  do disco atrasado em relação ao da memória; a prova de que sobrevive à
  reabertura está nos testes do `phxsql-store`, não nesta sonda.

## Como se refaz

```bash
flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server
python3 bancada/sequencias/sonda.py
```
