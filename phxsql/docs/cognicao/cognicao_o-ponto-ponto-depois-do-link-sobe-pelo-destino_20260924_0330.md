# O `..` depois de um link sobe pelo destino, e não pelo nome

**Data:** 24/09/2026, 03:30 (UTC) · **Frente:** pedido 372, segunda rodada
(revisão SEC, achado M1) · **Papel:** B.

## 1. O que aconteceu

A primeira rodada do 372 recusava a chave mestra que caísse dentro da pasta do
banco «pelo caminho REAL», e a função `caminho_real` (`config.rs`) fazia duas
coisas nesta ordem: resolvia `.` e `..` **pelo texto** e depois pedia ao
sistema o `canonicalize` do ancestral mais longo que existisse. O `chave()`
lia o caminho **cru** declarado.

A revisão SEC montou `fora/link -> <pasta do banco>/sub` e declarou
`fora/link/../chave.hex`. Pelo texto, `link/..` se anula e sobra
`fora/chave.hex` — fora da pasta, aceito. O kernel segue o link primeiro e sobe
a partir do **destino**: `<pasta do banco>/chave.hex`. O `cat` leu a chave de
dentro.

## 2. O que eu concluí primeiro, e estava errado

«Normalizar o texto antes deixa o `canonicalize` mais robusto quando o arquivo
ainda não existe.» Estava errado: resolver `..` pelo texto só é equivalente ao
kernel quando nenhum componente anterior é link — e a conferência existe
justamente para o caso em que alguém pôs um link no caminho. O comentário da
função dizia «links resolvidos», e resolvia os links do caminho **errado**.

## 3. O que a medição disse

- Reposto o defeito (o `..` pelo texto primeiro), o `Config::ler` aceita:
  `a conferencia viu Some(".../fora/chave.hex"), e o kernel abre esse caminho
  DENTRO da pasta do banco`.
- O mesmo vermelho mostrou uma segunda barreira que não estava prevista:
  `a leitura devolveu a chave: false`. A leitura passou a usar o caminho
  **conferido** (e não o declarado), que com o defeito aponta para
  `fora/chave.hex`, que não existe. A chave de dentro não foi lida, mas a
  declaração foi aceita — e a cifra anunciava uma proteção que não tinha.
- A segunda barreira tem guarda própria (372.11): um diretório trocado por
  link **depois** da conferência. Com a leitura sem refazer o caminho, o
  vermelho é `a chave foi lida de DENTRO da pasta do banco`.

## 4. A regra

**Caminho que decide segurança vai inteiro ao sistema operacional antes de o
texto mexer nele — o texto só resolve o sufixo que ainda não existe —, e quem
lê, lê o caminho que foi conferido, refeito na hora.**

## 5. Como está guardado hoje

Nas guardas 372.10 (o `..` pelo texto) e 372.11 (a leitura que não refaz o
caminho), com os testes `a_chave_por_link_seguido_de_ponto_ponto_e_recusada` e
`a_chave_nao_se_le_de_um_caminho_que_mudou_depois_da_conferencia`, provados
contra o sistema (link de verdade, `#[cfg(unix)]`). **O buraco que fica**: a
conferência compara caminhos, e não arquivos — *bind mount* e link físico
passam por ela; está escrito no FORMATO §19, no MANUAL 15.8 e no DBLINK.md. E
há outro `canonicalize` na casa que merece o mesmo olhar: não varri.
