# Vídeo de uso

`wx-claude-code-video-de-uso.webm` (1280×720, VP8, 3 min 38 s): vinte e nove
cenas, da instalação ao resultado em Rust, cada uma reproduzindo uma saída real
de sessão do Claude Code ou de script do
plugin, as mesmas capturas dos prints em `../prints/`. As quatro últimas
fecham o arco que dá sentido ao resto: a bateria pesada de cenários, a
procedure WLanguage lida do PDF do legado, o Rust gerado por uma sessão real
citando a página de origem dentro do código, e o `cargo test` que prova a
regra — com o que mudou de semântica dito, não escondido. O comando aparece
digitado, a saída aparece linha a linha, e nada foi inventado ou editado.

O `.mp4` (H.264, mesmo conteúdo) foi convertido do WebM com um ffmpeg estático com libx264 (`imageio-ffmpeg`).

Para regravar: `node gravar-video.mjs <pasta-de-saida> <pasta-das-capturas>`.
O script usa o Playwright do ambiente e grava em WebM porque é o único codec
de vídeo que o ffmpeg do Playwright codifica; para MP4, converta com um ffmpeg
que tenha libx264.

## Segundo vídeo: de PHP para Rust

`wx-claude-code-video-php.webm` (1 min 26 s, 11 cenas) mostra o outro caminho,
o que prova a parte **E/OU** da regra do legado: um sistema PHP procedural de
2009, sem nada de WINDEV, atravessando o plugin inteiro — instalação, o serial
recusando e depois liberando, o questionário, o portão G0 aceitando um projeto
sem um único PDF, a conversão para Rust por uma sessão real, o `cargo test`
contra o golden master capturado do próprio legado, a entrega com hashes e o
registro de operações.

Os dois roteiros vivem no mesmo gerador:

```bash
node gravar-video.mjs <pasta-saida> <pasta-capturas>        # roteiro 'uso'
node gravar-video.mjs <pasta-saida> <pasta-capturas> php    # roteiro 'php'
```
