# F24 — Auto-learning + Skill Evolution (v0.22)

## Regra central

PhxClaw pode **aprender e propor**, mas aprendizado não ganha autoridade de enforcement. A fronteira é:

```text
RAW OBSERVATION
  -> UNVERIFIED CONTEXT
  -> ACCEPTED EVIDENCE
  -> SKILL CANDIDATE
  -> EVALUATION
  -> PROMOTION GATE
  -> STAGED/CANARY
  -> ACTIVE SKILL
```

Nunca existe transição autônoma de observação/memória para Constitution, microkernel ou policy.

## Promotion Gate

Um candidato fica elegível apenas quando:

- alvo é `skill_plugin` em namespace `skills.*`;
- artifact/manifest/source-state são imutavelmente vinculados por SHA-256;
- evidência pertence exatamente ao mesmo artifact e source state;
- evidência está fresca;
- há ao menos dois avaliadores independentes;
- suítes obrigatórias têm zero falhas;
- zero achados de segurança HIGH/CRITICAL;
- benchmark não regride além da política;
- atualização possui caminho de rollback.

Auto-promotion é permitida somente para skill de baixo risco, reversível e sem mudança comportamental. Todo o restante exige aprovação humana vinculada ao digest do candidato.

## Integração

Research Core produz evidências com proveniência. Hypothesis Core transforma lacunas em hipóteses testáveis. F24 só sintetiza skill após evidência aceita e publica via Plugin Registry/Skill Runtime. Event Bus e Evidence Ledger registram avaliação/promoção/rollback. F17 fornece checkpoints e rollback.
