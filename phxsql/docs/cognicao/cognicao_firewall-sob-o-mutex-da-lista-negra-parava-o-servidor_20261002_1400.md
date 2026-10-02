# O comando de firewall rodava sob o mutex que toda conexão pega — e o «primeiro exec do produto» já existia

**Estado:** PENDENTE

## 1. O que aconteceu

Na frente do gancho do 249 (o programa do operador que recebe o aviso de disco)
a revisão SEC achou que `blacklist.rs:369` já executava um `argv` do config
com `Command::output()`: sem prazo, com o ambiente herdado e o `stderr` do
filho dentro do erro. Pior: `Firewall::bloquear` rodava dentro do
`self.lista_negra.lock()`, e `barrado()` pega o mesmo mutex em **toda**
conexão. Três tokens errados — sem credencial — bastavam para um comando
pendurado parar o servidor (pedido 638, ALTA). O parecer do papel J tinha
escrito «primeiro `exec` do produto»; era o segundo, e o mais velho era o pior.

## 2. O que eu concluí primeiro, e estava errado

Que bastava pôr o prazo no `output()`. Prazo sozinho deixaria o servidor
parado até `timeout_s` a cada bloqueio: o defeito era o **lugar** (sob o
mutex), não só a falta de prazo. E que o `limpar_vencidos` (que também chama o
firewall) fosse um caminho raro: ele roda em `barrado()`, a cada conexão — o
irmão do `bloquear`, com o mesmo erro.

## 3. O que a medição disse

Teste pelo soquete (`tests/firewall-que-pendura.rs`, firewall
`["/bin/sleep","60"]`, 3 tentativas, 3 tokens errados, depois `ping` de outro
cliente): com o defeito, **2,01 s sem resposta** (o prazo do teste); com o
conserto, responde na hora e o terceiro cliente recebe a resposta dele logo
depois de `timeout_s`. As guardas `firewall-sob-o-mutex-da-lista-negra` e
`firewall-output-sem-prazo-e-com-stderr` repõem os dois defeitos.

## 4. A regra

Quem executa programa de fora usa **o** motor (`gancho::rodar`) e nunca o faz
segurando um mutex que outra conexão precisa: a estrutura de estado devolve o
que fazer, e o chamador executa depois de soltar a trava (a função que aplica
pede o `&Mutex`, não um guarda).

## 5. Como está guardado hoje

Teste de soquete + duas guardas no catálogo + `a_lista_fica_livre_enquanto_o_
firewall_roda`. Buracos ditos: o `kill` alcança só o filho direto; a conexão
que encontra bloqueios vencidos espera `timeout_s` por eles; o 642
(descritores do filho) não tem guarda de defeito reposto porque a `std` abre
tudo com `CLOEXEC` — a prova ali é o controle do detector.
