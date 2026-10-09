# Instalação em Docker e Kubernetes

Fecha o gap `instalacao_docker_k8s` do n8n (decisão do dono, 09/10/2026). Tudo em `deploy/`.

| Arquivo | O que é |
|---|---|
| `deploy/Dockerfile` | multi-estágio: `rust:1.98.1-bookworm` compila (`--locked --no-default-features`), `debian:bookworm-slim` executa; usuário 10001, `HEALTHCHECK`, `EXPOSE 8787`, `VOLUME /data` |
| `deploy/.dockerignore` | `target/`, `.git`, `.env*`, `*.key`/`*.pem`, `master.key`, `segredos/`, `var/` fora do contexto |
| `deploy/docker-compose.yml` | agente + PostgreSQL opcional (`--profile postgres`); segredos por `secrets:` de arquivo |
| `deploy/k8s/` | Namespace, ConfigMap, PVC, Deployment, Service, `secret-exemplo.yaml` (sem valor) e `kustomization.yaml` |

## Docker Compose

```bash
mkdir -p deploy/secrets && chmod 700 deploy/secrets
openssl rand -hex 32    > deploy/secrets/api_token     # Bearer, 24+ caracteres
openssl rand -base64 32 > deploy/secrets/master_key    # chave-mestra do cofre (32 bytes em base64)
chmod 444 deploy/secrets/*                              # o contêiner lê como 10001
docker compose -f deploy/docker-compose.yml up -d --build
curl -fsS http://127.0.0.1:8787/health                  # {"ok":true}
```

`deploy/secrets/` é ignorado pelo git e pelo `.dockerignore`. O compose publica a porta só em
`127.0.0.1`. Perfil do PostgreSQL: crie também `deploy/secrets/pg_senha` e suba com
`--profile postgres`.

## Kubernetes

```bash
docker build -f deploy/Dockerfile -t phxclaw:0.70.0 .   # e publique no seu registro (ajuste `images:` do kustomization)
kubectl apply -f deploy/k8s/namespace.yaml
kubectl -n phxclaw create secret generic phxclaw-segredos \
  --from-literal=api-token="$(openssl rand -hex 32)" \
  --from-literal=master.key="$(openssl rand -base64 32)"
kubectl apply -k deploy/k8s
```

O Secret **não está** no `kustomization.yaml` de propósito: sem ele o pod fica em
`ContainerCreating`, que é a falha certa (melhor que subir com token vazio). O Service é
`ClusterIP`; expor fora do cluster, com TLS à frente (a API fala HTTP), é decisão do operador.

## Como o segredo chega, e por quê

- **Nada de segredo na imagem, em `ENV`, `ARG` ou manifesto.** O token da API e a chave-mestra
  do broker entram como **arquivo**: `/data/api.token` e `/data/segredos/master.key` — os
  caminhos que o agente já lê (`token_de` e `FileMasterKeyProvider`). Arquivo não aparece em
  `docker inspect`, `kubectl describe` nem `/proc/<pid>/environ`.
- Sem `api_token` fornecido, o `servir` **gera** um em `/data/api.token`: escutar em `0.0.0.0`
  dentro do contêiner nunca significa API aberta.
- `PHXCLAW_API_HOST=0.0.0.0` é necessário dentro do contêiner (o loopback dele não é alcançável
  de fora); o que limita a exposição é a publicação da porta (compose) e o Service (k8s).
- `/data/segredos` nasce do dono certo (imagem no Docker; `initContainer` no k8s): senão o
  runtime o criaria como root ao montar a chave-mestra e o cofre não conseguiria gravar ao lado.

## Conformidade

`cargo test -p phxclaw-types --test deploy_conformidade` lê `deploy/` e **falha** se: faltar
`USER` não-root na imagem final, `HEALTHCHECK`, `EXPOSE`, `VOLUME`; houver `ENV`/`ARG` com nome
de segredo e valor; o estágio final copiar o contexto ou arquivo de segredo; o `.dockerignore`
deixar de excluir `target/`, `.git`, `.env`, chaves, cofre ou `var/`; qualquer YAML trouxer
**forma de credencial** (a regra do motor único `phxclaw_types::segredo`, nada reescrito); o
Secret de exemplo tiver valor; o Deployment perder `runAsNonRoot`, `readOnlyRootFilesystem`,
`allowPrivilegeEscalation: false`, `drop: [ALL]`, sondas ou fixar `:latest`; o compose perder
`user`, `read_only`, `cap_drop`, porta no loopback ou o perfil do PostgreSQL. Cada conferência
tem o mutante que a prova (aplica UM defeito ao arquivo real e exige que seja nomeado).

## Limites declarados

- **Imagem e manifestos NÃO construídos/aplicados nesta máquina** (sem daemon Docker nem
  cluster): `docker compose config` valida a sintaxe do compose; o build é «NÃO MEDIDO».
  Comando: `docker build -f deploy/Dockerfile -t phxclaw:0.70.0 .` e, depois,
  `docker run --rm -p 127.0.0.1:8787:8787 phxclaw:0.70.0` + `curl /health`.
- O binário sai **sem** a feature `desktop` (mouse, teclado, captura de tela): servidor não tem
  sessão gráfica.
- **O terminal Helix do IDE web não abre no contêiner endurecido**: ele exige `bwrap` (user
  namespaces), que `drop: ALL` e o seccomp padrão não permitem — e o Helix nem está na imagem.
  O IDE web continua servindo símbolos, arquivo e completação.
- A ferramenta `db` (PostgreSQL) usa o driver `postgres` do Rust, que **não lê `.pgpass`**
  (o texto do catálogo diz o contrário); com `scram-sha-256` a senha tem de ir na
  `PHXCLAW_PG_URL`. O compose por isso **não define** essa variável: ponha-a você, fora do git.
- Uma réplica (volume `ReadWriteOnce`, cofre de um escritor).
