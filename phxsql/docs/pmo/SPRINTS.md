# Sprints faltantes — PhxSql

```text
Sessão v2 | Sprint SPR-00 (planejamento) | 01/10/2026
```

**Por que SPR e não SP.** Nesta casa `SP000025`, `SP000057`… são **códigos de erro**
do motor (`[SP000025] acesso negado`). Sprint com o mesmo prefixo confundiria quem
lê um log com quem lê o plano. Numeração global, sequencial, sem reuso, a partir de
SPR-01.

**De onde sai cada item:** `docs/PENDENCIAS.md` (☐ e ◐ em 01/10/2026). **Ordem:**
decisão do dono de 01/10/2026 — estabilizar (fechar os defeitos da conta) → 572
(TLS 1.3) → 325 → 454/455 → 333 → 495/496. Estimativas em **ondas** (uma onda =
3 a 4 frentes de código em paralelo + integração; hoje rendeu ~meio dia cada) —
**estimadas, não medidas**.

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
| **Estado** | 290/294 e 513 prontos, em integração; 329/331 e 607–609 em frente |
| **Aceite** | portões verdes no HEAD unido; guardas novas PROVADAS; nada apagado sem `limpar-frentes.sh` |

## SPR-02 — Segurança que resta

| Campo | Conteúdo |
|---|---|
| **Itens** | 610 (teto de bytes no motor phxsql), 611 (manifesto e destino do backup pelo descritor), 612 (SCRAM final no cliente PG), 613 (réplica sem cofre recusa — decisão do dono), 606 (chave fraca no Windows = fatia T0 do TLS) |
| **Papéis** | B (titular), SEC (revisão adversária no fim) |
| **Aceite** | RED/GREEN por achado; a SEC reroda a revisão independente sobre o diff e não acha ATIVO |
| **Risco** | 606 não se mede sem Windows: prova pelo motor da `std` em Linux + leitura |

## SPR-03 — Formato antes de selar (parecer do DBA)

| Campo | Conteúdo |
|---|---|
| **Itens** | 601 (linhagem da tabela, UUID v7 no PSCH v11), 603 (teto de 32.767 colunas + evento antigo sem o bit), 605 (fsync que garante um terceiro — MEDIR primeiro), ressalvas do 290 |
| **Papéis** | C (decide e revisa), B |
| **Aceite** | `FORMATO.md` atualizado; banco antigo abre (teste do comportamento velho); queda simulada para o 605 |
| **Por que antes da 0.19.0** | depois de selar, mudança de formato vira migração |

## SPR-04 — Garantias da replicação e do cluster

| Campo | Conteúdo |
|---|---|
| **Itens** | 297, 300, 309 (rownum honrado na réplica fiel), 330 (absorção sob trava de leitura), 424 (censo e aviso, pétrea ganhou), 207 (quórum: invisível até o ok, 10 s, degrada dizendo), 513 passo 2 |
| **Decisões** | todas já tomadas pelo J em `docs/propostas/decisoes-onda-01-10-2026.md` |
| **Aceite** | replicação real entre servidores por soquete; 513 passo 2 com prova de 1 GB e escritor concorrente |

## SPR-05 — Higiene do motor, do ODBC e das guardas

| Campo | Conteúdo |
|---|---|
| **Itens** | 175, 229, 238, 245 O2(b), 249, 259, 322, 364, 427, 524 (resto), 263 (as 177 guardas nunca julgadas, em lotes de 1 compilação), 268 (ou ⏸ com motivo) |
| **Papéis** | B, G (guardas), C (245 O2b) |
| **Aceite** | conta da F1 em 0% de ☐/◐ ativos fora de decisão do dono |

---

# FASE 2 — SELAR

## SPR-06 — Selagem da 0.19.0

| Campo | Conteúdo |
|---|---|
| **Itens** | portão da versão (hoje vermelho: 0.19.0 a ~300 commits da selagem), backup total provado, dossiê e páginas republicadas, 338 fechado, 190 (bateria de todos os botões, no navegador) |
| **Do dono** | 326 — refazer o compartilhamento a partir da versão viva; 339(c) — validação jurídica |
| **Aceite** | `portao-dos-geradores.py` verde; pacote restaurado byte a byte; dossiê com o selo do commit selado |

---

# FASE 3 — TLS 1.3 (pedido 572; plano em `docs/propostas/plano-tls13-572.md`)

O servidor já está pronto (T1–T6a, 30/09). Falta o lado de quem conecta.

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

Cada uma da F4 nasce com uma sprint de desenho do J antes de virar ondas de código;
o tamanho só se escreve depois do desenho — antes disso seria número inventado.
