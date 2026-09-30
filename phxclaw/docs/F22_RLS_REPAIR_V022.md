# Correção v0.22 — migration F22/0021

A v0.21 continha um defeito de quoting na geração dinâmica das policies RLS de `0021_device_nodes.sql`. A expressão `NULLIF(current_setting(...), '')` ficou mal formada dentro de `format(...)`.

A v0.22 traz `patches/migrations/0021_device_nodes.sql.fixed` com policies explícitas e sintaticamente claras. Como a migration original é transacional, a forma defeituosa deve falhar antes do `COMMIT`; ainda assim, o aplicador v0.22 só substitui automaticamente o arquivo se o SHA-256 for exatamente o snapshot defeituoso conhecido da v0.21. Qualquer divergência é recusada para revisão manual.

## Reparo Rust F22

A revisão v0.22 também detectou leitura de `DeviceState` e `RiskLevel` por `matches!` a partir de structs emprestadas. A correção adiciona `Copy` apenas a esses enums escalares. O aplicador usa SHA-256 do snapshot v0.21 e recusa sobrescrever fonte divergente.
