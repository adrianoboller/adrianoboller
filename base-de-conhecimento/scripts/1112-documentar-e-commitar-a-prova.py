# Documentar e commitar a prova
# 29/08 11:15

import io
p='docs/DBLINK.md'
s=io.open(p,encoding='utf-8').read()
s += '''

## A prova contra um MySQL(R) de verdade (0.18.0)

Feita a pedido, na máquina da bancada, contra o **MySQL(R) 8.0.46** real — não
o servidor falso do teste de fio. O roteiro e o resultado, para quem refizer:

1. No MySQL(R): base `crm`, tabela `clientes` (BIGINT, VARCHAR, DECIMAL(12,2),
   DATE, chave primária e índice `porCidade`), 5 linhas, usuário `phx` com
   `caching_sha2_password` — **o padrão do 8.x, de propósito**, porque é o
   caminho difícil.
2. `dblink_salvar` + `dblink_testar` + `dblink_tabelas` + `dblink_ler` +
   `dblink_estrutura` pela porta de dados, e a mesma coisa pela tela.

**O que a prova achou, nos dois sentidos:**

- **Primeira tentativa recusada, e a recusa é a documentada**: o servidor pediu
  a autenticação *completa* do `caching_sha2_password`, que exige TLS ou a
  chave RSA — nenhum dos dois cabe sem dependência externa. O erro nomeia as
  duas saídas, e as duas foram provadas:
  - **caminho rápido**: uma conexão de qualquer cliente oficial aquece o cache
    do servidor e o nosso handshake passa (testado: versão, `current_user()` =
    `phx@127.0.0.1`, 0 ms);
  - **caminho durável**: `ALTER USER ... IDENTIFIED WITH mysql_native_password`
    — sobrevive ao reinício do mysqld, e é o que o manual recomenda para a
    ponte.
- `dblink_tabelas` viu `clientes` (InnoDB, 5 registros estimados);
  `dblink_estrutura` trouxe os tipos com precisão do DECIMAL e os dois índices;
  `dblink_ler` trouxe as 5 linhas ordenadas — e a grade da tela somou o limite
  (37.851,25) sobre dados que nunca estiveram num arquivo nosso.

**A distinção que importa**: isto é o **DbLink nativo**, escrito aqui, zero
dependências — o caminho «por protocolo» que `docs/MULTILINK.md` recomenda. O
pacote MULTILINK proprietário continua fora pelas 582 crates que ele arrasta;
esta prova mostra que o destino dele já é alcançável sem ele.
'''
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
