# O vídeo de demonstração

Cinco minutos gravados **contra o servidor de verdade** — não é maquete, não é
mockup. O Playwright dirige a interface, a legenda é injetada na própria página
(por isso ela entra no vídeo sem edição depois), e o ffmpeg converte para MP4.

```bash
cargo build --release
node docs/video/roteiro.mjs        # grava em bruto/ (WebM)
./docs/video/converter.sh          # converte para MP4
```

O roteiro espera **dois servidores** no ar: um master em `127.0.0.1:5900`
(web em 8900, `papel: source`, `imagem_da_linha: true`) e uma réplica em
`127.0.0.1:5901` (web em 8901, `papel: replica`, `somente_leitura: true`,
puxando do master). O `bancada/replicacao/montar.py` monta um par igual, só
com outras portas.

## Os dezessete capítulos

| | |
|---|---|
| 01 | Entrar — e a senha que não trafega |
| 02–03 | Criar o database e a tabela, com índices e chave primária |
| 04 | Inserir pela ficha |
| 05 | Carga em lote: colar CSV, conferir, gravar |
| 06–07 | Vinte mil linhas, paginação por cursor e o salto por posição |
| 08 | Alterar |
| 09–10 | As duas exclusões, o `.reason` e a lixeira |
| 11 | Consultar — e a parte honesta: **não há SQL** |
| 12–13 | Integridade e backup |
| 14–15 | Replicação: o master, e a réplica com a mesma contagem |
| 16 | **O que ainda falta** — a parte que vídeo de produto não mostra |
| 17 | O painel |

## Por que uma cena que falha não derruba o vídeo

Cada capítulo roda dentro de `cena()`, que captura o erro, escreve uma linha no
log e segue. A primeira gravação se perdeu inteira porque um passo falhou no
meio; com isso, o pior caso é um capítulo faltando em vez de nada.

## O vídeo achou três defeitos

Vale registrar, porque foi o melhor argumento a favor de gravá-lo:

1. **A ficha mandava 8 valores para 9 colunas.** Todo salvar e todo incluir
   pela interface falhavam desde que o `rownum` entrou. O erro aparecia no
   canto da tela num quadro do capítulo 9.
2. **A tela da Replicação dizia que a replicação não existia** — texto
   verdadeiro na 0.14.0 e falso na 0.15.0.
3. **Essa mesma tela lia o campo errado** da resposta de `bancos`, e dizia
   «nenhuma tabela ainda» numa réplica que tinha a tabela na árvore ao lado.

Nenhum dos três apareceria lendo o código, e os três apareceram em cinco
minutos de tela.
