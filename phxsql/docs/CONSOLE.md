# `phxsqlcmd` — o console

Um prompt que fala o protocolo JSON com um servidor PhxSql. Uma linha digitada
vira um pedido; a resposta vira uma tabela que dá para ler.

```bash
phxsqlcmd --host 10.1.1.102 --porta 5000 --token segredo --usuario adriano
PHXSQL_SENHA='a senha' phxsqlcmd --usuario adriano --database loja
phxsqlcmd --comando 'tabelas database=loja'      # roda uma linha e sai
```

```text
phxsql> bancos
phxsql> /use loja
phxsql> tabelas
phxsql> ler tabela=clientes rowid=42
phxsql> inserir tabela=clientes valores={"id":9,"nome":"Ana Maria"}
phxsql> buscar tabela=clientes indice=porId chave=[42]
phxsql> SELECT nome, cidade FROM clientes LIMIT 20
phxsql> {"op":"ping"}
phxsql> /help
phxsql> /help buscar
phxsql> /sair
```

---

## O que ele NÃO tem, e por que isso está no `--help`

**Histórico e setas não existem nesta rodada.** A seta para cima escreve `^[[A`
na tela em vez de trazer o comando anterior; `ctrl+R` não procura nada; não dá
para editar no meio da linha.

Um `readline` de verdade é o terminal em **modo cru** — ler byte a byte,
interpretar as sequências ANSI, redesenhar a linha, manter o histórico. Isso é
uma crate, e a regra do projeto é zero dependências externas. `std::io::stdin()
.read_line` faz o resto do trabalho, e o resto do trabalho é o que o console
existe para fazer.

Está escrito no `--help` de propósito: descobrir sozinho, apertando a seta e
vendo `^[[A` aparecer, é pior do que ler que não tem.

## O `/help` vem do servidor — e essa é a decisão que sustenta o resto

Não existe uma lista de comandos dentro do console. `/help` é a op `catalogo`
**pela rede**, e `/help <operacao>` é a mesma op com o campo `operacao`.

Uma lista aqui envelheceria calada, e o console passaria a documentar um
servidor que já mudou — a mesma armadilha que o catálogo existe para fechar. E
tem um efeito que uma lista local nunca teria: **o `/help` mostra só o que
aquela sessão pode chamar.** Um leitor não vê `excluir_tabela`; o rodapé diz
quantas ficaram de fora, para uma lista curta não parecer um servidor pequeno.

Pedir `/help` de uma operação que existe mas a sessão não pode responde *por
que* — com o nome da permissão que faltou —, em vez de «não existe», que
mandaria procurar erro de digitação onde não há.

## A autenticação é a mesma da réplica

`Console::entrar` chama `Cliente::autenticar` de
`crates/phxsql-server/src/replica.rs`. A senha não viaja: vira a chave derivada,
e o que atravessa é o HMAC do nonce.

Escrever um segundo cliente aqui teria sido escrever um segundo jeito de
autenticar — e o segundo jeito é o que fica para trás quando o primeiro muda.
É a mesma razão de a op `sql` não abrir tabela e de a ponte MCP não chamar
`executar`.

E o console **não ganha poder por ser console**: com cadastro no servidor, uma
operação antes do login recusa, e a tabela negada continua negada. Há teste
pelo soquete para os dois.

## A tabela não mente sobre o dado

Duas regras, e as duas vêm de erros já cometidos neste projeto:

- **A caixa não muda.** «Blumenau» sai «Blumenau». Escrever «BLUMENAU» é uma
  mentira sobre o dado, porque quem olha não sabe qual dos dois está no disco.
- **O corte é marcado.** Uma célula maior que 40 caracteres é cortada com `…`.
  Cortar já muda o valor; cortar sem avisar faz o leitor achar que viu tudo.

E **toda** lista da resposta vira uma tabela, não só a primeira: o `esquema`
traz colunas, índices, chaves estrangeiras e volumes, e mostrar só a primeira
esconderia três sem que ninguém percebesse. As colunas de cada tabela saem da
**união** das chaves de todas as linhas — usar as do primeiro objeto esconderia
um campo que só o segundo tem.

## A armadilha que só o soquete mostrou

O partidor de linha tirava **toda** aspa, para `nome="Ana Maria"` funcionar. Os
testes de unidade dele passavam.

Contra o servidor de verdade, a primeira inserção falhou:

```text
inserir tabela=clientes valores={"id":1,"nome":"Adriano"}
  -> erro: a linha precisa ser um objeto ou uma lista
```

O valor chegava como `{id:1,nome:Adriano}` — sem as aspas não é JSON, então o
console mandava aquilo como **texto**, e o servidor reclamava de um jeito que
não aponta para o console em lugar nenhum. Ler o código não mostrava; o teste
de unidade também não, porque ele testava o caso do nome com espaço.

A regra certa é **por posição**: a aspa delimita quando abre o pedaço (ou vem
logo depois do `=`), e é literal dentro de um `{...}` ou `[...]`, onde o espaço
também deixa de separar. Com o defeito reposto, tanto o teste de unidade quanto
o de soquete falham — e foi assim que ele se provou.

É a lição do `BULKINSERT` outra vez: **o que depende do outro lado se prova
contra o outro lado.**

## Duas escolhas pequenas que vale registrar

**Valor com ponto vira TEXTO, não número.** `total=12.34` chega como `"12.34"`.
`f64` não representa 12.34, e o protocolo trafega decimal como texto justamente
por isso. Quem quiser um ponto flutuante de verdade escreve JSON cru. Errar
para o lado do texto não perde nada; errar para o lado do `f64` perde centavo.

**`--comando` sai com código de erro quando a linha deu erro.** Sem isso, um
script encadeado (`phxsqlcmd --comando '...' && proximo`) seguiria como se
tivesse dado certo.

**A senha vem de `PHXSQL_SENHA`.** `--senha` funciona e avisa que é pior:
argumento aparece no `ps` de qualquer um na máquina e fica no histórico do
shell. É o mesmo cuidado que o `phxsqld --senha` já tinha.
