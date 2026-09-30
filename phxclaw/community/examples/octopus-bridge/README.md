# PhxClaw Octopus Bridge

Este exemplo encapsula o Phoenix Octopus como um plugin externo do PhxClaw.

- Não copia código proprietário do Octopus para o core Apache-2.0.
- Só expõe subcomandos allowlisted.
- Não executa shell arbitrário.
- Usa o Process Protocol + Sandbox + Evidence Ledger.
- O manifesto é um template: build, hash, assinatura Ed25519 e pacote devem ser gerados antes da instalação.
