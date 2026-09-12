# PhxMail × SMTP — o comparativo honesto, e como a comunicação funciona

**Papéis:** J (pesquisa) + H (documentação). **Data:** 11/09/2026.

Este documento responde a duas perguntas do dono: o comparativo do correio nativo
do PhxSql (phxmail) com o e-mail tradicional (SMTP), com vantagens e desvantagens;
e como seria a comunicação entre todos os usuários e servidores usando o phxmail.

A honestidade vem antes do elogio, porque a lei da casa é a mesma para dentro e
para fora: **número citado é número que não se mede**, e vantagem que esconde o
preço não é vantagem, é propaganda.

## 0. A moldura, sem a qual a tabela engana

**O phxmail NÃO é um substituto do e-mail internet.** SMTP é o padrão universal,
federado, aberto, que liga qualquer endereço a qualquer endereço no planeta desde
1982 (RFC 821, hoje RFC 5321). O phxmail é outra coisa: um **correio corporativo
cifrado fim-a-fim, sem spam por desenho, restrito a quem controla as duas pontas**
— só o domínio `phxmail.com.br`, só entre usuários que
aceitaram uma relação de confiança. Está mais perto de um Signal/Matrix federado e
cercado do que do Gmail.

Comparar os dois é comparar um **jardim murado com garantias fortes** a uma **rede
aberta, universal e fracamente autenticada**. Cada vantagem do phxmail existe
*porque* ele abre mão da universalidade. Quem precisa mandar e-mail para a avó no
Gmail: é SMTP, não há discussão. Quem precisa de um canal interno (ou entre
empresas parceiras) cifrado, auditável e sem spam: é aí que o phxmail ganha.

## 1. Comparativo, dimensão a dimensão

| Dimensão | SMTP (tradicional) | PhxMail | Quem ganha |
|---|---|---|---|
| **Alcance** | Universal e federado: qualquer endereço, qualquer provedor, via DNS MX | Cercado: só `phxmail.com.br`, só entre pares com confiança | **SMTP**, e por muito |
| **Interoperabilidade** | Décadas de clientes (Outlook, Thunderbird, Apple Mail, apps), bibliotecas, RFCs | Nenhum cliente pronto; protocolo próprio na porta 8000 | **SMTP** |
| **Cifra do conteúdo** | Texto claro por padrão; TLS só protege salto-a-salto (o servidor LÊ); E2E (PGP/S-MIME) é opcional e raro | **E2E obrigatório**: ECDH X25519 por par, o servidor NÃO lê | **PhxMail** |
| **Spam** | Aceita de qualquer um; guerra eterna de SPF/DKIM/DMARC/greylist/filtro | **Sem contato sem confiança aceita** — nos dois canais (mensagem e pedido) | **PhxMail** |
| **Falsificação do remetente** | `From:` se forja trivialmente; SPF/DKIM/DMARC são remendos | Remetente é par de chaves + senha; `de`/`para` autenticados no AAD do selo | **PhxMail** |
| **Prioridade** | `X-Priority` é conselho, e se forja | Prioridade e tipo (🚨/🔔) **dentro do AAD** — rebaixar um alerta quebra a etiqueta | **PhxMail** |
| **Moderação** | Nenhuma nativa | Banir/quarentena/multa com **motivo obrigatório**, e a decisão também com motivo | **PhxMail** |
| **Anexos** | Blob MIME dentro da mensagem (base64, incha 33%) | Armazém **à parte**, cifrado, a mensagem guarda só a referência | **PhxMail** |
| **É consultável?** | Mora em mbox/Maildir/IMAP — infra separada | **É o próprio banco**: transação, índice, backup e replicação do motor | **PhxMail** |
| **Contato a frio (legítimo)** | Sem atrito: qualquer um te escreve | Atrito por desenho: pedir e aceitar antes do primeiro contato | **SMTP** (é atrito, não defeito) |
| **Entrega assíncrona / entre operadores** | Store-and-forward maduro: fila, retry, bounce, DSN (RFC 3461) | Modelo de entrega entre servidores ainda é **proposta**, não padrão | **SMTP** |
| **Recuperação de senha** | Correio mora no servidor; o provedor recupera | Esqueceu a senha = perdeu o correio (a menos que exista escrow, que **enfraquece** o E2E) | **SMTP** (é o preço do E2E de verdade) |
| **Sigilo futuro (forward secrecy)** | Não tem (nem SMTP+PGP) | Ainda não: chaves X25519 **estáticas** — vazou a privada, vaza o histórico do par | Empate (nenhum tem hoje) |
| **Metadado** | Envelope (de/para) em claro para o operador | `de`/`para`/prioridade/tipo em claro no AAD, para roteirizar — o grafo social não se esconde do operador | Empate |
| **Maturidade** | Batido desde 1982, todo caso-limite já apanhado | Novo, não provado em escala | **SMTP** |
| **Dependências** | Stacks enormes (Postfix, Exim, Dovecot…) | Zero dependências externas — a cifra é a da casa (X25519/HKDF/ChaCha20-Poly1305/PBKDF2), conferida contra vetor | **PhxMail** |

