# Vídeo de uso

`wx-claude-code-video-de-uso.webm` (1280×720, VP8, 1 min 21 s): nove cenas, cada
uma reproduzindo uma saída real de sessão do Claude Code ou de script do
plugin, as mesmas capturas dos prints em `../prints/`. O comando aparece
digitado, a saída aparece linha a linha, e nada foi inventado ou editado.

Para regravar: `node gravar-video.mjs <pasta-de-saida> <pasta-das-capturas>`.
O script usa o Playwright do ambiente e grava em WebM porque é o único codec
de vídeo que o ffmpeg do Playwright codifica; para MP4, converta com um ffmpeg
que tenha libx264.
