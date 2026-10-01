# A trava do núcleo é da descrição aberta, não do processo — e o filho entre `fork` e `exec` a herda

**Estado:** PENDENTE

## O que aconteceu

Pedido 635, a trava de instância (`trava_de_instancia.rs`): `File::try_lock`
sobre o `.phxsql.trava` de cada pasta, tomada no ponto único da abertura
gravável. A prova por processos reais (`tests/trava-de-instancia.rs`) sobe
três filhos em paralelo, e o teste do comportamento VELHO — um processo só
abrindo a mesma pasta várias vezes — foi recusado **pelo próprio processo**:
a recusa trazia o pid do pai.

## O que eu concluí primeiro, e estava errado

Três hipóteses, nesta ordem:

- **H1 (morta):** o registro do processo reaproveitava uma entrada viva com
  o arquivo errado. Lido: a entrada era `Weak` morta, e o arquivo novo se
  abria do zero.
- **H2 (morta):** o `Drop` da entrada ainda não tinha fechado o descritor.
  Morta por leitura: o `Arc` fecha no mesmo fio, antes de o `upgrade` falhar.
- **H3 (viva):** um filho de OUTRO teste, entre o `fork` e o `exec`, carrega
  cópia de todo descritor do pai — inclusive o da trava que o pai acabou de
  soltar. O `O_CLOEXEC` só fecha no `exec`; até lá a descrição aberta (e a
  trava, que é dela) continua viva no filho.

E antes disso, o erro do próprio teste: o pai lia a saída do filho esperando
uma linha que começasse com «SEGURANDO», e o harness escreve
«test papel_do_filho ... » **na mesma linha** — o teste travou duas vezes
antes de alguém ler a saída crua.

E o primeiro conserto também estava errado: **tentar de novo**, 10 vezes de
10 ms. Fechava a janela (0 de 30), mas a abertura gravável acontece com a
trava global de dados na mão, e o `portoes.sh` reprovou na catraca do mapa da
trava: `rede-ou-espera-2` subiu de **11 para 13**. Dormir sob a trava global
é exatamente o que aquela catraca existe para impedir.

## O que a medição disse

- Só fechando o descritor: **19 de 150** corridas da bateria falharam
  (a maioria recusando o próprio pid).
- Com `File::unlock` antes de fechar (`soltar_o_arquivo`): **0 de 150**. O
  `unlock` solta a descrição inteira, inclusive a cópia que o filho carrega
  entre o `fork` e o `exec`; não há espera nenhuma, e a catraca
  `rede-ou-espera-2` voltou a **11**.
- Defeito reposto (sem `tomar` no `abrir_com`): **3 de 5** testes caem; sem
  a fixação na raiz: **1 de 5** (o do servidor ocioso).
- O custo da escolha do meio: subir a versão mínima de 1.75 para 1.89 trouxe
  **19** avisos novos do `clippy`, todos mecânicos. A H2 da frente (arquivo de
  pid com prova de vida) morreu por deixar trava eterna no Windows e por
  corrida na tomada da trava velha; a H3 (`flock` por `extern "C"`), por
  `unsafe` num crate que já recusou FFI duas vezes.

## A regra

Trava do núcleo é da descrição aberta: solte-a com `unlock` antes de fechar,
porque um filho entre `fork` e `exec` carrega a descrição e a mantém travada —
e nunca conserte corrida dormindo sob a trava global.

## Como está guardado hoje

- `crates/phxsql-store/tests/trava-de-instancia.rs` (cinco testes, processos
  reais, `kill -9` incluído).
- Guardas `segundo-gravador-sem-trava-de-instancia-635` e
  `raiz-ociosa-solta-a-trava-de-instancia-635`.
- **O buraco:** a janela do `fork` não tem guarda determinística — o
  vermelho dela é 19 em 150, e guarda que cai às vezes não é guarda. E o
  Windows não foi exercitado: nem o alvo de compilação está instalado aqui.
