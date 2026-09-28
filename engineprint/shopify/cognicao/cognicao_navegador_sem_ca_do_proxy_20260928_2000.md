# "Nunca resolvido" era um repositorio de certificados vazio

**Descoberto em** 28/09/2026, 20:00. **Onde:** prova do Judge.me no preview
do tema 17, pelo Chromium do Playwright, atras do proxy do ambiente.

## O que aconteceu

Toda medicao em navegador desta frente vinha sendo feita sobre HTML salvo em
disco, porque o Chromium recusava a loja com `ERR_CERT_AUTHORITY_INVALID`. A
anotacao da sessao anterior dizia: *"timeouts through the agent proxy. Never
solved; all browser measurement was done..."* — uma limitacao registrada que
ninguem remediu. Para provar o Judge.me era preciso a loja de verdade: o
widget e um modulo que o app carrega da CDN dele, nao existe em HTML salvo.

## O que eu conclui primeiro, e estava errado

1. **Que o CA do proxy tinha sido regerado e o navegador guardava o antigo.**
   O pacote `/root/.ccr/ca-bundle.crt` era de 28/09 19:20 e o
   `~/.pki/nssdb` de 16/09 — a historia fechava. Medido com `certutil -L`: o
   repositorio estava **vazio**, zero certificados. O README do proxy diz que
   o "browser NSS store" vem configurado; neste conteiner, nao veio.
2. **Que desviar toda requisicao pelo Node resolvia sem mexer em nada.**
   `ctx.route('**/*')` + `route.fetch()` buscou 142 requisicoes com o CA atual
   e zero falhas — e a pagina passou a **fechar sozinha** no meio da
   navegacao, sem crash e sem desconexao do navegador, e sem ninguem chamar
   `window.close()` (substitui a funcao e ela nunca foi chamada). Abandonado
   sem causa achada; a correcao de raiz tornou o desvio desnecessario.
3. **Que o `openssl s_client -proxy` mostrava o que o navegador via.** Ele
   recebeu a cadeia real da Let's Encrypt (YE1 → Root YE → ISRG Root X2) e me
   fez suspeitar da raiz nova da LE. O Chromium falhava tambem em google.com,
   cdn.shopify.com e letsencrypt.org (Google Trust Services e LE): o proxy
   intercepta o navegador e nao intercepta aquele `openssl`.
4. **Que simular dado buscando o HTML pelo Node era neutro.** Na passada
   simulada eu trocava o HTML do produto buscando-o com `route.fetch()`: um
   segundo cliente na mesma sessao. O Shopify respondeu **503** ao pedido de
   secao do cabecalho em 2 de 3 corridas a 1280 px — contra 0 em 6 cargas
   normais em cada tema. O "erro novo" era do metodo, nao do tema. O selo
   passou a ser preenchido no proprio navegador (MutationObserver no
   `addInitScript`) e deu 3 de 3 limpas.

## O que a medicao disse

- Antes: 4 de 4 hosts com `ERR_CERT_AUTHORITY_INVALID` (headless shell,
  Chromium completo e o executavel de `/opt/pw-browsers`, todos iguais).
- `certutil -L -d sql:/root/.pki/nssdb`: vazio.
- Importados os 6 CAs com `O = Anthropic` do pacote oficial, confianca `C,,`:
  4 de 4 hosts com 200.
- Erros de console: a comparacao pagina a pagina com o tema publicado
  reprovou o candidato por erro que a base tambem da (web-pixels: 7 na base
  numa corrida, 5 na seguinte). Base virou a **uniao** de todas as paginas e
  larguras. A telemetria do Shopify (`shopifysvc.com`) falhou numa corrida
  longa e deu 0 em 4 cargas em cada tema depois — fica fora da conta, com o
  numero escrito no script.

## A regra

Limitacao que bloqueia medicao se remede na rodada em que ela custa algo — e
se remede no ponto de confianca (o que o navegador confia), nao por desvio: o
desvio muda o cliente, e cliente diferente e outra medicao.

## Como esta guardado hoje

- `mock/confia-ca-do-proxy.sh` importa os CAs do pacote oficial no nssdb
  (idempotente; testado numa casa vazia e na real).
- `mock/prova-judgeme.mjs` para com a receita quando ve
  `ERR_CERT_AUTHORITY_INVALID`, em vez de cair num erro cru.
- **Buraco:** o conteiner e efemero. Conteiner novo volta com o nssdb vazio
  e sem `certutil`; o script precisa rodar antes de qualquer prova em
  navegador. Nao ha gancho de inicio de sessao fazendo isso.
