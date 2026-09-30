# PhxClaw v0.57 — Self-Evolving Intelligence & Continuous Learning

Esta versão transforma cada execução do PhxClaw em experiência governada para melhorar decisões futuras sem permitir auto-modificação irrestrita do core.

## Ciclo
`USE → OBSERVE → MEASURE → LEARN → HYPOTHESIS → EXPERIMENT → VERIFY → PROMOTE → MONITOR/ROLLBACK`.

## Autonomia padrão
**L2 / Auto-Tune**: model routing, context budget, cache e prompts reversíveis podem ser ajustados dentro de políticas já aprovadas. Skills e workflows continuam Promotion Gate. Alterações de código usam worktree/sandbox e geram proposta. Microkernel, Constitution, Security Policy, Secret Broker, Promotion Gate e Release Policy nunca têm auto-merge.

## Bases de conhecimento
- **Frutífera**: padrões comprovados, promovidos e frescos; podem ser fortemente preferidos quando o contexto é compatível.
- **Infrutífera**: failure signature, root cause, remediation e safe retry. Só bloqueia uma rota quando está governada, fresca e compatível com o Context Fingerprint.

## Integração v0.56
A v0.57 pode consumir as evidências da campanha nativa, mas **não** pode marcar sprint verde. O Sprint Gate Reconciler da v0.56 continua sendo a autoridade.

## Release gates pendentes neste host
`cargo`, `rustc`, PostgreSQL real/RLS, Ollama real, experiments E2E e self-improvement worktree E2E continuam não comprovados aqui.

## Capabilities novas
- `learning.experience.capture`
- `learning.experience.append`
- `learning.experience.read`
- `learning.experience.query`
- `learning.experience.summarize`
- `learning.experience.cost.record`
- `learning.experience.quality.record`
- `learning.experience.evidence.bind`
- `learning.experience.outcome.classify`
- `learning.experience.lineage.trace`
- `learning.context.fingerprint.create`
- `learning.context.fingerprint.compare`
- `learning.context.fingerprint.validate`
- `learning.context.compatibility.score`
- `learning.context.source_state.bind`
- `learning.context.environment.bind`
- `learning.context.task_class.bind`
- `learning.context.risk.bind`
- `learning.fruitful.pattern.create`
- `learning.fruitful.pattern.query`
- `learning.fruitful.pattern.reuse`
- `learning.fruitful.pattern.confidence`
- `learning.fruitful.pattern.freshness`
- `learning.fruitful.pattern.promote`
- `learning.fruitful.pattern.revoke`
- `learning.fruitful.pattern.explain`
- `learning.unfruitful.pattern.create`
- `learning.unfruitful.pattern.query`
- `learning.unfruitful.pattern.guard`
- `learning.unfruitful.pattern.remediation`
- `learning.unfruitful.pattern.retry_conditions`
- `learning.unfruitful.pattern.freshness`
- `learning.unfruitful.pattern.promote`
- `learning.unfruitful.pattern.explain`
- `learning.pattern.mine`
- `learning.pattern.cluster`
- `learning.pattern.support.count`
- `learning.pattern.candidate.create`
- `learning.pattern.candidate.rank`
- `learning.pattern.candidate.explain`
- `learning.hypothesis.create`
- `learning.hypothesis.criteria.define`
- `learning.hypothesis.experiment.link`
- `learning.hypothesis.evidence.link`
- `learning.hypothesis.validate`
- `learning.hypothesis.reject`
- `learning.experiment.create`
- `learning.experiment.sandbox.prepare`
- `learning.experiment.run`
- `learning.experiment.compare`
- `learning.experiment.benchmark`
- `learning.experiment.security.check`
- `learning.experiment.rollback`
- `learning.experiment.evidence.emit`
- `learning.prompt.variant.create`
- `learning.prompt.compile`
- `learning.prompt.arena.run`
- `learning.prompt.compare`
- `learning.prompt.champion.select`
- `learning.prompt.rollback`
- `learning.prompt.evidence.emit`
- `learning.skill.candidate.create`
- `learning.skill.candidate.test`
- `learning.skill.candidate.benchmark`
- `learning.skill.candidate.security`
- `learning.skill.promotion.request`
- `learning.skill.rollback`
- `learning.skill.lineage.trace`
- `learning.workflow.candidate.create`
- `learning.workflow.candidate.simulate`
- `learning.workflow.candidate.test`
- `learning.workflow.candidate.compare`
- `learning.workflow.promotion.request`
- `learning.workflow.rollback`
- `learning.workflow.lineage.trace`
- `learning.model.outcome.record`
- `learning.model.affinity.update`
- `learning.model.exploration.plan`
- `learning.model.champion.select`
- `learning.model.drift.detect`
- `learning.model.cost.optimize`
- `learning.freshness.evaluate`
- `learning.freshness.decay`
- `learning.freshness.stale.mark`
- `learning.freshness.revalidate`
- `learning.freshness.expire`
- `learning.contradiction.detect`
- `learning.contradiction.open`
- `learning.contradiction.resolve`
- `learning.contradiction.evidence.compare`
- `learning.contradiction.block_promotion`
- `learning.self_improvement.hypothesis.create`
- `learning.self_improvement.worktree.create`
- `learning.self_improvement.change.plan`
- `learning.self_improvement.change.apply_sandbox`
- `learning.self_improvement.test.run`
- `learning.self_improvement.promotion.request`
- `learning.self_improvement.rollback`
