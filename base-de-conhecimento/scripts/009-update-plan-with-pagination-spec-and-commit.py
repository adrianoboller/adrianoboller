# Update plan with pagination spec and commit
# 27/08 18:14

p='phxsql/docs/PLANO.md'
s=open(p).read()

# 1. Nota sobre o anexo repetido
s=s.replace('''## 2. FraseSQL: o contrato de integração

Lido o pacote `FraseSQL_5.zip` (v1.2, 3.032 linhas de Rust, 35 arquivos).''','''## 2. FraseSQL: o contrato de integração

Lido o pacote `FraseSQL_5.zip` (v1.2, 3.032 linhas de Rust, 35 arquivos).

> **Nota sobre o anexo.** O pacote foi enviado duas vezes, a segunda anunciada
> como "versão 2.0 evolução". Os dois arquivos são byte a byte idênticos
> (MD5 `ebd0a95c6a5f780c688986cd52e5a089`), e ambos declaram `version = "1.2.0"`
> no `Cargo.toml` e "v1.2" no `MANUAL.txt`. A 2.0 não chegou. Tudo abaixo se
> refere à 1.2.''')

# 2. Nova secao de paginacao, antes das questoes abertas
s=s.replace('''### 3.3 Reindex''','''### 3.3 Paginação de tabelas grandes

Definida no `CREATE TABLE`, com dois parâmetros:

| Parâmetro | Significado |
|---|---|
| `registros_por_arquivo` | quantos registros cabem em cada volume |
| `max_arquivos` | quantos volumes a tabela pode ter |
| `digitos` | largura do sufixo, padrão 3 (`_001`) |

Capacidade da tabela = `registros_por_arquivo x max_arquivos`.

```
cadastroClientes_001.reg
cadastroClientes_002.reg
cadastroClientes_003.reg
```

**O endereçamento continua sendo uma conta, não uma busca** — que é a
propriedade que faz o `.reg` valer a pena:

```
volume = (rowid - 1) / registros_por_arquivo + 1
slot   = (rowid - 1) % registros_por_arquivo + 1
offset = data_offset + (slot - 1) * slot_size
```

Três garantias sobrevivem intactas:

- **Ordem de digitação:** o volume N+1 vem sempre depois do N, e dentro de
  cada volume os slots continuam em ordem de inserção.
- **O rowid é global e nunca muda.** Ele não é "posição no volume", é posição
  na tabela; o volume sai dele por divisão.
- **O `.ndx` não muda em nada.** Ele já guarda rowid, e o rowid continua
  global. Nenhuma linha do código de índice precisa saber que existe volume.

#### O que pagina e o que não pagina

| Arquivo | Pagina? | Motivo |
|---|---|---|
| `.reg` | sim | cresce por quantidade de registros |
| `.bin` | sim | é o que mais cresce em bytes (fotos, anexos) |
| `.memo` | sim | idem |
| `.log` | sim | append-only, cresce para sempre |
| `.ndx` | **não** | é uma B+tree por índice sobre a tabela inteira; partir o arquivo partiria a árvore |

#### Consequência no ponteiro externo

O ponteiro gravado no `.reg` precisa passar a dizer em qual volume do `.bin` /
`.memo` o conteúdo está. Ele continua com 16 bytes, redistribuídos:

| Off | Tam | Campo |
|----:|----:|---|
| 0 | 6 | offset dentro do volume (u48 — 256 TB por volume) |
| 6 | 2 | número do volume (u16 — 65.535 volumes) |
| 8 | 4 | tamanho do conteúdo |
| 12 | 4 | CRC-32 do conteúdo |

É mudança de formato. Como estamos na versão 1 e não há nada em produção,
entra agora, em vez de virar uma migração depois.

#### Abertura preguiçosa

Uma tabela de 999 volumes não pode abrir 999 descritores de arquivo. Abre-se o
volume `_001` (que traz o esquema) e os demais sob demanda, com um cache LRU de
descritores. É exatamente o *lazy open* do `FileManager` do Clarion.

Cada volume carrega o cabeçalho completo, com o seu número, o
`registros_por_arquivo` e o total de volumes — se o `_001` se perder, os outros
ainda sabem dizer o que são.

#### Tabela cheia

`rowid > registros_por_arquivo x max_arquivos` devolve erro explícito
"tabela cheia", em vez do estouro silencioso de 2 GB que o TopSpeed dava.

### 3.4 Reindex''')

# 3. Requisito de paginacao na tabela de requisitos
s=s.replace('''| 11 | Hierarquia database → schema → tabela | a fazer |''','''| 11 | Hierarquia database → schema → tabela | a fazer |
| 12 | Paginação de tabelas grandes (`_001`, `_002`, ...) | a fazer |
| 13 | Quantidade de registros e de arquivos definida no `CREATE TABLE` | a fazer |''')

# 4. Ordem de trabalho atualizada
s=s.replace('''1. FK no `Schema` (o FraseSQL precisa para gerar JOIN, e o dicionário Clarion
   tem RELATION com CASCADE/RESTRICT)
2. Hierarquia database/schema/tabela em disco
3. `Tabela.log` e o comando `phxsql log`
4. Reindex e compactação
5. `config.json` e o servidor TCP na porta 5000
6. Servidor MCP
7. Camada SQL — Fase A ou B, conforme a decisão
8. ODBC — cliente ou driver, conforme a decisão
9. Integração no FraseSQL como `engine = "phxsql"`''','''**Não dependem de nenhuma decisão pendente — podem começar já:**

1. Paginação do `.reg`/`.bin`/`.memo`/`.log` e o novo formato do ponteiro
   (é mudança de formato: quanto antes, mais barato)
2. FK no `Schema` (o FraseSQL precisa para gerar JOIN, e o dicionário Clarion
   tem RELATION com CASCADE/RESTRICT)
3. Hierarquia database/schema/tabela em disco
4. `Tabela.log` e o comando `phxsql log`
5. Reindex e compactação
6. `config.json` e o servidor TCP na porta 5000

**Dependem das respostas da seção 4:**

7. Servidor MCP
8. Camada SQL — Fase A ou B, conforme a decisão
9. ODBC — cliente ou driver, conforme a decisão
10. Integração no FraseSQL como `engine = "phxsql"`''')
open(p,'w').write(s)
print("PLANO.md atualizado")
