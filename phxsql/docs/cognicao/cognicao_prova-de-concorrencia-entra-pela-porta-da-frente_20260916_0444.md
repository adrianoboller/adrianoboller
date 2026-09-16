# Cognição: prova de concorrência entra pela porta da frente — dois portões irmãos moram em camadas diferentes, e o teste que chama a de dentro pula o de fora

- **Assunto:** leitura repetível pela trava, pedida (`docs/SOMBRA.md` §5b) —
  a prova real do escritor barrado
- **Descoberta:** 16/09/2026, 04:44 UTC
- **Arquivos:** `crates/phxsql-server/src/servidor.rs` (`executar`,
  `dentro_da_transacao`, `travar_leitura_repetivel`, `despachar`,
  `portoes_do_pedido`, `barrado_por_travas`, `mod testes_leitura_repetivel`)

## 1. O que aconteceu

A funcionalidade estava inteira: a S (`Trava::Compartilhada`) no `travas.rs`,
o campo `leitura_repetivel` na abertura, o gancho no portão das transações.
Escrevi três provas com duas conexões (`ligacao` 1 e 2) chamando `s.executar`
direto, como os ajudantes de outros módulos de teste fazem. O controle passou;
as duas provas da funcionalidade **falharam no mesmo ponto**: a escrita
autocommit da conexão `b` **passava** enquanto `a` segurava a S.

## 2. O que eu concluí primeiro, e estava errado

Primeira hipótese, antes da compactação: «o portão `dentro_da_transacao` mora
no `despachar`, e não no `executar` — o teste nunca chega nele, então a S
**nunca é tomada**». Estava errada na metade que importava: o portão das
transações **está** no `executar` (linha do «O PORTAO DAS TRANSACOES, e ele vem
antes do despacho inteiro»), e a S **era tomada**. A leitura de `a` fazia tudo
certo.

O que o teste pulava era o **outro** portão: o da escrita comum. Quem barra o
autocommit contra uma trava alheia é `barrado_por_travas`, chamado por
`portoes_do_pedido`, chamado por `despachar` — a camada de FORA do `executar`.
O teste entrava pela porta de dentro e por isso a escrita de `b` nunca foi
perguntada se podia.

São dois portões **irmãos** — os dois leem a mesma tabela de travas para
decidir a mesma coisa — e moram em camadas diferentes por um motivo escrito no
código: a transação tem direito de **esperar** o `LOCK TIMEOUT` que declarou
(`esperar_trava`, dentro do `executar`), e o pedido solto não declarou nada e
recusa **na hora** (`barrado_por_travas`, antes do `executar`). O desenho está
certo; a prova é que entrou pela porta errada.

## 3. O que a medição disse

- Com as provas reescritas pelo `despachar` (`manda(s, &mut sessao, corpo)`,
  o mesmo molde do `na_tx` dos testes de transação): **4 de 4** verdes na
  primeira corrida.
- Prova real no sentido contrário, com o gancho `travar_leitura_repetivel`
  removido do portão: **3 vermelhos, 1 verde** — o verde é o controle
  `sem_pedir_a_leitura_continua_nao_repetivel`, que TEM de continuar passando
  porque «guarda nova entra pedida, não imposta».
- Quinta prova acrescentada depois: duas transações que pediram e leram a mesma
  tabela recebem `LOCK TIMEOUT` ao tentar escrever o que a outra leu — nos dois
  sentidos, porque não há detector de impasse e o prazo é quem resolve.

## 4. A regra que fica

**Prova de concorrência entra pela porta da frente.** Um teste de trava tem de
percorrer o caminho que um cliente percorre — aqui, `despachar` —, porque o
portão que decide «pode escrever?» pode estar numa camada acima daquela que o
ajudante de teste chama. `executar` direto serve para provar **o que uma
operação faz**; não serve para provar **o que impede a outra conexão de
fazê-la**.

E o corolário para quem for procurar um irmão: **irmão é quem lê a mesma
estrutura para decidir a mesma coisa**, não quem tem o nome parecido. Aqui os
dois se chamam de jeitos diferentes (`esperar_trava` × `barrado_por_travas`) e
nem estão na mesma função — e são o mesmo portão, dividido pela única
diferença que importa entre os dois clientes: um pode esperar, o outro não.

## 5. O que NÃO virou regra nova

«Conserto entra no caminho que o motivou, e o caminho irmão fica» já é pétrea
do `CLAUDE.md`. Este arquivo registra o **alcance** dela que faltava: a pétrea
falava do conserto; vale igual para a **prova** — a prova que entra por um
irmão só prova aquele irmão.
