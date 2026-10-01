# Segurança do Gateway v0.6

## Padrão seguro

O modo `profile_id` é o padrão.

O arquivo de profile contém `password_env`, não a senha:

```json
{
  "id": "prod-readonly",
  "dialect": "postgresql",
  "host": "db.internal",
  "database": "app",
  "username": "schema_reader",
  "password_env": "PHX_PROD_DB_PASSWORD",
  "tls": "require"
}
```

A variável é resolvida somente no processo Rust.

## Garantias implementadas

- `ConnectionSpec::Debug` mascara password;
- `safe_source_name()` nunca contém usuário ou senha;
- endpoint `/profiles` devolve somente metadados não secretos;
- `/api/*` não é armazenado pelo Service Worker;
- conexão efêmera desabilitada sem `PHX_ALLOW_EPHEMERAL=1`;
- bind padrão somente em `127.0.0.1:8787`;
- gateway não registra corpo do request;
- introspecção deve usar usuário somente-leitura.

## Produção/remoto

Antes de expor o Gateway fora do host local, adicionar:

- HTTPS obrigatório no reverse proxy;
- autenticação do usuário;
- autorização por profile;
- rate limiting;
- rotação/secret manager;
- allowlist de hosts/bancos;
- timeout de conexão e query;
- auditoria sem segredos;
- limites por quantidade de objetos;
- bloqueio de redes/endpoints não autorizados para evitar SSRF.

v0.6 é **local-first**. Não trate o binário padrão como gateway público de Internet.
