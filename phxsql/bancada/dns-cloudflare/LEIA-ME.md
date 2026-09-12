# Provisionamento de DNS no Cloudflare — a prova

Cada **servermail** tem um **IP fixo**, e esse IP é cadastrado via **API do
Cloudflare** num registro `A` por empresa:

    <empresa>.phxmail.com.br ->  <ip_fixo>

Esta bancada prova o mecanismo **sem tocar o Cloudflare de verdade e sem gastar
token** — contra um Cloudflare **falso** local que imita só as rotas usadas, com
o envelope idêntico ao real (`{"success","errors","result"}`, conferido no doc
oficial da API v4).

## Arquivos

| Arquivo | O que é |
|---|---|
| `provisionar_dns.py` | A ferramenta de ops (implementação de referência). Idempotente: cria ou atualiza. Token só de `CF_API_TOKEN`, nunca em argv/repo/log. Só a stdlib. |
| `cloudflare_falso.py` | O Cloudflare falso, para a prova. Confere `Authorization: Bearer`; token errado → 403. |
| `rodar.sh` | O roteiro da prova, nos dois sentidos. |
| `resultados.json` | O placar medido da última corrida (lido pela página de testes). |

## Rodar a prova

    ./rodar.sh          # sobe o falso, prova, derruba o falso; PROVA VERDE/VERMELHA

São **10 checagens**: cria o registro; é idempotente (roda de novo e
**atualiza**, não duplica); atualiza quando o IP muda; **falha com token
errado** (é o que faz a prova pegar); e uma empresa não mexe na outra.

## Usar de verdade (no host de provisionamento, não aqui)

    export CF_API_TOKEN='...'            # segredo; nunca no repositorio
    export CF_ZONAS='{"phxmail.com.br":"<zone_id>"}'
    python3 provisionar_dns.py prado 203.0.113.10

Pré-requisitos que **faltam** e são decisão/lavoura do dono: a zona
`phxmail.com.br` existir no Cloudflare, e um **token com escopo `Zone.DNS`** só
nessa zona. Ver `docs/CORREIO-DNS.md`.

## Por que Python, e não o motor Rust

O motor é **zero dependências externas** (pétrea) — e a `std` do Rust não fala
TLS. Falar HTTPS com `api.cloudflare.com` a partir do próprio servidor exigiria
uma crate de TLS, o que **não passa sem o dono** (é o mesmo choque do «TLS na
conexão» já documentado). Provisionar DNS é **ops**, roda raramente (uma vez por
empresa, e quando o IP muda), então mora fora do motor, numa ferramenta de ops
— e o motor continua zero-deps. O «meio» (o servidor chamar isto sozinho, via
`curl`, ou deixar como passo de ops) é a decisão registrada em
`docs/CORREIO-DNS.md`.
