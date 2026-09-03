# PhxSql em conteiner.
#
# A imagem final e `scratch` -- sem shell, sem gerenciador de pacotes, sem
# nada alem do binario. Isso so e possivel porque o PhxSql **nao tem
# dependencia externa nenhuma**: nem crate, nem biblioteca de sistema. E o
# dividendo da regra que fez a compilacao cruzada para Windows funcionar de
# primeira, cobrado aqui.
#
# Medido neste projeto:
#
#     alvo gnu   (padrao) ... dinamico: libc.so.6, libgcc_s.so.1, ld-linux
#     alvo musl  .......... static-pie
#
# O tamanho do binario musl NAO esta aqui de proposito: o "3,4 MB o servidor
# + 1,2 MB o cliente" que este comentario tinha ficou parado enquanto o
# binario crescia, e so foi pego porque um relatorio a parte mediu o mesmo
# binario com `strip` em 7,66 MB (medido em 02/09/2026, ver
# docs/dossie/relatorio-conteineres.html e docs/PENDENCIAS.md #167). Numero
# que nao sai de gerador nenhum envelhece calado.
#
# O do cliente FOI remedido em 03/09/2026, junto de uma reconferencia do
# servidor: `phxsqld` 7.664.416 bytes (7,66 MB) -- o MESMO valor de 02/09, um
# dia depois, por medicao independente -- e `phxsql` 1.331.928 bytes (1,33 MB),
# contra o "1,2 MB" que estava aqui. O binario ja sai `stripped` do perfil de
# release, entao o `strip` da linha abaixo e um no-op que se mantem por
# clareza.
#
# E fica a armadilha que a reconferencia pagou: 7,66 MB (10^6) e 7,31 MiB
# (2^20) sao O MESMO ARQUIVO. Ao remedir, o primeiro numero pareceu desmentir
# o documento, e nao desmentia -- era `du`/`awk` em MiB contra um documento em
# MB. Numero sem unidade e numero pela metade.
#
# Para o tamanho de hoje:
# `ls -la target/x86_64-unknown-linux-musl/release/{phxsqld,phxsql}` depois
# do `strip` deste Dockerfile.
#
# Por isso o alvo e musl e nao o padrao: com o padrao, `FROM scratch` nao sobe
# -- falta o carregador dinamico dentro da imagem.
#
#   docker build -t phxsql .
#   docker run -p 5000:5000 -p 5001:5001 -v $PWD/dados:/dados phxsql
#
# O `config.json` mora no volume, junto com os dados: trocar configuracao nao
# reconstroi imagem. Para gerar um hash de senha:
#
#   docker run --rm phxsql --senha
#
# ---------------------------------------------------------------- compilar
FROM rust:1.83-slim AS construtor
RUN rustup target add x86_64-unknown-linux-musl
WORKDIR /fonte
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
# `--offline` prova o que a imagem promete: se alguma dependencia externa
# tivesse entrado sem ninguem notar, a compilacao pararia aqui em vez de sair
# buscando na rede.
RUN cargo build --release --offline --workspace --target x86_64-unknown-linux-musl \
 && strip target/x86_64-unknown-linux-musl/release/phxsqld \
          target/x86_64-unknown-linux-musl/release/phxsql

# --------------------------------------------------------------- executar
FROM scratch
COPY --from=construtor /fonte/target/x86_64-unknown-linux-musl/release/phxsqld /phxsqld
COPY --from=construtor /fonte/target/x86_64-unknown-linux-musl/release/phxsql  /phxsql
# Modelo de configuracao, para quem quiser copiar de dentro da imagem.
COPY exemplos/Config_docker.json /modelos/config.json
COPY exemplos/Config_docker_replica.json /modelos/config-replica.json

WORKDIR /dados
VOLUME ["/dados"]
EXPOSE 5000 5001

# Sem shell na imagem, o ENTRYPOINT e o binario direto. Ele procura
# `config.json` no diretorio de trabalho, que e o volume.
ENTRYPOINT ["/phxsqld"]