## 2. Como a comunicação funciona entre todos os usuários e servidores

O desenho abaixo é o que o protótipo `crates/phxsql-core/examples/correio-e2e.rs`
prova em memória (24/24 PROVA VERDE). O que já é verdade e o que ainda é proposta
está marcado no fim da seção — não se anuncia como pronto o que não está.

### A identidade

Cada usuário tem uma identidade **X25519**. A chave **privada mora cifrada sob a
senha** dele (PBKDF2 → ChaCha20-Poly1305); a **pública** fica em claro. A senha
destranca só a **própria** privada, e mais nada.

### O endereço e o servidor

O endereço é `usuario@empresa.phxmail.com.br`. O `empresa` é um subdomínio, e o
**alias dele é um registro DNS criado na Cloudflare** apontando para o *server
mail* que hospeda aquele inquilino (tenant). Um server mail hospeda **muitas
caixas** e recebe **muitos clientes conectados** ao mesmo tempo — pela porta TCP
**8000**, com usuário e senha, sobre a cifra do fio (não é SMTP, não é TLS de
biblioteca: é o aperto de mão próprio da casa).

### A relação de confiança (o anti-spam)

Antes de A poder falar com B, **A pede confiança e B aceita** — vale igual para a
mesma empresa e para empresas diferentes. Sem confiança aceita, não há entrega, e
não só na mensagem: **o canal de pedido também é limitado** (pedido repetido
recusado, bloqueado não pede, teto de pendentes). É o que mata o spam na raiz, em
vez de filtrá-lo depois.

### O envio (o E2E, sem nunca pedir a senha do outro)

Correio é assíncrono: quando A manda, B pode estar desligado, e A nunca tem a senha
de B. Então NÃO se combinam as duas senhas. Usa-se **ECDH**:

1. A abre a **própria** privada com a **própria** senha.
2. A faz `segredo = ECDH(privada_A, pública_B)` — a simetria do Diffie-Hellman.
3. Desse segredo sai, por HKDF com sal por mensagem, a chave que sela o corpo (e
   cada anexo, no armazém à parte) com ChaCha20-Poly1305. `de`/`para`/prioridade/
   tipo entram no **AAD** — mexer neles quebra a etiqueta.

B decifra com a **própria** senha (que abre a privada de B) + a chave **pública**
de A. "A senha dele com a [chave do] outro" — a pública, nunca a senha.

### Entre servidores diferentes (a federação cercada)

Quando A (no servidor X) manda para B (no servidor Y, outra empresa): X resolve
`empresa.phxmail.com.br` pelo DNS → conecta em Y pela porta 8000 → entrega o
**blob cifrado**. Y guarda na caixa de B e **não consegue ler** — só a senha de B
com a pública de A abre. É federação, mas **cercada**: só domínios phxsql/phxmail,
e só entre pares com confiança.

### A leitura, e o monitor

B conecta o *client mail*, que **monitora a caixa** (a chegada gera popup e flag de
novos) e lê a mensagem com a própria senha. Camadas extras opcionais encaixam por
cima: **alto segredo** (uma frase passada por telefone/SMS) e **Masson** (flag X, a
terceira camada) — cada uma exige o próprio segredo para abrir.

### Moderação

Pedidos de banir/quarentena/multa vão ao servidor/administrador, sempre com motivo,
e a decisão (deferir/indeferir) também carrega motivo.

### O que já é verdade, e o que é proposta

- **Provado (em memória, 24/24):** identidade X25519 selada pela senha; gate de
  confiança nos dois canais; E2E por ECDH; três camadas; anexos à parte;
  moderação com motivo; estado das solicitações.
- **Proposta, decide-se com o dono antes de gravar** (`docs/CORREIO-FORMATO.md`):
  o formato em disco das tabelas PSCH, a data/hora de sistema por linha, e a
  **entrega entre servidores** (o roteamento X→Y na porta 8000 ainda não abre
  soquete no protótipo).
- **Aberto:** recuperação de senha (enfraquece a garantia), forward secrecy
  (ratchet), revogação de confiança.

## 3. Veredito — quando cada um

- **SMTP** quando você precisa alcançar o mundo: clientes, fornecedores, qualquer
  endereço lá fora. É insubstituível para isso, e o phxmail não tenta substituí-lo.
- **PhxMail** quando as duas pontas são suas ou de parceiros que aceitam a relação:
  um canal interno (ou entre empresas conhecidas) **cifrado de verdade, sem spam,
  auditável, e que é o próprio banco** — com backup, replicação e consulta de
  graça, porque a mensagem é uma linha de tabela como qualquer outra.

A frase honesta para a marca: *correio corporativo cifrado e sem spam, para quem
controla as duas pontas* — nunca *substitui seu e-mail*.
