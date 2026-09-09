# O store não sabe qual arquivo faltou

## 1. O que aconteceu

Achado A13 (`p12_existe_sql_e_mensagens.py`): `SELECT * FROM x` com nome
errado — e o `varrer` pelo protocolo — respondia «nenhum volume de x.reg em
/tmp/…/dados/base/p12», o caminho absoluto do disco do servidor, a todo
cliente que errasse uma letra. O erro cru era do `reg.rs`, propagado por
`abrir_qualificada`. A mesma correção já tinha sido paga em `table.rs` para a
chave conferida contra mãe inexistente.

## 2. O que eu concluí primeiro, e estava errado

Que bastava traduzir todo `NaoEncontrado` de `Table::abrir` para «a tabela x
não existe» — em `abrir_com`, casando o texto «nenhum volume de». Dois erros
numa frase: casar texto de erro é o que a lei «texto se resolve por chave,
nunca por comparação da frase» proíbe; e o store **não sabe qual dos dez
arquivos faltou** — um `NaoEncontrado` ao abrir tanto é a tabela ausente
quanto uma tabela que perdeu um arquivo, e a segunda **não pode** virar «não
existe», porque aí o operador cria outra por cima da quebrada.

A segunda ideia — conferir `existe_tabela` antes de abrir — paga um
`read_dir` no laço quente, que abre a tabela a cada pedido.

## 3. O que a medição disse

A existência, no store, é o `.reg` (`nome_da_tabela` só reconhece essa
extensão): uma tabela sem `.ndx` **existe** para o catálogo, e o erro que sai
para ela continua sendo o do componente. O teste de unidade cobre as três
frases (tabela, `schema.tabela`, database) e a tabela quebrada. Custo no
caminho feliz: zero — a lista do diretório só é lida depois de a abertura
falhar.

## 4. A regra

**Traduza o erro no ponto que sabe o nome, e só depois de confirmar o que o
erro cru não sabe dizer** — no caminho do erro, nunca no laço quente.

## 5. Como está guardado hoje

`Database::tabela_que_nao_existe` em `catalogo.rs`, chamada pelas duas fichas
(`abrir_tabela` e `Raiz::abrir_para_ler`); `abrir_database` sem o caminho da
base, o irmão; guarda `tabela-inexistente-vaza-o-caminho`; nota em
`docs/SQL.md` §5.
