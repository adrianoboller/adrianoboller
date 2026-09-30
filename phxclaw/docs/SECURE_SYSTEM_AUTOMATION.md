# PhxClaw — System Automation Security Model

O PhxClaw pode executar comandos Windows/Linux, controlar input e capturar a tela, mas essas funções não ficam abertas por padrão.

## Regras

1. `system.command.execute`, `desktop.input` e `screen.capture` são capabilities privilegiadas.
2. Estado padrão: **desabilitado**.
3. Somente o Desktop Host confiável recebe acesso ao SO do usuário.
4. Plugins normais continuam em sandbox e pedem operações ao host via contrato tipado.
5. Toda execução registra actor, task UUIDv7, capability, target, policy decision, timestamps e resultado no capability audit.
6. Timeout obrigatório para processos.
7. stdout/stderr/exit code fazem parte da evidência de execução.
8. Shells suportados são explícitos: `direct`, `cmd/msdos`, `powershell`, `sh`, `bash`.
9. Operações de publicação, exclusão irreversível, elevação ou mudança de infraestrutura podem exigir approval gate humano.
10. Credenciais não são injetadas automaticamente no ambiente de processos.

## Princípio

“Controle total” significa que o produto terá os drivers necessários quando o usuário autorizar; não significa que agentes recebam privilégios ilimitados por padrão.
