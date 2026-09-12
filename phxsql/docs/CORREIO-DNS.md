# CORREIO — IP fixo e DNS no Cloudflare

> Ordem do dono, 11/09/2026: *«Cada servermail deve estar num IP fixo e esse IP
> fixo deve estar cadastrado via API do Cloudflare nos registros DNS
> `nome_empresa.phxsql.com.br` e `nome_empresa.phxmail.com.br`.»*
>
> **Atualização, 12/09/2026:** *«phxsql.com.br pode remover; vamos usar apenas o
> phxmail.com.br.»* — a partir daqui há **um** domínio e **um** registro por
> empresa. O texto abaixo já reflete a decisão nova.

## 0. O que isto decide

Como um cliente acha o servermail da empresa dele **sem SMTP e sem MX**: por
nome, resolvido a um **IP fixo**, publicado no Cloudflare. **Um nome por
empresa, um IP:**

    <empresa>.phxmail.com.br  A   <ip_fixo_do_servermail>

O cliente conecta na **porta 8000** desse IP e faz o aperto cifrado (Noise). O
DNS é só o mapa nome→IP; a confiança do canal continua vindo da pinagem da
chave pública do servidor, **não** do DNS (DNS não autentica ninguém).

## 1. Um domínio só (era dois)

Até 11/09 o correio permitia dois domínios (`phxsql.com.br` e `phxmail.com.br`);
em 12/09 o dono removeu o `phxsql.com.br`. Fica **só o `phxmail.com.br`** — um
registro `A` por empresa, um IP. Menos superfície, menos zona para manter, e o
nome do produto (phxmail) é o que sobra. Quando o IP do servermail muda, o
registro muda; não há mais um par para manter em sincronia.

## 2. O dado: quem guarda o quê (papel C — DBA)

O IP fixo e o registro são **metadados do tenant**, não caixa de e-mail. Moram
no cadastro de empresas do servermail (a tabela que o painel do console mostra),
um por empresa:

    empresa        : rotulo DNS (a-z 0-9 -), unico
    ip_fixo        : IPv4 do servermail que hospeda a empresa
    registro       : { fqdn, zone_id, record_id, estado, medido_em }
    provisionado_em: instante da ultima sincronizacao bem-sucedida

`estado` ∈ { `pendente`, `no_ar`, `erro` }. `record_id` é o id que o Cloudflare
devolve — guardá-lo é o que torna a próxima sincronização um **update** barato
em vez de um create que duplicaria. É o mesmo princípio do resto da casa:
guarda-se a chave para não varrer depois.

**Idempotência é lei aqui**: provisionar de novo (mesmo IP) não pode criar um
segundo registro. A ferramenta confere por nome antes de gravar (ver §4).

## 3. A API do Cloudflare, conferida (papel J)

Conferida no doc oficial da API v4 (não de memória). Base:
`https://api.cloudflare.com/client/v4`.

| Ação | Método | Caminho |
|---|---|---|
| listar por nome | `GET` | `/zones/{zone_id}/dns_records?type=A&name={fqdn}` |
| criar | `POST` | `/zones/{zone_id}/dns_records` |
| atualizar | `PUT` | `/zones/{zone_id}/dns_records/{record_id}` |

Cabeçalhos: `Authorization: Bearer {token}` e `Content-Type: application/json`.
Corpo (criar/atualizar):

    { "type": "A", "name": "prado.phxmail.com.br",
      "content": "203.0.113.10", "ttl": 3600, "proxied": false }

`proxied: false` de propósito — a porta 8000 é TCP do nosso protocolo, não HTTP;
não pode passar pelo proxy HTTP do Cloudflare. Envelope de resposta, igual no
sucesso e no erro:

    { "success": true|false, "errors": [ {"code":N,"message":"..."} ],
      "result": {...}|[...]|null }

## 4. Idempotência (cria-ou-atualiza)

Para o domínio `phxmail.com.br`:

1. `GET .../dns_records?type=A&name={empresa}.phxmail.com.br`
2. veio registro? → `PUT .../{record_id}` (atualiza o `content`)
3. não veio? → `POST .../dns_records` (cria)
4. `success:false` no envelope → falha com a mensagem do Cloudflare (nunca com
   o token)

Rodar duas vezes com o mesmo IP: a 1ª cria, a 2ª atualiza — **um registro, nunca
dois**. IP mudou: roda de novo, o registro vira o IP novo.

## 5. A pétrea que este item toca: zero dependências × HTTPS

