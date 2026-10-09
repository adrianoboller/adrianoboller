# Sprints faltantes — PhxSql

```text
Sessão v2 | Fecho do dia 01/10/2026 (papel H) | estado contado por docs/pmo/sprints.py
```

**Por que SPR e não SP.** Nesta casa `SP000025`, `SP000057`… são **códigos de erro**
do motor (`[SP000025] acesso negado`). Sprint com o mesmo prefixo confundiria quem
lê um log com quem lê o plano. Numeração global, sequencial, sem reuso, a partir de
SPR-01.

**De onde sai cada item:** `docs/PENDENCIAS.md`. **Quais pedidos** cada sprint leva é
decisão de plano e está na linha `**Pedidos**` dela; **o estado** de cada um não se
escreve aqui — sai do `PENDENCIAS.md` pelo gerador, no bloco abaixo. **Ordem:**
decisão do dono de 01/10/2026 — estabilizar (fechar os defeitos da conta) → 572
(TLS 1.3) → 325 → 454/455 → 333 → 495/496. Estimativas em **ondas** (uma onda =
3 a 4 frentes de código em paralelo + integração; hoje rendeu ~meio dia cada) —
**estimadas, não medidas**.

## Estado medido

<!-- SPRINTS:inicio -->
_Contado do `PENDENCIAS.md` por `python3 docs/pmo/sprints.py`, gerado em 09/10/2026 03:08 UTC. Não se edita: muda a linha `**Pedidos**` da sprint ou o estado no `PENDENCIAS.md`, e roda o gerador._

