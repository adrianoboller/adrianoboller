# A bancada da cifra do fio

```bash
cargo build --release
python3 bancada/cifra-do-fio/prova.py
```

Sobe um `phxsqld` **próprio** nas portas **7210** (dados) e 7211 (web, mas
desligada), e mata **só o processo que ela mesma criou, pelo PID**. Nunca
`pkill` — pode haver `phxsqld` de outra pessoa na máquina.

O desenho e o argumento de cada decisão estão em
[`docs/CIFRA-DO-FIO.md`](../../docs/CIFRA-DO-FIO.md).

## Por que ela existe, se já há teste em Rust

`crates/phxsql-server/tests/cifra-do-fio.rs` já prova o laço de conexão pelo
soquete. Esta bancada dá duas coisas que aquele teste não consegue dar:

### 1. Independência

O X25519, o ChaCha20-Poly1305, o HKDF e a máquina do aperto estão escritos
**de novo, em Python, só com a biblioteca padrão**. Quando os dois lados
fecham o aperto, isso deixa de ser «o mesmo código concordando consigo mesmo»
e passa a ser **duas implementações independentes chegando à mesma chave**.

Isso **não é** interoperabilidade certificada com o Noise — nada aqui foi
rodado contra os vetores do *cacophony*, e o documento diz isso na §9. É
evidência boa, e o que ela vale está escrito.

O cliente Python confere os **vetores da RFC 7748** antes de qualquer outra
coisa. Sem esse passo, um erro no cliente apareceria no relatório como «o
servidor Rust está errado».

### 2. O sistema operacional

Cortar o fio de verdade, com o servidor num **processo separado**, é a única
forma de provar que o corte vira **erro** e não fim de sessão. Um teste de
unidade não prova queda de conexão — a lição do `BULKINSERT`.

## A armadilha que esta casa já pagou, e que este arquivo NÃO repete

`socket.makefile()` do Python **segura o descritor**: fechar só o soquete
deixa o `fd` aberto, o servidor nunca vê o fim da conexão, e o teste passa por
engano. Um teste que passa por engano é pior que um teste que falta.

Por isso não há `makefile` em lugar nenhum aqui: o buffer de linha é feito à
mão sobre `recv`, e `Cliente.fechar()` fecha o soquete e mais nada existe para
segurar o descritor.

## O que ela prova, na ordem

O resultado esperado de cada passo está escrito **antes** de rodar — é o que
separa prova de demonstração.

| # | o que prova |
|---|---|
| 0 | os vetores da RFC 7748 no próprio cliente Python |
| 1 | o cliente **velho**, que nunca ouviu falar do aperto, grava e lê como sempre — e o servidor **não** cria a chave do fio |
| 2 | o aperto fecha entre o Python e o Rust, e o mesmo trabalho acontece dentro do túnel |
| 3 | a chave apresentada é a que `phxsqld --chave-do-fio` imprime — é ela que o cliente pina |
| 4 | o **pino errado** derruba o aperto, no cliente, antes de qualquer pedido |
| 5 | registro **repetido** não é atendido: o contador por direção vale |
| 6 | **fio cortado vira erro no log; despedida não** — os dois vereditos, contados no `acessos.log` |
| 7 | registro **truncado** (metade de uma linha, e o soquete morre) também vira erro |
| 8 | com `exigir` ligado: claro é recusado com erro nomeado e a conexão fecha; o túnel continua trabalhando |
| 9 | **diagnóstico, não prova**: quanto o túnel engorda o fio. É daqui que sai a tabela da §6 do `CIFRA-DO-FIO.md` — o documento chegou a dizer «+33%», que é a expansão do Base64 no limite e **não** o que se paga num pedido curto |

O passo 6 é o que mais custa acertar: ele conta as linhas `"op":"fio"` do
`acessos.log` **antes e depois**, e exige exatamente **uma** — a do corte. Se a
despedida também virasse erro, ou se o corte passasse por fim limpo, a conta
não fecharia.

E ele espera **por condição**, e não por tempo fixo: o log é escrito na thread
da conexão, e dormir um tempo fixo é o teste que passa nesta máquina e falha
na próxima.

## Código de saída

`0` tudo passou; `1` alguma conferência reprovou (o relatório nomeia quais).
