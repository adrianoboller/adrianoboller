# Premissa P-A: quantas das 64 vagas HTTP a interface consome em repouso

**Estado: NÃO MEDIDA.** O arnês funciona; a medição não conclui, e este
documento diz exatamente por quê. Aberta em 17/09/2026 pelo integrador.

## Por que ela existe

O pedido **333** está publicado dizendo que um long-poll de 50 s ocupa uma das
`conexoes_web_max = 64` vagas por 50 s, e que o teto do chat é 64 **menos o que
a própria tela já come**. Esse «menos» nunca foi medido — o próprio pedido o
declara «raciocinado, não medido». Enquanto ele não for número, o teto de
usuários do chat é palpite dentro de um pedido publicado, e a lei da casa manda
medir a premissa antes de o item virar plano.

## O que ela mede, e o que já saiu medido

O número sai **do próprio servidor**: a op `telemetria` devolve `resultado.tetos`,
montado por `Servidor::tetos_das_threads` (`crates/phxsql-server/src/servidor.rs`),
com `familia`, `vivo`, **`em_uso`**, `teto` e `esperando` para `dados` e `http`.
O teste `os_tetos_das_threads_saem_com_a_ocupacao_viva` trava esse contrato.

Medido em 17/09/2026 17:5x UTC, binário de 17:53 (recompilado porque o de 13:46
era mais velho que o `servidor.rs` do commit `acfaf60` — *medidor com binário
velho mede o passado*):

| medida | valor |
|---|---:|
| `teto` da família `http` | **64** |
| `em_uso` com o servidor de pé e nenhum navegador (**controle positivo**) | **0** |
| `em_uso` com a tela aberta, 6 amostras de 1 s | **0, 0, 0, 0, 0, 0** |

## Por que isso NÃO é «a tela custa zero»

Dois motivos, os dois medidos, e é por isso que a bancada **se recusa a
concluir** em vez de publicar o zero:

1. **O navegador para no login.** O `ui/index.html` tem **225** ocorrências de
   `login`/`senha`, e os **6** `setInterval` dele vivem **depois** do login
   (`atualizaBarra`, `relogioMaquina`, o Profiler a 1 s). A tela que a bancada
   abre não está sondando nada, então o zero é do login — não da tela em uso.
2. **Amostragem a 1 Hz não resolve ocupação de ~1 ms.** Uma vaga tomada por um
   pedido curto tem chance da ordem de 0,1% de cair no instante da amostra.
   Mesmo com a tela sondando, este instrumento daria zero quase sempre — e
   **zero por cegueira é pior que zero ausente**.

## O que falta, nomeado

- **Login pela tela** dentro do `abrir-e-segurar.mjs`, para os `setInterval`
  começarem.
- **Marca-d'água de pico** no `Semaforo` (um `max` que só sobe) no lugar da
  amostragem instantânea. É a mesma diferença entre `vivo` e `em_uso` que já
  derrubou uma premissa aqui.

## Três premissas do integrador que morreram no caminho

Todas pegas pelo **controle positivo**, que é a razão de ele vir antes do
veredito:

- `vivo` **não** é a ocupação — é um booleano que diz se a família tem semáforo
  de verdade (o `fixo` do fonte põe `false`). A ocupação é **`em_uso`**.
- O envelope da resposta é `{"ok":…, "op":…, "resultado":{…}}`, não `resposta`.
- **Token de serviço não abre a `telemetria`**: o servidor recusa com
  `[SP000025] acesso negado: faca login antes: {"op":"login",…}` — e a mensagem
  dele diz o remédio por extenso, que é o que uma mensagem de erro tem de fazer.

## Como roda

```
flock /tmp/phx-cargo.lock cargo build --release --bin phxsqld
python3 bancada/vagas-da-tela/medir.py --sem-tela   # só o controle
python3 bancada/vagas-da-tela/medir.py             # controle + tela
```

Sobe um `phxsqld` próprio em porta alta e diretório temporário (nunca encosta
em banco de ninguém) e o mata **pelo PID** — nunca por `pkill`, que derrubaria
o servidor de outra frente na mesma máquina. O navegador entra pelo **Node**,
por caminho absoluto, como o `docs/dossie/olhar.mjs`.
