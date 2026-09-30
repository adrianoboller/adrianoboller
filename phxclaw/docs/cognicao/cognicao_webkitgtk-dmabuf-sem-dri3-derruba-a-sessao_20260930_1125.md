# WebKitGTK sem DRI3 derrubava a sessão WebDriver no meio (não era o app)

**Estado:** INFRUTÍFERO

**Causa:** o renderizador dmabuf do WebKitGTK precisa de DRI3; o Xvfb não tem, e o processo web caía em ponto aleatório da sessão (5/9 verdes). A hipótese «defeito do app sob IPC» morreu: o binário sozinho viveu 3×25 s.

**Prevenção:** o script que sobe o próprio Xvfb define `WEBKIT_DISABLE_DMABUF_RENDERER=1` (18/18 verdes; `WEBKIT_DISABLE_COMPOSITING_MODE=1` sozinho também resolve, 8/8). Teste de UI que floca se investiga antes de se repetir.

## O que aconteceu

`tests/desktop/desktop_e2e.py` falhava com `RemoteDisconnected` em checagens diferentes.

## O que eu concluí primeiro, e estava errado

Que o app morria sob a sequência de `invoke`. O app isolado não cai.

## O que a medição disse

Sem a variável: 5/9. Com ela: 18/18. Cada variável sozinha: 8/8.

## A regra

Separar app × infraestrutura rodando o binário sem o driver antes de mexer no código.

## Como está guardado hoje

Na linha do `env.setdefault` do script, com o número no comentário.
