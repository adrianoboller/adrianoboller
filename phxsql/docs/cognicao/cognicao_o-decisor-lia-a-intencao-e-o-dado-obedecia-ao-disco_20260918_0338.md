# O decisor lia a intenção e o dado obedecia ao disco

**Descoberto em 18/09/2026, 03:38** — a hora do vermelho medido, não a do
conserto. Frente F356, pedido 356.

## 1. O que aconteceu

O Profiler escondia o texto do pedido do `perfil.txt` quando a tabela estava
nomeada em `config.cifra.tabelas` (`profiler.rs`, `tabela_e_sigilosa`). Quem de
fato cifra o dado é a **marca de coluna**, lida na criação da tabela
(`store/src/reg.rs`, `esquema.tem_dado_pessoal()`), e gravada no cabeçalho do
`.reg`.

Dois campos, uma garantia. E como `cifra.tabelas` **nasce vazia** em todo
`config.json`, o caso comum era o furo: cofre ligado, coluna marcada, ninguém
declarou nada — e o valor marcado ia em claro para o arquivo que, segundo o
cabeçalho do próprio `profiler.rs`, «viaja com o disco e entra no backup».

A ironia está documentada: a §13.3 do `docs/SEGURANCA.md`, escrita em
05/09/2026, já dizia **«o disco manda, a lista pede»** — e o Profiler, escrito
na mesma rodada, decidia pela lista. **Lei escrita num documento não alcança o
código que não a chama.**

## 2. O que eu concluí primeiro, e estava errado

**Duas coisas, e as duas teriam compilado, passado nos testes e ficado erradas.**

**(a) «O conserto é uma segunda lista, colhida no `ligar`.»** Simétrica à
primeira, alimentada nos mesmos dois lugares (`profiler_ligar` e o gravar
config), varrendo os esquemas uma vez. Barata e limpa.

Ela envelhece **dentro da sessão**, e envelhece calada no caso mais provável de
todos: perfilar uma carga que **cria** a tabela e insere nela em seguida. No
`ligar` a tabela não existe — entra na lista como «não cifrada» — e o `inserir`
seguinte grava em claro exatamente o que o conserto existe para tirar do
arquivo. Nenhum teste da própria frente pegaria isso: eles criam a tabela antes
de ligar o Profiler.

**(b) «O furo é a coluna do pedido.»** Tapei o `pedido`, os dois testes ficaram
verdes, e a linha continuava vazando **pela coluna ao lado**. O `erro` da mesma
linha do mesmo arquivo não tinha portão nenhum. Pior: a §13.9 do
`SEGURANCA.md` afirmava, com veredito, que o campo `erro` **não** carrega valor
de linha — e a afirmação tinha olhado **duas** famílias de mensagem (índice
único e chave estrangeira, que citam nome) e concluído sobre **todas**.

## 3. O que a medição disse

O vermelho do pedido, com a premissa medida na mesma corrida
(`{"op":"esquema"}` respondeu `"material":"cifrado"`):

```text
2026-09-18 03:38:49,239 127.0.0.1  -  inserir  loja.clientes  ok 5ms  123B
  {"token":"***","op":"inserir","database":"loja","tabela":"clientes",
   "linha":{"id":1,"cpf":"111.222.333-44"}}
```

O vermelho do irmão, **com o pedido já tapado pelo conserto**:

```text
inserir  loja.clientes  ERRO 0ms  115B  <tabela com .reg cifrado: pedido nao
gravado>  <- [SP000018] limite excedido: 123456 nao cabe em inteiro de 16 bits
```

O `123456` era o valor de uma coluna marcada (`Int2`, `dado_pessoal:
sensivel`). A terceira família de mensagem — a validação de **faixa** — cita o
valor que chegou.

E as contagens da busca do irmão:

| pergunta | número |
|---|---|
| chamadores de `tabela_e_sigilosa` no código | **1** |
| leituras de `config.cifra.tabelas` no código | **4** — 2 alimentam o Profiler, 2 declaram/validam o campo. **Nenhum outro decisor** |
| pontos de captura do Profiler | **2** (soquete e porta web), e os dois chamam o **mesmo** `chegou` |
| outros arquivos que gravam texto de pedido | **0** — reconferi o `Acesso` do `acessos.log` campo a campo (nenhum livre do corpo); o `jobs.rs` e a marca do `transacao.rs` seguem como a §13.9 os deixou |
| furo que ficou, medido | **1** — `{"op":"sql"}` nomeia a tabela dentro da frase, e nem a lista nem a marca o alcançam. Medido nos dois casos, e é furo de 05/09 |

## 4. A regra

**Quando o portão passar a olhar um campo novo, procure quem não tem esse campo
— inclusive a coluna ao lado, na mesma linha do mesmo arquivo.** E:
**conferência que varre duas famílias e conclui sobre todas é amostra, não
veredito** — amostra escrita como veredito é o que impede a próxima pessoa de
olhar.

E a regra de desenho que a (a) deixou: **decisão de segurança que depende de uma
lista colhida antes não é decisão, é retrato** — pergunte ao estado no momento
de decidir, ou prove que o estado não muda nesse intervalo.

## 5. Como está guardado hoje

- `Profiler::sigilo_dos_alvos` pergunta à **lista** (memória, custo zero) e
  depois ao **disco** (`catalogo::reg_cifrado`: dez bytes do cabeçalho do
  primeiro volume). O portão vem antes: sem `arquivo` pedido, o disco não é
  consultado.
- As **quatro** respostas do disco (`RegNoDisco`) decidem diferente, e as duas
  ausências são separadas de propósito: `SemVolume` deixa o texto passar (senão
  todo `criar_tabela` e toda visão sairiam cegos), `Ilegivel` não arrisca.
- O `erro` da linha vira o **tamanho** quando o evento é sigiloso — analisando,
  não recortando. O anel continua com o texto inteiro, como o pedido.
- Nove testes, com a prova real nos dois sentidos, listados na §13.13 do
  `docs/SEGURANCA.md`. O do comportamento **velho** tem irmão novo
  (`reg_em_claro_continua_com_o_texto`): sem ele, um conserto que cegasse toda
  tabela passaria no antigo.
- **Onde o buraco ficou:** o caminho `{"op":"sql"}`, medido e nomeado na
  §13.13, **sem pedido registrado** — o número novo nasce por quem integra a
  rodada. E o par «catálogo de guardas / catraca» desta frente não foi mexido:
  as guardas do pedido 195 estão em `bancada/guardas/catalogo.py` e as novas
  ainda não entraram lá.