| Sprint | Pedidos | ☑️ feitos | ◐ | ☐ | ⏸ | Abertos (◐ + ☐) |
|---|---:|---:|---:|---:|---:|---|
| SPR-01 · A onda em curso (integrar o que já está pronto) | 10 | 10 | 0 | 0 | 0 | 0 |
| SPR-02 · Segurança que resta | 8 | 8 | 0 | 0 | 0 | 0 |
| SPR-03 · Formato antes de selar (parecer do DBA) | 3 | 3 | 0 | 0 | 0 | 0 |
| SPR-04 · Garantias da replicação e do cluster | 8 | 8 | 0 | 0 | 0 | 0 |
| SPR-05 · Higiene do motor, do ODBC e das guardas | 16 | 16 | 0 | 0 | 0 | 0 |
| SPR-06 · Selagem da 0.19.0 | 4 | 2 | 0 | 0 | 2 | 0 |
| SPR-07 · T6b-1 cliente TLS por pino da chave + T6e (buracos do servidor) | — | — | — | — | — | sem lista de pedidos (não contada) |
| SPR-08 · T6b-2 TLS no lugar do Noise (réplica/cluster/cmd/DbLink phx, Remoto, ODBC); servidor ainda aceita Noise | — | — | — | — | — | sem lista de pedidos (não contada) |
| SPR-09 · T6c-1 RSA (PKCS#1 v1.5 e PSS), P-384, SHA-384 | — | — | — | — | — | sem lista de pedidos (não contada) |
| SPR-10 · T6c-2 cadeia e nome do servidor (RFC 5280/9525) | — | — | — | — | — | sem lista de pedidos (não contada) |
| SPR-11 · T6d TLS de saída: PostgreSQL, MySQL, SMTP | — | — | — | — | — | sem lista de pedidos (não contada) |
| SPR-12 · 325 — 20 caixas de supermercado e 1 servidor (contingência) | 1 | 1 | 0 | 0 | 0 | 0 |
| SPR-13 · 454/455 — PhxZip como produto (web, PhxZipCmd, pacote, manual) | 2 | 0 | 0 | 0 | 2 | 0 |
| SPR-14 · 333 — chat estilo WhatsApp e robô no PhxMail | 1 | 0 | 0 | 0 | 1 | 0 |
| SPR-15 · 495/496 — as duas IAs (crime cibernético; DBA sênior) | 2 | 0 | 0 | 0 | 2 | 0 |
| **Total nas sprints** | 55 | 48 | 0 | 0 | 7 | 0 |

**Abertos (◐ + ☐) no `PENDENCIAS.md` fora de sprint nenhuma: 1** — 751.
<!-- SPRINTS:fim -->

O bloco conta o `PENDENCIAS.md` **da árvore de trabalho** na hora da corrida. Em
01/10/2026 o 175 e o 364 já estavam ☑️ ali, mas o merge do integrador que os fecha
ainda não estava comitado: se o merge cair, o gerador volta a contá-los abertos
sozinho.

## Visão geral

| Fase | Sprints | Foco | Papéis | Ondas (estim.) |
|---|---|---|---|---:|
| F1 · Estabilizar | SPR-01 – SPR-05 | defeitos ativos na conta | B, C, G, SEC | 5 |
| F2 · Selar 0.19.0 | SPR-06 | portão da versão, backup, dossiê | A, H, I, E | 1 |
| F3 · TLS 1.3 (572) | SPR-07 – SPR-11 | cliente, RSA/P-384, cadeia, saída | J, B, SEC | 6–7 |
| F4 · Produto | SPR-12 – SPR-15 | 325, 454/455, 333, 495/496 | J, B, E, H | a medir |

---

# FASE 1 — ESTABILIZAR

## SPR-01 — A onda em curso (integrar o que já está pronto)

| Campo | Conteúdo |
|---|---|
| **Itens** | 290 + 294 (decisões do dono de 17/09), 513 passo 1, 329 + 331 + troca de chave (com o 604), 607 + 608 + 609 (SEC, o 607 é ALTO), fechar 293 como superado pelo 344 + 613 |
| **Pedidos** | 290 294 329 331 604 607 608 609 293 615 |
| **Fechou** | todos os da lista; em 01/10 fechou também o 615 (irmão do 290: a faixa da sequência chega à CLI e à FFI pelo mesmo motor, `cad5f2a2`). O 513 conta no SPR-04, onde está o passo 2 que resta |
| **Resta** | nada aberto — ver o bloco |
| **Aceite** | portões verdes no HEAD unido; guardas novas PROVADAS; nada apagado sem `limpar-frentes.sh` |

## SPR-02 — Segurança que resta

| Campo | Conteúdo |
|---|---|
| **Itens** | 610 (teto de bytes no motor phxsql), 611 (manifesto e destino do backup pelo descritor), 612 (SCRAM final no cliente PG), 613 (réplica sem cofre recusa — decisão do dono), 606 (chave fraca no Windows = fatia T0 do TLS) |
| **Pedidos** | 610 611 612 613 606 616 618 619 |
| **Fechou em 01/10** | 616 — coluna INLINE marcada sem cofre recusa também inline e no bidirecional (`b528b9d3`), achado da frente do 613 |
| **Resta** | 618 (M1, LGPD: `.fts.novo` órfão de redeclaração interrompida) e 619 (M2: redeclarar índice de texto pede só `Criar`), os dois da revisão SEC de 01/10 sobre o 364 |
| **Papéis** | B (titular), SEC (revisão adversária no fim) |
| **Aceite** | RED/GREEN por achado; a SEC reroda a revisão independente sobre o diff e não acha ATIVO |
| **Risco** | 606 não se mede sem Windows: prova pelo motor da `std` em Linux + leitura |

## SPR-03 — Formato antes de selar (parecer do DBA)

| Campo | Conteúdo |
|---|---|
| **Itens** | 601 (linhagem da tabela, UUID v7 no PSCH v11), 603 (teto de 32.767 colunas + evento antigo sem o bit), 605 (fsync que garante um terceiro — MEDIR primeiro), ressalvas do 290 |
| **Pedidos** | 601 603 605 |
| **Fechou em 01/10** | 601 + 603 — linhagem da tabela no PSCH v11; teto de colunas e o evento sem o bit do selo (`6fc8244e`) |
| **Resta** | 605 ☐ — medir primeiro (queda simulada); raciocinado, não medido |
| **Papéis** | C (decide e revisa), B |
| **Aceite** | `FORMATO.md` atualizado; banco antigo abre (teste do comportamento velho); queda simulada para o 605 |
| **Por que antes da 0.19.0** | depois de selar, mudança de formato vira migração |

## SPR-04 — Garantias da replicação e do cluster

| Campo | Conteúdo |
|---|---|
| **Itens** | 297, 300, 309 (rownum honrado na réplica fiel), 330 (absorção sob trava de leitura), 424 (censo e aviso, pétrea ganhou), 207 (quórum: invisível até o ok, 10 s, degrada dizendo), 513 passo 2 |
| **Pedidos** | 297 300 309 330 424 207 513 620 |
| **Fechou em 01/10** | 309 + 297 — réplica fiel honra o `rownum` da imagem; lixeira da réplica e do nó, por escrito (`f703f0e7`). 424 — censo do ledger (`b236d662`). 330 e 300 andaram e seguem ◐ (`b236d662`) |
| **Resta** | 330 ◐: teto de RAM do mapa de toques (papel C). 300 ◐: item (4) escrita local na réplica pulando evento, contador de órfãs (§2.7), master contando tabela negada. 207 ◐: escrita com quórum (o «ok» já decidido pelo dono). 513 ◐: passo 2 (cópia sem trava + acerto curto, prova de 1 GB com escritor). 620 ☐: mapa de toques do bidirecional não zera quando a tabela some (SEC B1 sobre o 330) |
| **Decisões** | todas já tomadas pelo J em `docs/propostas/decisoes-onda-01-10-2026.md` |
| **Aceite** | replicação real entre servidores por soquete; 513 passo 2 com prova de 1 GB e escritor concorrente |

## SPR-05 — Higiene do motor, do ODBC e das guardas

| Campo | Conteúdo |
|---|---|
| **Itens** | 175, 229, 238, 245 O2(b), 249, 259, 322, 364, 427, 524 (resto), 263 (as 177 guardas nunca julgadas, em lotes de 1 compilação), 268 (ou ⏸ com motivo) |
| **Pedidos** | 175 229 238 245 249 259 322 364 427 524 263 268 617 621 622 623 |
| **Fechou em 01/10** | 617 + 322 — veredito do libtest atravessado pelo stderr; Portão 4 lê todas as tabelas do pedido (`c17df0b0`). 175 + 364 ☑️ no `PENDENCIAS.md`, merge do integrador ainda não comitado |
| **Resta** | ver o bloco: os ◐ de higiene (229, 238, 245, 249, 259, 263, 524), os ☐ 427 e 268, e três da auditoria QA de 01/10: 621 (páginas publicam `TETO_DE_COLUNAS = 0`), 622 (nove pétreas sem guarda que reponha o defeito), 623 (teste que floca) |
| **Papéis** | B, G (guardas), C (245 O2b) |
| **Aceite** | conta da F1 em 0% de ☐/◐ ativos fora de decisão do dono |

---

# FASE 2 — SELAR

## SPR-06 — Selagem da 0.19.0

| Campo | Conteúdo |
|---|---|
| **Itens** | portão da versão (hoje vermelho: 0.19.0 a ~300 commits da selagem), backup total provado, dossiê e páginas republicadas, 338 fechado, 190 (bateria de todos os botões, no navegador) |
| **Pedidos** | 338 190 326 339 |
| **Do dono** | 326 — refazer o compartilhamento a partir da versão viva; 339(c) — validação jurídica |
| **Aceite** | `portao-dos-geradores.py` verde; pacote restaurado byte a byte; dossiê com o selo do commit selado |

---

# FASE 3 — TLS 1.3 (pedido 572; plano em `docs/propostas/plano-tls13-572.md`)

O servidor já está pronto (T1–T6a, 30/09). Falta o lado de quem conecta.

As cinco sprints são **fatias de um pedido só**, o 572 (◐). Por isso nenhuma tem
linha `**Pedidos**`: o gerador as mostra como «não contada» e nomeia o 572 entre
os abertos fora de sprint, em vez de contar o mesmo pedido cinco vezes ou de dar
zero aberto a quatro delas.

| Sprint | Fatia | Prova |
|---|---|---|
| SPR-07 | T6b-1 cliente TLS por pino da chave + T6e (buracos do servidor) | RFC 8448 §3/§5 e `openssl s_server` |
| SPR-08 | T6b-2 TLS no lugar do Noise (réplica/cluster/cmd/DbLink phx, Remoto, ODBC); servidor ainda aceita Noise | os 3 pontos por soquete |
| SPR-09 | T6c-1 RSA (PKCS#1 v1.5 e PSS), P-384, SHA-384 | vetores NIST e Wycheproof |
| SPR-10 | T6c-2 cadeia e nome do servidor (RFC 5280/9525) | x509-limbo |
| SPR-11 | T6d TLS de saída: PostgreSQL, MySQL, SMTP | contra os servidores reais do contêiner |

**Do dono, sem pressa:** quando o servidor deixa de aceitar o Noise.

---

# FASE 4 — PRODUTO (na ordem do dono)

| Sprint | Pedido | Primeiro passo |
|---|---|---|
| SPR-12 | 325 — 20 caixas de supermercado e 1 servidor (contingência) | J: desenho medido (modo offline do caixa, reconciliação) antes de código |
| SPR-13 | 454/455 — PhxZip como produto (web, PhxZipCmd, pacote, manual) | E + B; o 455 já está ◐ |
| SPR-14 | 333 — chat estilo WhatsApp e robô no PhxMail | J: desenho; depende do 572 (cifra) |
| SPR-15 | 495/496 — as duas IAs (crime cibernético; DBA sênior) | J: hipóteses e o que medir |

| Sprint | Pedidos |
|---|---|
| **Pedidos SPR-12** | 325 |
| **Pedidos SPR-13** | 454 455 |
| **Pedidos SPR-14** | 333 |
| **Pedidos SPR-15** | 495 496 |

Cada uma da F4 nasce com uma sprint de desenho do J antes de virar ondas de código;
o tamanho só se escreve depois do desenho — antes disso seria número inventado.