Falar com `api.cloudflare.com` é **HTTPS**, e a `std` do Rust **não tem TLS**.
Fazer o motor chamar a API sozinho exigiria uma crate de TLS — e isso é
exatamente o choque já documentado no `CLAUDE.md` («TLS na conexão»): o
**comportamento** (registrar o DNS) é meta; o **meio** (puxar uma crate) **não
passa sem o dono**, porque a pétrea é mais forte que a conveniência.

**Direção do dono, 12/09/2026:** o cadastro é feito pela **API interna do
binário** falando com o Cloudflare — o dono aponta para o mundo do **(b)** (o
binário fazendo a chamada), não uma ferramenta de ops à parte. Isso torna a
pétrea zero‑deps × TLS **viva**, e a decisão que resta é só o **como o binário
fala HTTPS sem crate**:

- **(b) `curl` por *shell‑out* (recomendado):** binário do sistema, **não** uma
  crate — `cargo build --offline` continua válido. Uma dependência de runtime
  (`curl` instalado), não de compilação.
- **(c) escrever um cliente TLS na `std`** (como se fez com SHA‑256/HMAC): mais
  caro e arriscado; só se o dono quiser o motor autossuficiente também aqui.

A ferramenta de ops (`bancada/dns-cloudflare/provisionar_dns.py`, só stdlib) e a
prova do Cloudflare falso continuam valendo como **implementação de referência e
teste** do que o binário passará a fazer.

## 5.1 Fluxo de cadastro de um servidor (por e‑mail) — decisão do dono, 12/09

O cadastro de um servidor da rede **não** usa token pré‑compartilhado; a
credencial é **liberada por um aperto por e‑mail**:

1. **Pedido.** O servidor novo manda um **e‑mail para `cadastro@phxmail.com.br`**
   (mailbox de sistema) pedindo cadastro — com sua identidade: `pub_no`, o
   `ip_fixo` pretendido e o `rotulo` da empresa.
2. **Liberação da senha.** O sistema central valida o pedido e **libera uma
   senha** (credencial de cadastro, de escopo restrito e curta duração). Essa
   senha **nunca** vai em claro para repo/log; é o gatilho do `cadastro_estado`
   do servermail passar de `pendente` para `liberado`.
3. **Cadastro no Cloudflare.** Com essa senha, a **API interna do binário** chama
   o Cloudflare (o §3/§4) e cria o registro `A` — e o cadastro passa a funcionar.

O e‑mail do passo 1 é o próprio correio (porta 8000, cifrado), então o aperto de
cadastro anda pelo mesmo cano do produto. A senha liberada é o que amarra «quem
pediu» a «quem pode escrever no DNS», sem espalhar um token global.

## 6. Segredo (papel Segurança)

- O **token** vem só de `CF_API_TOKEN` (variável de ambiente). Nunca em argv,
  nunca em log, nunca no repositório. A ferramenta não imprime o cabeçalho de
  autorização em nenhum caminho, nem no de erro.
- Escopo mínimo: um token **`Zone.DNS` (edit)** restrito à zona `phxmail.com.br`.
  Não um token global de conta.
- `zone_id` **não** é segredo (é identificador público da zona), mas vem por
  configuração (`CF_ZONAS`), não cravado no código.

## 7. O que falta para valer de verdade (passo de implantação)

Medido na bancada do server mail (07–11/09): **`phxmail.com.br` não resolve
hoje** — a zona ainda não está no Cloudflare. Antes de qualquer chamada real:

1. registrar/mover a zona `phxmail.com.br` para o Cloudflare (dá o `zone_id`);
2. emitir o token `Zone.DNS` restrito;
3. cada servermail nascer com um IP fixo (VM/host com IP estável).

Só então a ferramenta do §4 cadastra de verdade. **Nada disso foi feito nesta
sessão** (não há token, a zona não existe, e tocar produção é decisão do dono).

## 8. A prova

`bancada/dns-cloudflare/` prova o mecanismo contra um **Cloudflare falso** local
— cria, é idempotente, atualiza no IP novo, **falha com token errado**, e uma
empresa não mexe na outra: **10/10 PROVA VERDE**, sem tocar a API real nem
gastar token. Ver o `LEIA-ME.md` da pasta.

## 9. Na tela

O console do servermail (`docs/dossie/tela-servermail.html`) mostra, por
empresa, o **IP fixo** e o **registro** com o estado no Cloudflare
(`no ar` / `pendente`), e a ação «Provisionar DNS».
