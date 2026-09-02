# Dois PhxSql em contêiner: o canal direto, e o que o DbLink não alcança

    python3 bancada/docker/montar-dois.py      # sobe phx-a e phx-b
    python3 bancada/docker/exercitar-dois.py   # exercita e mede
    docker rm -f phx-a phx-b                   # derruba

## O arranjo, e por que ele tem de ter as duas metades

| | papel | base própria | tabelas |
|---|---|---|---|
| **phx-a** | `source` | `loja` | `clientes`, `pedidos` |
| **phx-b** | `replica` de A | `rh` | `funcionarios`, `cargos` |

Se `phx-b` só tivesse a `loja`, isto seria replicação e não dois bancos. Se só
tivesse o `rh`, seriam dois servidores isolados e não comunicação. **As duas
juntas** é o que a prova exige — e é o que ela confere.

## Medido em 02/09/2026

- **A `loja` chegou em `phx-b` em ~8 s**, pelo nome do serviço na rede do
  Docker, e o `rh` dele continuou intacto: `["loja","rh"]` de um lado,
  `["loja"]` do outro.
- **Imagem `FROM scratch`, 12,8 MB.** Sem shell, sem gerenciador de pacotes.
- **O `phxsqld` estático tem 7,66 MB com `strip`** — o comentário do
  `Dockerfile` diz **3,4 MB**, número que envelheceu.

## O DbLink NÃO fala PhxSql, e isso é medido

O pedido era «dblink entre eles». Ele não existe, e a recusa é limpa:

    {"op":"dblink_salvar","motor":"phxsql",…}
    → [SP000018] motor de dblink desconhecido: "phxsql" (use "mysql" ou "postgres")

O DbLink existe para alcançar **motor de fora** — MySQL/MariaDB e PostgreSQL.
Entre dois PhxSql o canal é a **replicação**, que é nativa e é a que este
roteiro exercita.

E uma coisa que a prova conferiu de graça, apontando o DbLink para uma porta
que não fala MySQL: ele **não trava o servidor**. Devolve
`leitura falhou: Resource temporarily unavailable` em **10,2 s**, e durante
essa espera outra conexão fez login e listou bancos em **0,32 s** — a tentativa
não segura a trava global.

## E o DbLink provado contra o motor que ele ALCANÇA

    python3 bancada/docker/dblink-mariadb.py   # sobe erp-mariadb e prova

Terceiro contêiner, terceira base, terceiras tabelas: `erp` com `fornecedores`
e `notas`. Medido em 02/09/2026, de dentro do `phx-a`:

| | |
|---|---|
| `dblink_testar` | **0,01 s** · `11.8.9-MariaDB-ubu2404` · usuário efetivo `leitor@%` |
| `dblink_bancos` | `["erp","information_schema"]` |
| `dblink_tabelas` | `fornecedores` (3) e `notas` (3), com motor e bytes |
| `dblink_ler` | as três linhas, com tipo e `primaria: true` no `id` |
| `dblink_consultar` | `LEFT JOIN` + `GROUP BY` rodando **lá**, resultado chegando aqui — inclusive o `SUM` nulo do fornecedor sem nota |

E o `phx-a` continua com a base dele: `bancos` → `["loja"]`. O DbLink **lê de
fora**, não importa para dentro.

**A senha não vaza.** Ela entra por `senha_env` — variável de ambiente do
contêiner —, e a ficha da ligação devolve `senha: "(oculta)"`.

**O contraste que vale guardar:** `dblink_testar` contra o MariaDB responde em
**0,01 s**; contra a porta de um PhxSql, que não fala o fio do MySQL, ele
desiste em **10,2 s** com erro nomeado. Os dois números são o mesmo recurso
funcionando.

## O que este roteiro NÃO prova

O `Dockerfile` do projeto não foi usado inteiro: o estágio construtor falha
**neste ambiente** porque o `rustup target add x86_64-unknown-linux-musl`
dentro do contêiner não alcança `static.rust-lang.org` — a rede do build não
passa pelo proxy da sessão. Os binários vêm compilados de fora, com o mesmo
alvo e o mesmo `strip`. Então isto prova a imagem e o comportamento; **não**
prova o `--offline` do construtor, que é a linha que pegaria uma dependência
externa entrando sem ninguém notar.
