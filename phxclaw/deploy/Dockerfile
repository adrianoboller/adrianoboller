# syntax=docker/dockerfile:1
# PhxClaw em contêiner: build multi-estágio, imagem final mínima e sem segredo.
#
# Construir, da RAIZ do repositório (o contexto é a raiz, não deploy/):
#   docker build -f deploy/Dockerfile -t phxclaw:0.70.0 .
#
# Nenhuma credencial entra na imagem: nem em ENV, nem em ARG, nem em COPY. O token da API
# e a chave-mestra do cofre chegam em tempo de execução, por arquivo (volume ou secret).

# ---- estágio 1: compilação --------------------------------------------------------------
# A versão é a do rust-toolchain.toml da raiz (1.98.1); manter as duas iguais é o que faz o
# rustup NÃO baixar outra toolchain dentro do build.
FROM rust:1.98.1-bookworm AS build
WORKDIR /src

# cmake: o aws-lc-sys (rustls) pode pedi-lo; pkg-config: crates de sistema.
RUN apt-get update \
 && apt-get install -y --no-install-recommends cmake pkg-config \
 && rm -rf /var/lib/apt/lists/*

COPY . .
# --no-default-features: tira a feature `desktop` (mouse, teclado e captura de tela), que
# liga libgbm, libdrm, libwayland e libxcb no executável. Servidor não tem sessão gráfica.
RUN cargo build --release --locked --no-default-features -p phxclaw \
 && strip target/release/phxclaw

# ---- estágio 2: execução ----------------------------------------------------------------
FROM debian:bookworm-slim AS final

# curl: só para o HEALTHCHECK (a imagem distroless não tem cliente HTTP, e acrescentar um
# subcomando ao binário só para isto seria código de produto por causa da imagem).
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl \
 && rm -rf /var/lib/apt/lists/* \
 && groupadd --system --gid 10001 phxclaw \
 && useradd --system --uid 10001 --gid 10001 --home-dir /data --shell /usr/sbin/nologin phxclaw \
 && mkdir -p /data/segredos \
 && chown -R 10001:10001 /data
# /data/segredos já nasce do dono certo: o compose monta a chave-mestra DENTRO dele, e se o
# contêiner o criasse (como root) o cofre não conseguiria gravar o cofre/ ao lado da chave.

COPY --from=build /src/target/release/phxclaw /usr/local/bin/phxclaw

# Não é segredo: é onde o agente guarda tarefas, config.json, cofre e o api.token gerado.
ENV PHXCLAW_HOME=/data \
    HOME=/data \
    PHXCLAW_API_HOST=0.0.0.0
# Escutar em 0.0.0.0 só é seguro porque a API SEMPRE exige Bearer: sem token fornecido, o
# `servir` gera um em /data/api.token (0600). Quem expõe a porta fora da rede do contêiner
# decide isso por escrito (compose: `ports`; k8s: Service).

USER 10001:10001
WORKDIR /data
VOLUME ["/data"]
EXPOSE 8787

# /health não exige credencial e não toca o disco.
HEALTHCHECK --interval=30s --timeout=3s --start-period=20s --retries=3 \
  CMD curl -fsS http://127.0.0.1:8787/health >/dev/null || exit 1

ENTRYPOINT ["phxclaw"]
CMD ["servir", "--porta", "8787"]
