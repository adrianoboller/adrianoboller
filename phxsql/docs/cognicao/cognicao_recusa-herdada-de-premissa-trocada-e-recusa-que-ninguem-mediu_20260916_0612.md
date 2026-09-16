# Cognição: recusa herdada de premissa trocada é recusa que ninguém mediu — e o gargalo do transporte P2P não era a rede

- **Assunto:** P2-DESIGN, a pesquisa do transporte P2P do Pilar 2
  (`docs/propostas/p2p-transporte-2026-09.md`)
- **Descoberta:** 16/09/2026, 06:12 UTC (o relatório do `pesquisa-rede`)
- **Arquivos:** `docs/P2P-DISTRIBUIDO.md` §5.2, `docs/CORREIO-DNS.md`,
  `docs/CORREIO-FORMATO.md` §6, `docs/VISAO.md`, `bancada/replicacao/resultados.json`,
  `bancada/quorum/resultados.json`, `docs/pmo/BACKLOG.md` (linha P2-DESIGN)

## 1. O que aconteceu

O `P2P-DISTRIBUIDO.md` §5.2 recusava DHT para o PhxMail com um motivo escrito:
«manter DNS fixo é decisão de produto, não lacuna técnica». A recusa estava
certa **para a premissa de então** — um servermail por empresa, IP fixo,
registro `A` no Cloudflare (`CORREIO-DNS.md`). A ordem do dono de 16/09
(`VISAO.md`, Pilar 2) trocou a premissa: «sem servidor central de entrega, os
pares trocam mensagem direto», e o servidor do usuário pode estar atrás de NAT
sem porta aberta. O pesquisador não herdou a recusa: remediu. A DHT continua
recusada, mas por **outro** motivo e com número — par achado por DHT atrás de
NAT continua inalcançável, então a DHT não entrega o que falta.

## 2. O que eu concluí primeiro, e estava errado

Eu escrevi o item do board como «descoberta, NAT, gossip/anti-entropia,
identidade sem domínio» — quatro problemas **de rede** — e a lista dizia, sem
dizer, que o gargalo do transporte era a rede. O documento que voltou mediu o
contrário: o atraso de uma inserção até a réplica é **2.012 ms**
(`bancada/replicacao`, 07/09/2026), e o transporte puro custa **0,47 ms**
(`bancada/quorum`, 07/09/2026, localhost, piso declarado). A rede é **0,023%**
do atraso. O resto é o **sono do laço de pull** — e não existe long-poll no
servidor: zero `Condvar` em `phxsql-server/src`, sendo que a peça já existe em
`phxsql-core/src/semaforo.rs`. O item de maior valor ÷ custo não estava na
minha lista de quatro.

E o segundo erro, do mesmo naipe: eu teria deixado a recusa de §5.2 valer como
ponto de partida, porque estava escrita e fundamentada. Recusa fundamentada em
premissa que caducou é a mais perigosa: ela continua parecendo medida.

## 3. O que a medição disse

- Atraso medido até a réplica: **2.012 ms**; transporte puro: **0,47 ms**.
- `Condvar` em `phxsql-server/src`: **0**. `UdpSocket` no repositório: **0**
  em 271 arquivos / 194.328 linhas; `TcpStream` **109**, `TcpListener` **48**.
- Hole punching: **82% UDP / 64% TCP** — Ford/Srisuresh/Kegel, USENIX 2005
  (310/380 e 184/286, 68 fabricantes). **Nenhuma medição primária posterior
  encontrada**; o que circula é blog de fornecedor e uma estimativa declarada
  pelo próprio autor. Vinte anos de idade num número que decidiria investir em
  furar NAT — por isso é a premissa P3, a medir, e não um dado.
- O choque de frentes: o AAD do selo (`CORREIO-FORMATO.md` §6) usa o endereço,
  e a ordem pede identidade por chave sem domínio. Barato hoje, migração de dado
  cifrado depois. Virou a pendência **#251**, parada por decisão do dono.

## 4. A regra

**Quando a premissa de uma recusa muda, remeça a recusa antes de herdá-la — e
grave a nova com o número dela, ao lado da premissa de que depende.**

## 5. Como está guardado hoje

A remedição está no documento da pesquisa (§0 a premissa caducada, §3 a
recusa nova com número, §5 as oito premissas a medir). O choque do AAD está na
pendência #251 e na linha P2-DESIGN do board. O long-poll é a premissa P2 e a
mais barata: repetir `bancada/replicacao/medir.py` com o `replicar` segurando a
resposta, contra os 2.012 ms.

**Onde o buraco ficou:** as recusas desta casa — no `TECNOLOGIAS.md`
(«avaliado e recusado, com o número»), no `P2P-DISTRIBUIDO.md`, no
`DESEMPENHO.md` — **não carregam a premissa de que dependem**. Quem lê vê o
número e o motivo, não a hipótese que os sustenta; quando a hipótese cai,
ninguém sabe qual recusa caiu junto. Não há guarda para isso, e não é catraca
que resolve: é disciplina de escrita — a recusa nova de §3 já nasce assim, com
a premissa nomeada.
