---
description: "Identidade SPIFFE assinada por papel e o atestado do que a maquina realmente expoe (que nao e attestation)."
argument-hint: "[atestado|emitir --papel X --chave-privada ARQ|conferir ARQ]"
allowed-tools: "Read, Glob, Grep, Bash"
---

# Identidade e atestado

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/identidade.py" \
  --project-root . "${1:-atestado}"
```

**Identidade**: cada papel ganha um `spiffe://dominio/projeto/X/agente/qa`
assinado com a mesma RSA do serial, com validade curta. Quem recebe confere com
a chave pública; papel adulterado quebra a assinatura.

**Atestado**: lê o que o sistema expõe — TPM presente, Secure Boot, flags de
CPU confidencial, virtualização, contêiner — e cada um vira `sim`, `não` ou
**INDISPONÍVEL**. O documento diz, em letras, que **isto não é attestation**:
attestation exige uma *quote* assinada pelo próprio chip e um verificador
remoto, e nada disso acontece aqui. Um campo `attested: true` sem quote é a
mentira mais cara desta lista, porque é a que alguém leva para auditoria.
