# O Profiler gravava em texto o que a cifra escondia no disco

**Descoberto em 05/09/2026, por volta das 14h50**, medindo o terreno da frente
CIFRA-POR-TABELA antes de escrever qualquer linha.

## 1. O que aconteceu

`profiler.rs` guardava, em cada `Evento`, o campo `pedido = redigir(linha_crua)`
— o pedido **inteiro**, redigido **só por nome de campo**. A lista `SEGREDOS`
tem `senha`, `senha_b64`, `nova_senha`, `prova`, `token`, `chave`,
`chave_privada` e `assinatura`. Não tem `linha` — **e não podia ter**, porque
`linha` é justamente o dado que o usuário está gravando.

E o Profiler **grava arquivo de texto**: `perfil.txt`, com rodízio `.1`, `.2`…

Então o payload de uma tabela cifrada ficava **em claro num arquivo em disco, ao
lado do `.reg` cifrado** — anulando, palavra por palavra, o propósito escrito no
`store/src/cofre.rs`:

> *«Protege o ARQUIVO COPIADO — disco levado, backup vazado, cópia numa máquina
> que não é esta.»*

**Quem leva o disco leva o `perfil.txt`.** Um `inserir` com CPF numa tabela
cifrada deixava o CPF em claro no diretório do servidor, e nada acusava.

## 2. O que eu concluí primeiro, e estava errado

**Concluí que a saída era não capturar nada.** O briefing dizia *«o Profiler
guarda `op`, `tabela`, duração e o tamanho em bytes — nunca o texto»*, e eu
aceitei isso como a decisão inteira e comecei a escrever o portão para cegar o
instrumento nos dois lugares.

**Estava errado, e o dono corrigiu com uma frase que já estava no fonte:** *«se
for administrador, a regra usa a senha do `config.json`»*. O Profiler tem
**dois** lugares — o anel em memória e o arquivo — e eu os tratei como um só. O
portão `portao_do_profiler` já garante que quem vê a tela é **administrador
deste servidor**; e administrador tem o `config.json`, logo **tem a senha do
cofre**. Esconder o texto dele é teatro: ele abre o arquivo e lê a senha.

O que não é teatro é manter o texto **fora do arquivo**, e a razão é a diferença
que eu não tinha visto: **o arquivo não tem quem olha.** A tela tem login,
sessão e portão; o `perfil.txt` viaja com o disco, entra no backup, e ali não há
nada disso.

Errei um segundo diagnóstico no mesmo dia, menor e da mesma família: **decidi
olhar o campo `"tabela"` do pedido** para saber se ele toca tabela declarada. O
`CLAUDE.md` já tinha a resposta escrita e eu não a apliquei de primeira —
**três operações escondem tabela desse campo**: `juntar` guarda em
`a.tabela`/`b.tabela`, `unir` numa **lista**, e `pivotar` põe a de fatos no
campo que se lê e as de consulta dentro de um `juntar` aninhado. O primeiro
nível teria deixado passar em claro exatamente o pedido que lê a tabela cifrada
como lado B de uma junção.

## 3. O que a medição disse

| medida | número |
|---|---|
| campos na lista `SEGREDOS` | 8, e nenhum cobre o dado da linha |
| o que o arquivo gravava | o pedido inteiro, ~345 B por pedido (número já medido no próprio módulo) |
| operações que nomeiam tabela onde o primeiro nível não olha | **3** (`juntar`, `unir`, `pivotar`) |
| testes que caem com o defeito reposto (o arquivo volta a gravar o texto) | **4 de 4** |
| testes que caem tirando só a descida na árvore | **1 de 1** — e é o ponto: nenhum outro acusa |
| casos da bateria de frontend depois da mudança | **43/43** |

E o que a mesma medição **negou**, porque valia conferir: o campo `erro` da
linha do arquivo **não** carrega valor de linha. As mensagens citam **nome**:
`"indice unico {nome} ja tem essa chave"`, `"{fk}: nao existe {tabela}({colunas})
com esse valor"`. Não havia nada a consertar ali, e o achado fica escrito para
ninguém supor o contrário.

## 4. A regra

**Instrumento que grava ARQUIVO e instrumento que pinta TELA não têm a mesma
regra de sigilo — a tela tem portão, o arquivo não.** Antes de cegar um
observador, pergunte de qual das duas saídas ele é: esconder da tela do
administrador é teatro; deixar no arquivo é o furo.

E o corolário, que é a lei do irmão aplicada a dado em vez de código: **quando
um portão passa a olhar um campo, procure quem não tem esse campo** — inclusive
quando o portão é sobre *dado* e não sobre *permissão*.

## 5. Como está guardado hoje

- **O conserto**: `crates/phxsql-server/src/profiler.rs` — `Evento.sigiloso`
  decidido em `chegou`, no **mesmo** percurso que já redigia o pedido (uma
  análise de JSON, não duas), e `Evento::linha()` grava `SEM_TEXTO` no lugar do
  pedido. O anel continua com o texto.
- **A colheita**: `colher_tabelas`, que desce a árvore inteira e leva o
  `database` corrente consigo.
- **Nove testes**, sete deles em `profiler::testes_tabela_sigilosa`. O que
  carrega o peso é `o_anel_ve_o_texto_e_o_arquivo_nao`: as **duas** saídas na
  **mesma** corrida, com o anel como **controle** — sem ele, um Profiler cego
  por acidente passaria igual.
- **Duas guardas** em `bancada/guardas/catalogo.py`:
  `perfil-grava-o-texto-da-tabela-declarada` (4/4) e
  `perfil-so-olha-a-tabela-do-primeiro-nivel` (1/1), as duas **provadas**.
- **O buraco que fica, e ele é do formato:** marcar uma tabela **não reescreve**
  `perfil.txt.1`, `.2`… já gravados. Está como **teste**
  (`declarar_depois_nao_limpa_o_arquivo_ja_gravado`) e não como comentário,
  porque a surpresa é que custa caro — quem marca depois de ter perfilado acha
  que limpou o passado. A tela diz isso, nos seis idiomas.
- **O que NÃO foi entregue**: `Criptografar`/`Descriptografar`. Enquanto ela não
  existe, declarar **não cifra** — e é isso, literalmente, que o rodapé da tela
  diz. `docs/SEGURANCA.md` §13.
