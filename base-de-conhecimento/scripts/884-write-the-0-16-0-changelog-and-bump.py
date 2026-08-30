# Write the 0.16.0 changelog and bump
# 28/08 23:09

import pathlib
p = pathlib.Path("CHANGELOG.md")
s = p.read_text()
antigo = """## 0.15.0 — 2026-08-28"""
novo = """## 0.16.0 — 2026-08-28

**Profiler**, as **cores da ação**, **Docker** e a leitura do HFSQL(R) e do
DBeaver contra o projeto.

### Corrigido

- **Salvar e incluir pela tela estavam quebrados** desde que o `rownum` entrou:
  a ficha tirava só a *primeira* coluna de sistema e mandava 8 valores para uma
  tabela de 9 colunas. Achado **gravando o vídeo de demonstração**.
- **A tela da Replicação** dizia que a replicação não existia, e lia o campo
  errado da resposta de `bancos`.
- **O erro da réplica** saía sempre como «acesso negado», inclusive para um
  database que ainda não existe no master.

### Adicionado

- **Profiler.** O que está chegando pela porta, **antes de virar dado** — o
  ponto de captura é uma linha depois do `read_line` e uma antes do despacho.
  Por isso o pedido que *trava* aparece na lista como «em curso», que é
  justamente o que se quer achar. Filtra por banco, usuário, operação e «só
  escrita»; guarda num anel de tamanho fixo e grava num `.txt` no caminho que o
  administrador escolher. Observa **as duas portas** — deixar a interface web
  de fora faria ele mentir por omissão para quem está olhando por ela — e não
  observa a si mesmo.

  **A senha não passa por aqui**, e é a regra que mais importa neste arquivo:
  um profiler é exatamente onde uma senha vazaria sem ninguém notar. O texto é
  **analisado** e os campos sensíveis viram `"***"` antes de encostar na
  memória ou no arquivo — nunca recortado, porque recortar depende de o pedido
  estar escrito de um jeito. Pedido que não é JSON vira o tamanho em bytes.

- **As cores da ação**: verde inclui, amarelo altera, rosa marca (o excluir que
  volta), vermelho exclui de vez, azul consulta. **Contorno e não fundo cheio**
  — a lição já estava escrita no CSS: fundo laranja com texto escuro em cima
  ficava ilegível. No diálogo de excluir o botão troca de cor junto com o texto.

- **Docker**, com imagem `scratch`: sem shell, sem gerenciador de pacotes, só o
  binário. Exige o alvo **musl** — medido: o padrão linka `libc.so.6`,
  `libgcc_s.so.1` e o carregador dinâmico, e `FROM scratch` não subiria. Com
  musl são 3,4 MB o servidor e 1,2 MB o cliente, `static-pie`, e o binário roda.
  Um `docker-compose.yml` sobe um master e duas réplicas.

- **Teste da chave composta**, livre e única — as duas já existiam no formato e
  nenhuma tinha teste que as separasse.

- **`docs/HFSQL.md`**, **`docs/DBEAVER.md`** e **`docs/CLUSTER.md`**: a leitura
  da documentação do HFSQL(R) item a item contra o código, o que dá para
  reaproveitar do DBeaver, e o que exatamente falta para haver cluster.

### Sabido

- **O `docker build` não foi executado** — não há daemon Docker na máquina em
  que isto foi escrito. O que foi verificado: que o alvo musl produz binário
  estático, e que esse binário sobe e responde.
- **Não há cluster.** Há replicação, e com ela escala de leitura. Falta endereço
  único, eleição de primário e promoção automática.
- **A segunda gravação ainda vence em silêncio.** O HFSQL(R) mostra uma janela
  de conflito com «valor anterior / o outro escreveu / você escreve»; aqui não
  há detecção nenhuma. A peça está no formato — o `.reg` guarda uma versão por
  registro — e o item está em `PENDENCIAS.md`.

---

## 0.15.0 — 2026-08-28"""
assert antigo in s
s = s.replace(antigo, novo, 1)
p.write_text(s)
