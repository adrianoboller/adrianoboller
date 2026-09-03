# A decisão envelheceu o manual — e o teste que a contradizia continuou verde

*Descoberto às 16:55 de 03/09/2026, levantando o pedido 175: procurava quantas
tabelas desta base declaram chave sem o índice, e achei a receita de todas.*

## 1. O que aconteceu

«Chave declarada **nasce conferida**» é decisão do dono, tomada nesta rodada, e
está no `CLAUDE.md`, no `valores.rs:247` e no `ForeignKey::new`. Ela mudou o
significado de tudo o que já ensinava a declarar uma chave — e três lugares que
ensinam continuaram dizendo o de antes:

* **`MANUAL.txt`**, seção *CHAVE ESTRANGEIRA NO criar_tabela*: o exemplo
  canônico declara `pedidos` com **um** índice, pela primária, e uma chave
  estrangeira em `cliente_id`. Sem o índice em `cliente_id`, a chave — hoje
  conferida por omissão — deixa `clientes` **sem excluir nada**, nem as linhas
  que ninguém referencia. O manual ensina o defeito.
* **`op_declarar_fk`** responde `"imposta": false` (`servidor.rs:9394`) e o
  doc-comment ainda diz *«declarar não é impor: [...] nenhuma gravação a
  confere»*. O campo existe para a tela dizer a verdade, e diz a mentira.
* **O teste** (`servidor.rs:20239`) trava a mentira, com o comentário *«a
  resposta diz a verdade que a tela precisa repetir»* — e monta uma `pedidos`
  com um índice só, exatamente como o manual. **Ele passa**, e passa porque
  nunca tenta excluir um cliente.

## 2. O que eu concluí primeiro, e estava errado

Li a §6 do `INTEGRIDADE.md` — *«dá para declarar chave conferida sem os índices
e só descobrir no primeiro `excluir`»* — e escrevi no rascunho que o estrago
era «o excluir de uma linha com filha falha». Parecia óbvio: sem índice na
filha, o motor não consegue procurar quem aponta para a mãe; logo, quem tem
filha não sai.

Rodado, o estrago é maior e de outra natureza: **o cliente que não tem filha
nenhuma também não sai.** O motor não pergunta «esta linha tem filha?»; ele
pergunta antes **«eu consigo perguntar?»**, e para na resposta. Uma tabela
perde a exclusão inteira porque falta um índice em **outra** tabela.

O diagnóstico plausível descrevia o caminho certo e o alcance errado — e a
frase da §6, que é curta e verdadeira, é justamente o que impediu qualquer um
de rodar o caso.

## 3. O que a medição disse

Prova real, nos dois sentidos, contra o motor:

```
--- SEM o índice (o que o MANUAL ensina) ---
  inserir pedido do cliente 3     : ACEITOU
  excluir o cliente 3 (TEM filha) : recusou — «não tem índice começando por (cliente_id)»
  excluir o cliente 7 (SEM filha) : RECUSOU — a MESMA mensagem

--- COM o índice em cliente_id ---
  inserir pedido do cliente 3     : ACEITOU
  excluir o cliente 3 (TEM filha) : recusou — «esta linha tem filhas [...] nunca
                                    se apaga o registro pai que tem filhos»
  excluir o cliente 7 (SEM filha) : aceitou (certo)
```

E o custo de consertar o exemplo, medido, para que a discussão não fique no
achismo: na tabela **vazia** — que é quando se modela — criar o índice junto
com a chave custa **70,4 µs**. Setenta microssegundos separam o exemplo do
manual de um exemplo que funciona.

## 4. A regra

**Decisão que muda o padrão de uma declaração envelhece todo exemplo que
declara — e o teste que travava o comportamento antigo passa a travar a
mentira, verde.** Ao mudar um padrão, procure quem *ensina* o antigo: manual,
resposta de protocolo, doc-comment e a asserção que os defende.

É o corolário de comportamento de «número digitado à mão envelhece calado». O
número tem gerador que o denuncia; **o exemplo não tem**, e um teste verde é
exatamente o que faz ninguém olhar de novo.

## 5. Como está guardado hoje

**Não está consertado** — esta frente era o parecer do pedido 175, e o pedido
não autoriza mexer no manual, no servidor nem no teste. Onde está registrado:

* `docs/PARECER-175-INDICE-NA-DECLARACAO.md` §1.2 (o exemplo do manual, com a
  prova real nos dois sentidos), §4 (o custo de cada saída) e §6, parte final
  (o `"imposta": false` e o teste que o trava).

O buraco fica nomeado em três pedaços, e nenhum deles é a mesma correção:

1. o exemplo do `MANUAL.txt` precisa do índice em `cliente_id` — **um índice a
   mais numa linha de JSON**, e é o conserto mais barato deste parecer inteiro;
2. `"imposta"` precisa dizer o que o esquema diz, e o doc-comment junto;
3. o teste precisa trocar de asserção, e a prova real dele é o §1.2: com a
   `pedidos` que ele monta, o `excluir` de um cliente **sem filha** tem de
   falhar hoje e passar depois.
