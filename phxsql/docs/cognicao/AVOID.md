# AVOID -- o que ja falhou aqui, e por que

<!-- GERADO por docs/cognicao/avoid-e-reuse.py -- NAO EDITE A MAO.
     `--catraca` reprova se este arquivo nao bater com o que o extrator
     geraria agora; rode o comando sem flag para atualizar. -->

Gerado dos `cognicao_*.md` com `**Estado:** INFRUTIFERO` -- 1 hoje, de 306 cognicoes no total.

## Juiz em markdown acerta a resposta e foge do formato: limiar e formato vão para código

- Causa: a primeira versão do PhxJev deixava ao próprio juiz (o agente, lendo a skill) calcular a confiança, aplicar os limiares e escrever o bloco de saída; instrução em prosa não amarra formato nem aritmética.
- Prevencao: o juiz só emite probabilidades em JSON; confiança, limiar, veredito e registro saem de `plugins/phxjev/scripts/phxjev.py`, que recusa JSON inválido e é provado por `plugins/phxjev/scripts/teste_phxjev.py`.
- Arquivo: [cognicao_juiz-em-markdown-foge-do-formato-limiar-vai-para-codigo_20260924_1040.md](cognicao_juiz-em-markdown-foge-do-formato-limiar-vai-para-codigo_20260924_1040.md)
