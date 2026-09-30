# v0.61 gates

1. DENY não percorre/copia corpus.
2. QUARANTINE gera zero candidatos reutilizáveis.
3. ALLOW + segredo => QUARANTINE.
4. ALLOW limpo => armazenamento content-addressed + candidato RawObservation.
5. Symlinks não são seguidos.
6. Limites de arquivo/volume são fail-closed.
7. Evidence Ledger recebe o resultado.
8. PostgreSQL bloqueia candidatos de runs diferentes de ALLOW.
9. Receipts de harvest são imutáveis.
