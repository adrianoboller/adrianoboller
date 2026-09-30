# Dispositivos: pareamento e sessão

```bash
phxclaw dispositivos --cert srv.pem --chave srv.key --tokens tokens.txt [--porta 8788]
# tokens.txt: uma linha "tenant_uuid token" por token (24+ caracteres, uso único)

PHXCLAW_DEVICE_WSS_URL=wss://servidor:8788/ PHXCLAW_TENANT_UUID=… PHXCLAW_NODE_UUID=… \
PHXCLAW_DEVICE_CA_PEM=ca.pem PHXCLAW_ENROLLMENT_TOKEN=… phxclaw-device-node   # 1ª vez
# depois, sem o token: o nó entra com a chave Ed25519 guardada
```

| Mensagem | Quem pode | O que o servidor confere |
|---|---|---|
| `device.enroll` | quem tem token | assinatura com a chave do próprio pedido (prova de posse), token de uso único |
| `device.hello` | nó pareado | assinatura com a chave registrada; abre sessão e devolve a cerca |
| `device.heartbeat` | nó na sessão aberta | sessão, sequência crescente, nonce inédito |

Toda verificação passa por `phxclaw_device_nodes::verify_envelope`. Recusa responde
`device.rejected` com o motivo e fecha a conexão. TLS é obrigatório; o nó fixa a CA do
servidor (`PHXCLAW_DEVICE_CA_PEM`).

**Onde fica a chave do nó.** Chaveiro do sistema (Keychain, Credential Manager, Secret
Service). Máquina sem chaveiro (Linux sem sessão gráfica) usa
`PHXCLAW_DEVICE_KEYSTORE=arquivo:/pasta`: arquivo 0600 em pasta 0700, como OpenSSH, WireGuard
e Tailscale. Chaveiro que não persiste **recusa** em vez de fingir.

**Provado (30/09, Linux):** 4 testes pelo fio com TLS real (pareamento, sessão, heartbeat,
replay, token reusado, nó desconhecido, chave de outro, corpo adulterado, cliente sem TLS,
CA estranha), RED medido em 3 defeitos repostos; e entre processos: pareia, reinicia sem
token e entra com cerca 3. **Não provado:** Windows, macOS, Android, iOS (sem hardware aqui);
registro durável no PostgreSQL (o servidor local guarda em memória).
