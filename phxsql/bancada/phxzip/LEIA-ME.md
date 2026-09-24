# A bancada do PhxZip por plataforma-alvo

Prova, alvo a alvo, se o `phxzip`/`phxzipcmd` compila, liga e roda fora do
x86_64 Linux. **Nenhum número aqui é digitado**: `plataformas.sh` mede e grava
`resultados.json`; é dali que a tabela alvo × nivel de prova se gera.

```bash
export CARGO_TARGET_DIR=/algum/lugar   # ver a chamada real no pedido/rodada
bash bancada/phxzip/plataformas.sh
```

## Nivel de prova, do mais forte ao mais fraco

- **RODA** — o binario executou sob emulacao (ou nativamente) e o resultado
  bate: sha256 do arquivo extraido igual ao original, ou testes de unidade
  passando.
- **LIGA** — o binario existe (o ligador terminou), mas nao rodou aqui.
- **COMPILA** — so a biblioteca `phxzip` (rlib) foi compilada, sem ligador.
- **NAO MEDIDO** — nada foi medido, com o motivo escrito ao lado.

## O que precisa instalado (ferramenta de teste, no CONTAINER -- nunca no produto)

```bash
apt-get install -y qemu-user qemu-user-static gcc-arm-linux-gnueabihf \
  musl-tools gcc-s390x-linux-gnu gcc-mingw-w64-x86-64 wine64
rustup target add armv7-unknown-linux-musleabihf s390x-unknown-linux-gnu \
  x86_64-pc-windows-gnu aarch64-linux-android aarch64-apple-darwin \
  x86_64-apple-darwin aarch64-apple-ios
```

Android precisa do NDK r27c em `/opt/android-ndk-r27c` (o `.cargo/config.toml`
do repositorio ja aponta o ligador para la):

```bash
curl -sS -o /tmp/ndk.zip \
  https://dl.google.com/android/repository/android-ndk-r27c-linux.zip
unzip -q /tmp/ndk.zip -d /opt
```

macOS/iOS: sem o SDK da Apple neste container, so a biblioteca compila — ligar
um binario exige esse SDK, que nao se baixa sem licenca Apple.

ESP32 classico (Xtensa) e AVR: **NAO MEDIDO** de proposito — o Rust estavel
1.94.1 nao tem `std` pre-compilada para esses alvos (`rustup target add`
recusa), e portar exigiria `-Z build-std` (nightly) ou o toolchain proprio da
Espressif/`avr-rust`, fora do escopo desta bancada.

## Nenhum teste pulado

A primeira corrida pulou (`-- --skip`) um teste da `phxzip` que estava
vermelho por causa de uma frente em andamento no codificador. O integrador
tirou o pulo antes de versionar: prova de portabilidade que pula o teste
vermelho prova menos do que diz. Teste que falha sob emulação se confere no
x86_64 nativo antes de culpar o alvo.

## PhxZip × 7-Zip: tamanho e velocidade

`comparar-7z.py` mede o mesmo corpus nos dois, com trabalho igual:
- o 7-Zip roda com `-mf=off` e `-mmt=1`, ou seja, sem o filtro BCJ e num fio
  só, como o PhxZip;
- os dois descompactam para uma pasta.

São cinco corridas por medida, e o resultado guarda mediana, mínimo e
máximo. O arquivo do PhxZip só conta depois de o `7z t` e o `7z x` o abrirem
com o conteúdo igual. O resultado vai para `comparar-7z.json`, e o gráfico sai
dali, pelo `docs/dossie/graficos-dos-testes.py`.

```bash
cargo build --release -p phxzip-cmd
python3 bancada/phxzip/comparar-7z.py      # 5 corridas; passe outro número se quiser
python3 docs/dossie/graficos-dos-testes.py
```

## Arquivos

| arquivo | o que e |
|---|---|
| `plataformas.sh` | a medicao: builda, roda (qemu/wine/nativo) e grava `resultados.json` |
| `resultados.json` | tudo que foi medido, cru, com data e comando de cada alvo |
| `comparar-7z.py` | PhxZip × 7-Zip: tamanho, tempo de compactar e de descompactar |
| `comparar-7z.json` | o que ele mediu, com mediana/min/max, sha256 do corpus e data |
