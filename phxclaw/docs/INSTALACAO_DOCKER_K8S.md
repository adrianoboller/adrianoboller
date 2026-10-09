# Instalação em Docker e Kubernetes

Fecha o gap `instalacao_docker_k8s` do n8n (decisão do dono, 09/10/2026). Tudo em `deploy/`.

| Arquivo | O que é |
|---|---|
| `deploy/Dockerfile` | multi-estágio: `rust:1.98.1-bookworm` compila (`--locked --no-default-features`), `debian:bookworm-slim` executa; usuário 10001, `HEALTHCHECK`, `EXPOSE 8787`, `VOLUME /data` |
| `deploy/Dockerfile.dockerignore` | o ignore que o Docker **lê** (ver abaixo): `target/`, `.git`, `.env*`, `*.key`/`*.pem`, `master.key`, `segredos/`, `secrets/`, `var/` fora do contexto |
| `deploy/docker-compose.yml` | agente + PostgreSQL opcional (`--profile postgres`); segredos por `secrets:` de arquivo |
| `deploy/k8s/` | Namespace, ConfigMap, PVC, Deployment, Service, NetworkPolicy, `secret-exemplo.yaml` (sem valor) e `kustomization.yaml` |

## Docker Compose

```bash
mkdir -p deploy/secrets && chmod 700 deploy/secrets
openssl rand -hex 32    > deploy/secrets/api_token     # Bearer, 24+ caracteres
openssl rand -base64 32 > deploy/secrets/master_key    # chave-mestra do cofre (32 bytes em base64)
chmod 444 deploy/secrets/*                              # o contêiner lê como 10001
docker compose -f deploy/docker-compose.yml up -d --build
curl -fsS http://127.0.0.1:8787/health                  # {"ok":true}
```

`deploy/secrets/` é ignorado pelo git e pelo ignore do build, **e o Dockerfile nem o copia** (ver «O contexto do build»). O compose publica a porta só em
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

A `NetworkPolicy` (`deploy/k8s/networkpolicy.yaml`) deixa entrar na 8787 **só** o namespace `ingress-nginx` — troque o rótulo
`kubernetes.io/metadata.name` pelo do seu Ingress — e deixa o **egresso aberto de propósito**: o agente sai para APIs
de modelo, canais e web, e uma lista de IPs aqui quebraria isso calada. Quem quiser fechar acrescenta `Egress` em
`policyTypes`. Exige CNI que aplique NetworkPolicy; sem ela o objeto é aceito e ignorado.

O Secret **não está** no `kustomization.yaml` de propósito: sem ele o pod fica em
`ContainerCreating`, que é a falha certa (melhor que subir com token vazio). O Service é
`ClusterIP`; expor fora do cluster, com TLS à frente (a API fala HTTP), é decisão do operador.

## O contexto do build: o que o Docker lê, e o que ele leva

O build é `docker build -f deploy/Dockerfile .` com contexto na **raiz**. Defeito achado na revisão
(09/10/2026): o ignore morava em `deploy/.dockerignore`, que **ninguém lê** — o Docker só procura o
`.dockerignore` da raiz do contexto ou, no BuildKit, `<Dockerfile>.dockerignore` ao lado do Dockerfile
(nome exato). Com o `COPY . .` de então, `deploy/secrets/` (token e chave-mestra), `var/`, `.git` e
`target/` (16 GB) entravam na camada de build.

- **Primeira defesa:** o Dockerfile copia uma **lista fechada** (`Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`,
  `crates/`, `apps/`, `config/`, `modelos/`, `plugins/`) — as três últimas são o que os `include_str!` embutem
  de fora dos crates, conferido varrendo todos os `.rs`. Segredo não entra nem que o ignore falhe. Um
  `include_str!` novo apontando para outra pasta quebra o build, alto; copiar tudo só quebraria no dia do vazamento.
- **Segunda defesa:** `deploy/Dockerfile.dockerignore`, **dentro de `deploy/`** e não na raiz: o nome é o que o
  BuildKit lê, não polui a raiz e viaja com o Dockerfile que descreve. (Raiz também valeria; `.dockerignore`
  na raiz do contexto serve a todo `docker build` do repositório, inclusive os que não são deste Dockerfile.)
  Vale para `docker compose` também (`build.dockerfile: deploy/Dockerfile`, BuildKit).
- O conferidor **recusa** `deploy/.dockerignore` (a isca) e exige o arquivo que o Docker lê.

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
de segredo e valor; o estágio final copiar o contexto ou arquivo de segredo; o ignore **que o Docker lê**
(`deploy/Dockerfile.dockerignore`, pelo nome que a regra do Docker usa) faltar, ou existir só um `deploy/.dockerignore`;
houver `COPY . .` sem esse ignore excluir `target/`, `.git`, `.env`, chaves, cofre, `deploy/secrets/` e `var/`;
um `COPY` levar `deploy/`, `var/` ou `.git` pelo nome; a NetworkPolicy faltar, liberar o ingresso a todos
(`from` ausente, `0.0.0.0/0`) ou não restringir a porta 8787; qualquer YAML trouxer
**forma de credencial** (a regra do motor único `phxclaw_types::segredo`, nada reescrito); o
Secret de exemplo tiver valor; o Deployment perder `runAsNonRoot`, `readOnlyRootFilesystem`,
`allowPrivilegeEscalation: false`, `drop: [ALL]`, sondas ou fixar `:latest`; o compose perder
`user`, `read_only`, `cap_drop`, porta no loopback ou o perfil do PostgreSQL. Cada conferência
tem o mutante que a prova (aplica UM defeito ao arquivo real e exige que seja nomeado).

## Limites declarados

- **Teste do canário: NÃO MEDIDO** (o cliente `docker` existe, mas não há daemon nesta máquina:
  `failed to connect to the docker API at unix:///var/run/docker.sock`). Medido no lugar disso, sem
  daemon: o conjunto que o `COPY` leva (14 MB) carrega o workspace (`cargo metadata --locked --offline`) e
  todo `include_str!` de código de produção resolve dentro dele. Comando para medir de verdade:
  ```bash
  mkdir -p deploy/secrets && echo x > deploy/secrets/canario
  docker build -f deploy/Dockerfile --target build -t t .
  docker run --rm t ls deploy/secrets      # tem de falhar: «No such file or directory»
  docker run --rm t ls -a /src             # nem .git, var, deploy, target
  ```

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
