# A imagem da bancada de replicacao em conteiner.
#
# `FROM scratch` -- sem shell, sem gerenciador de pacotes, sem libc solta: so o
# binario. Isso so e possivel porque o PhxSql nao tem dependencia externa
# nenhuma, e e a mesma decisao da imagem oficial (`phxsql/Dockerfile`).
#
# Tamanho, medido de tres jeitos porque os tres discordam: 6,42 MB de camada
# (`docker history`), 2,69 MB comprimidos para baixar (`docker image inspect`)
# e 9,11 MB no `docker images`, que conta o manifesto de atestacao do
# BuildKit. O `provar.py` publica os tres.
#
# A DIFERENCA para a oficial e de proposito: a oficial compila DENTRO do
# conteiner (`FROM rust:1.83-slim`), o que prova o `--offline` mas leva minutos
# e baixa ~1,5 GB. Esta bancada sobe e derruba mais de dez conteineres em cada
# corrida, entao o binario vem do `cargo build --release --target
# x86_64-unknown-linux-musl` da maquina -- MESMO alvo, MESMO `scratch`, mesma
# ausencia de carregador dinamico. O `provar.py` compila antes de construir.
#
# O contexto e um diretorio de estagio com um arquivo so, para nao mandar o
# `target/` inteiro ao daemon.
FROM scratch
COPY phxsqld /phxsqld
WORKDIR /dados
EXPOSE 5000
# Sem shell, o ENTRYPOINT e o binario. Ele le `config.json` do diretorio de
# trabalho, que e o volume montado.
ENTRYPOINT ["/phxsqld"]
