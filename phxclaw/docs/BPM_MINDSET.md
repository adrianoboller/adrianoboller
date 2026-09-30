# PhxClaw — Mindset + BPM

## Mindset

O mindset é uma política operacional versionada, não um prompt solto. A configuração padrão v0.6 usa estes princípios:

- evidência antes de alegação;
- separar hipótese de fato;
- gates determinísticos para segurança/release;
- menor privilégio;
- rollback antes de mudanças perigosas;
- idempotência e rastreabilidade;
- aprovação humana onde a política exigir.

## BPM

O crate `phxclaw-bpm` importa um subconjunto inicial de BPMN 2.0 XML e modela:

- Start Event;
- End Event;
- Task;
- User Task;
- Service Task;
- Exclusive Gateway;
- Parallel Gateway;
- Sequence Flow;
- token de execução UUIDv7;
- validação de processo;
- ordenação topológica para grafos acíclicos.

As migrations `0008_bpm.sql` adicionam perfis de mindset, definições, instâncias, tokens e histórico.

A evolução planejada adiciona timers, boundary events, compensation/saga, subprocesses, message events e editor BPMN visual via `bpmn-js`.
