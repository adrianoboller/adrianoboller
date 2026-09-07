# O que audita não mora no que é auditado

*Descoberto em 07/09/2026, 16:40, ao escolher onde guardar a diretiva por banco
(pedido 220) e o diário administrativo.*

## 1. O que aconteceu

Duas decisões de formato caíram na mesma tarde, e as duas pareciam ter a mesma
resposta óbvia — «guarde perto do que descreve»:

1. **A diretiva por banco.** «Proibir `reindexar` só em `financeiro`» pede um
   ajuste por banco, e o lugar natural parecia um `diretivas.json` dentro do
   diretório do banco. Fica ao lado do que descreve, some junto quando o banco
   é apagado, e não incha o `config.json`.
2. **O diário administrativo.** Nove campos por alteração, e o lugar natural
   parecia uma tabela do `phxsys` — a grade do Centro de Controle já a
   editaria, o backup já a levaria, a permissão por base já a protegeria, o
   `.log` da tabela já diria quem mudou o quê. **Zero mecanismo novo**, que é
   exatamente o argumento que fez a tabela de mensagens ser uma tabela.

As duas respostas óbvias estão erradas, e pelo **mesmo** motivo — que é o que
faz disto um aprendizado e não duas decisões soltas.

## 2. O que eu concluí primeiro, e estava errado

Concluí que o critério era **proximidade**: o dado de configuração mora perto
do que ele configura, e o argumento de reuso («a tabela já tem grade, backup e
auditoria») vencia sozinho, como venceu no `phxsys.mensagens`.

Errado, e o `phxsys.mensagens` é justamente o contraexemplo que me enganou: ele
é **texto de tela**. Se a tabela de mensagens ficar inacessível, o servidor cai
no texto de fábrica e ninguém percebe. Se o diário ficar inacessível, ninguém
percebe **também** — e é aí que a semelhança termina, porque no primeiro caso o
que se perde é uma tradução e no segundo é a prova de quem mexeu.

O critério certo não é proximidade nem reuso. É: **quem depende de quem, na
ordem em que as coisas acontecem.**

## 3. O que a medição disse

Não é número, é a ordem de execução do `despachar`, lida no fonte:

```text
despachar():
  portao 0  politica (comando proibido, base proibida)   <- AQUI
  portao 1  token
  portao 2  login
  portao 3  permissao
  ...       abrir o database
```

A política é conferida **antes do token, antes do login, antes de resolver nome
de base**. Um `diretivas.json` dentro do banco obrigaria a abrir um arquivo em
disco a cada pedido — e, pior, a **ler do diretório de dados a regra que
protege o diretório de dados**: quem pudesse escrever no banco levantaria a
própria restrição. A ordem inverte o argumento da proximidade.

O mesmo vale para o diário, por dois caminhos:

- um database obedece a `bases_proibidas`, à permissão por base e ao
  `somente_leitura` — **todos configuráveis pelo mesmo `ALTER SERVER` que o
  diário existe para registrar**. Trilha que a própria mudança pode calar não é
  trilha;
- um servidor recém-subido tem **zero databases**, e a configuração muda antes
  de haver banco. Gravar a primeira diretiva criaria o `phxsys` como efeito
  colateral de mexer num campo.

O que sobrou: a diretiva por banco entrou na **mesma lista** do `config.json`
(`seguranca.comandos_proibidos`, aceitando objeto ao lado de string — zero
migração, e string solta continua significando «em todo banco»), e o diário
virou `diretivas.log`, JSON Lines ao lado do `acessos.log`. Medido na bancada:
61 afirmações, 0 falhas, com a proibição por banco valendo **a quente** e a
mesma operação passando em outro banco.

## 4. A regra

**Antes de escolher onde um dado mora, pergunte o que ele controla — e não o
que ele descreve.** Guarda, política e trilha moram FORA do que elas guardam,
policiam ou registram; e o que é lido no portão mora onde o portão já olha,
porque um portão que abre arquivo é um portão que pode não abrir.

## 5. Como está guardado hoje

- `docs/DIRETIVAS.md` §3 (onde a diretiva por banco mora, com as duas casas
  recusadas e o motivo) e §5 (arquivo, e não tabela).
- `docs/FORMATO.md` §18 — o formato do `diretivas.log`, com os nove campos e a
  máscara por nome.
- O comentário de cabeçalho de `crates/phxsql-server/src/diretivas.rs` carrega
  os três motivos, para quem for mexer no arquivo não precisar achar o
  documento.
- Testes: `sem_regra_por_banco_nada_muda` (o comportamento velho),
  `o_proibido_do_banco_so_vale_naquele_banco` com controle positivo, e
  `a_diretiva_por_banco_nao_retira_nada`.

**Onde o buraco ficou:** o `diretivas.log` não tem rodízio de tamanho — o
`acessos.log` também não, e os dois crescem para sempre. Num servidor onde
alguém automatize `ALTER SERVER SET` isso vira um arquivo grande e calado. O
`profiler` tem rodízio (`profiler.arquivo_mib`/`profiler.arquivos`), e é dele
que a receita sairia; virou pedido, não código.
