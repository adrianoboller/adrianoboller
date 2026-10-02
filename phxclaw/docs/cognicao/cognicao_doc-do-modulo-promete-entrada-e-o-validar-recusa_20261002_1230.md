# O doc do módulo promete `{{entrada}}` e o `validar` recusa: comentário de campo também envelhece

**Estado:** PENDENTE

**Evidência pedida:** um teste em `tests/fluxo_motor.rs` com um passo `por_item` usando
`{{entrada}}` que falhe na leitura com «usa {{entrada}} sem declarar 'entrada' em depende», e
o comentário da linha 130 corrigido no mesmo commit. Nada compilou nesta rodada (cargo
proibido por disco); o achado é por leitura de `git show HEAD:…/fluxos.rs`.

## O que aconteceu

Ao escrever o formato do fluxo no `docs/N8N.md` §8 a partir do fonte, o comentário do campo
`por_item` (`fluxos.rs:130`) dizia «`{{entrada}}` é o item da vez». O código
(`fluxos.rs:1121-1133`) não cria chave `entrada` na visão: a cada passada ele substitui a
saída da dependência de entrada por um item só, e o item se lê pelo **id da entrada**
(`{{lista.n}}`, como o teste `execucao_por_item` faz). O `validar` (`:404-420`) recusa
`{{x}}` de id fora de `depende`, logo `{{entrada}}` nem chega a rodar.

## O que eu concluí primeiro, e estava errado

Que o doc do módulo era a fonte da verdade para a seção do formato e bastava transcrevê-lo.
Era a mesma armadilha do número digitado: o comentário foi escrito com um desenho e o código
saiu com outro, e os dois convivem porque nenhum teste lê comentário.

## O que a leitura disse

O teste `execucao_por_item` (`tests/fluxo_motor.rs:229`) é o único lugar onde a forma certa
está escrita — `{{lista.n}}` — e nenhum dos 10 testes usa `{{entrada}}`. A seção §8a do
`N8N.md` documenta o que o código faz e nomeia a divergência; o fonte não foi tocado porque
outra frente o edita nesta rodada.

## A regra

Seção de formato se confere contra `ler`/`validar` e contra o teste que exercita o campo, não
contra o comentário do campo. Quando os dois divergem, o documento publica o comportamento
do código e aponta o comentário como defeito — nunca o contrário.
