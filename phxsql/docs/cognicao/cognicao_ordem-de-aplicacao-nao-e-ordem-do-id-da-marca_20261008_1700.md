# A ordem de aplicação das marcas não é a ordem do id delas

**Estado:** PENDENTE

## O que aconteceu

O desenho único da recuperação (`docs/propostas/recuperacao-e-replica-desenho-unico.md`)
pôs como invariante I13 «a ordem de completar é a ordem de criação (id numérico), através de
todas as famílias», e listou como furo F5 (pedido 715) o arranque completar todas as
`transacao_*` antes das `bidi_*`. Na frente de 08/10/2026 (papel B), ao ler o
`aplicar_grupo_bidi` para escrever a prova do F5: a marca do bidirecional e a do grupo da
réplica são gravadas **fora** da trava de dados (`marcar_o_grupo_bidi`, catraca
`alcancam-fsync`), e só aplicadas depois de tomá-la; a do `COMMIT` é gravada **com** a trava.

## O que eu concluí primeiro, e estava errado

Que o F5 era um defeito de ordem a consertar no escalonador, como o desenho propõe (E5):
ordenar todas as famílias pelo id numérico.

## O que a medição disse

Duas coisas, uma medida e uma lida:

- **Medida:** depois do conserto do F1/F3 (pedidos 709/711, marca v7 com a versão do slot),
  a marca cuja passada terminou grava **zero eventos** no arranque
  (`tests/recuperacao-nao-volta-o-valor.rs::completar_a_marca_de_passada_terminada_nao_acrescenta_eventos`,
  RED medido antes: +1 evento). Só a marca em voo escreve, e a ordem só importa entre marcas
  que escrevem.
- **Lida:** a única coexistência de duas marcas que escrevem é um `COMMIT` em voo (com a
  trava) e um grupo do bidirecional cuja marca foi gravada fora da trava e espera por ela. Ao
  vivo, o grupo entra **depois** do `COMMIT`. O id do grupo pode ser **menor** (a marca nasceu
  antes de esperar a trava): ordenar pelo id o aplicaria **antes** — o contrário do que teria
  acontecido sem a queda. A ordem de hoje (todas as `transacao_*`, depois as `bidi_*`) é a
  que reproduz a vida.

## A regra

Ordem de recuperação é a ordem em que a **trava** teria aplicado, não a ordem em que o
bilhete nasceu: quando o bilhete nasce fora da trava, o id dele não diz quando ele entra.

## O alcance, que eu li menor do que era (revisão do papel C, mesmo dia)

Escrevi a regra só para o par `transacao_*` × `bidi_*`, e o mesmo engano morava **dentro**
de `transacao_*`: a marca do grupo da réplica fiel usa o mesmo prefixo e o mesmo contador, e
também nasce fora da trava (`marcar_o_grupo`); o id do `COMMIT` sai no **BEGIN**. Um `BEGIN`
depois da marca do grupo tem id maior e entra antes dele. Medido
(`crates/phxsql-server/tests/ordem-da-recuperacao.rs::o_commit_que_entrou_antes_do_grupo_e_completado_antes_dele`):
completando pelo id, a linha 2 de `t` saiu a da ORIGEM («o2») e o `COMMIT` local perdeu a
inclusão — dado como completado. O controle com o `BEGIN` antes da marca (id menor) já saía
certo. A conclusão «o F5 morreu» estava errada: morreu o F5 escrito, e vivia este.

## Como está guardado hoje

`marca::marcas_com_prefixo` ordena por `(nasce_fora_da_trava, id)`, e o primeiro termo sai da
**versão do cabeçalho** (5/6 = fora da trava), nunca do nome — as duas famílias dividem o
prefixo. O `completar_marcas_do_bidi` continua depois de todas. A E5 do desenho («por id através
das famílias») morreu com esse RED como prova.
