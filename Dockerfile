# PhxSql em contêiner.
#
# Duas etapas: uma compila, a outra só carrega o binário. A segunda pode ser
# `scratch` porque o PhxSql **não tem dependência externa nenhuma** — nem
# crate, nem biblioteca de sistema. É a mesma decisão que fez a compilação
# cruzada para Windows funcionar de primeira, cobrando o dividendo aqui: a
# imagem final não tem shell, não tem gerenciador de pacotes e não tem
# superfície de ataque que não seja o próprio servidor.
#
#   docker build -t phxsql .
#   docker run -p 5000:5000 -p 5001:5001 -v $PWD/dados:/dados phxsql
#
# O `config.json` mora no volume, junto com os dados: trocar configuração não
# reconstrói imagem.

# ---------------------------------------------------------------- compilar
FROM rust:1.83-slim AS construtor
WORKDIR /fonte
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
# `--offline` prova o que a imagem promete: se alguma dependência externa
# tivesse entrado sem ninguém notar, a compilação pararia aqui.
RUN cargo build --release --offline --workspace \
 && strip target/release/phxsqld target/release/phxsql

# --------------------------------------------------------------- executar
FROM scratch
COPY --from=construtor /fonte/target/release/phxsqld /phxsqld
COPY --from=construtor /fonte/target/release/phxsql  /phxsql
# O `config.json` de exemplo, para o contêiner subir mesmo sem volume montado.
COPY exemplos/Config_docker.json /padrao/config.json

WORKDIR /dados
VOLUME ["/dados"]
EXPOSE 5000 5001

# Sem shell na imagem, o ENTRYPOINT é o binário direto. Ele procura
# `config.json` no diretório de trabalho, que é o volume.
ENTRYPOINT ["/phxsqld"]
