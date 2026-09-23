# Cognição: a decisão do dono enterrada no corpo do pedido some da vista

Descoberta em 23/09/2026, 03:30 UTC, quando o papel J devolveu a triagem das
onze decisões «paradas no dono».

## 1. O que aconteceu

O dono perguntou, às 03:0x: *«Essa versão do motor não sai do 18. Tem muitos
gaps a serem feitos. Algo que eu possa te ajudar?»*

Levantei o `docs/PENDENCIAS.md` e respondi que **15 dos 87 planejados paravam
nele**. Perguntei quatro, ele respondeu quatro; perguntei mais quatro, ele
respondeu mais quatro. Só então o papel J foi conferir os **onze** restantes
contra o help e o fonte dos quatro motores — e o que ele achou não foi sobre
os motores:

| pedido | estado real |
|---|---|
| **274** | **já entregue.** O pedido **378** o fechou em 22/09 e diz isso no próprio corpo. Conferido no fonte: `dblink/mod.rs:183` tem `cifra`, `:493` faz `unwrap_or(CIFRA_DE_SAIDA_PADRAO)`, e a frase «a `std` não traz TLS» **não existe mais** em `dblink/phx.rs` nem no `DBLINK.md` |
| **293** | traz **«DECIDIDO PELO DONO, 17/09/2026 07:10 UTC — recusar no motor agora»** por extenso |
| **294** | traz **«DECIDIDO PELO DONO, 17/09/2026 07:10 UTC — manter a soma, medir por tabela AO LADO»** |
| **289** | **«DECIDIDO PELO DONO, 17/09 07:10 — nanos com avanço forçado»** |
| **290** | **«DECIDIDO PELO DONO, 17/09 07:10 — passo no esquema, início na identidade do nó»** |
| **355** | **«DECISÃO DO DONO, 18/09/2026 — recusar na DECLARAÇÃO»** |
| **340** | **«entra por aceite automático»** — a convergência dos três já o resolvia |
| **393** | forma fechada pelo papel C, nada a decidir |

**Oito dos onze estavam decididos ou entregues.** A fila real do dono era
**três**, não quinze.

## 2. O que eu concluí primeiro, e estava errado

Concluí que **o gargalo era a decisão do dono**. Escrevi para ele, com essas
palavras: «**15 param em você** — não há engenharia esperando, espera a
palavra». E montei a resposta inteira em cima disso: duas rodadas de pergunta,
oito decisões pedidas.

Estava errado por um motivo que não é de opinião, é de leitura: **eu classifiquei
pelo ESTADO da linha (`☐ Planejado`) e pelo TÍTULO, sem ler o corpo.** A decisão
do dono estava no corpo — em pedidos de 2.000 a 6.000 caracteres, no fim de
parágrafos de parecer do DBA, sem marca nenhuma que a fizesse aparecer na
listagem.

E o agravante é o **340**, porque ali eu **li** a frase certa e perguntei assim
mesmo: escrevi ao dono «a lei da casa parece já responder» e mandei a pergunta
junto. Não foi falta de informação. Foi não confiar na leitura que eu mesmo
tinha feito.

## 3. O que a medição disse

- **11 decisões** na fila aparente → **3** de verdade (325, 333, 368; mais o
  337, que nasceu da própria triagem).
- **8 de 11** já decididas ou entregues = **73%** da fila era fantasma.
- Placar da triagem: **4 A** (convergência: 251, 255, 300, 309), **1 meia B**
  (reparar o `.ndx` sozinho, recusado **6 × 4**), **4 C**, **3 fora da fila**.
- Latência da decisão mais velha ainda parada como «planejada»: **289 e 290,
  decididos em 17/09 07:10** — **seis dias** parados esperando quem já tinha
  respondido.
- Brinde do mesmo levantamento: das **10** referências `arquivo:linha` citadas
  nos onze pedidos, **7 estavam deslocadas**. Conteúdo certo, coordenada
  movida.

## 4. A regra

**Antes de pedir uma decisão, leia o CORPO do pedido até o fim e procure a
decisão que já está lá — e quando ela estiver, tire o pedido da fila em vez de
repetir a pergunta.** Pedido que carrega a resposta dentro de si não está
esperando o dono: está esperando alguém ler.

E o corolário, que é o que faz a regra durar: **estado de pedido tem de
distinguir «espera o dono» de «decidido, espera engenharia».** Um estado só
para os dois transforma decisão tomada em decisão invisível — e decisão
invisível é pedida de novo.

## 5. Como está guardado hoje, e onde o buraco ficou

**Guardado:** a lei nova no `CLAUDE.md` («perguntar ao dono é ÚLTIMO recurso»,
23/09) ordena a triagem antes da pergunta, e traz o **340** escrito nela como o
caso que a fundou. A triagem do J está em
`docs/propostas/triagem-das-decisoes-do-dono-2026-09-23.md`, com URL e
`arquivo:linha` em cada afirmação.

**O buraco, e ele continua aberto:** o `docs/PENDENCIAS.md` tem **três**
estados — ☑️ feito, ◐ parcial, ☐ planejado — e nenhum deles é «decidido, espera
engenharia». O `pagina-dos-pedidos.py` conta esses três e só esses três, então
a página dos pedidos e o painel PMO **herdam a mesma cegueira**. Enquanto isso
não mudar, a próxima decisão que o dono tomar dentro de um corpo de pedido vai
sumir do mesmo jeito.

Não consertei agora, e o motivo é de processo e não de preguiça: há **quatro
frentes vivas** na mesma árvore, uma delas (**V**) mexendo no
`portao-dos-geradores.py`, e acrescentar estado é mudar o que o gerador
**conta** — número que a página publica. Entra na onda 2, com prova real nos
dois sentidos: a contagem tem de mudar com o estado novo e **não** mudar para
quem não o usa.

**O que NÃO é o conserto:** varrer o corpo dos pedidos procurando a frase
«DECIDIDO PELO DONO». Isso é casar texto, e esta casa já mediu que casar texto
acha o caso e perde a família — foi assim com as oito interpolações de erro cru
das quais só duas eram defeito. O conserto é o **estado**, que é estrutura.
