# O teste que passava no repo e falhava do zip, por causa do /tmp

Data da descoberta: 12/09/2026, 03:47.

## 1. O que aconteceu

O dono pediu o zip de fontes e "instale e teste no Linux". Gerei o
`phxsql-0.18.0-fontes.zip` pelo `empacotar.sh fontes`, extrai num diretorio
limpo **sob o `/tmp`** (o scratchpad da sessao) e rodei `cargo build --offline
--release` (verde, 8 crates em 38s, zero-deps confirmado) e `cargo test
--offline --workspace`. Deu **1 falha em 1953**:
`restaurar::tests::o_vizinho_da_base_relativa_e_o_diretorio_de_trabalho`, no
`restaurar.rs:1065`, na asserção `assert!(!v.starts_with(std::env::temp_dir()))`.

O mesmo teste passa no repositorio (`/home/user/...`). A diferenca nao e o
codigo: e **o lugar de onde se roda**.

## 2. O que eu conclui primeiro, e estava errado

Meu primeiro palpite foi "o zip esta incompleto — faltou um fixture, e por
isso o restaurar falha". Estava errado, e a medicao derrubou: os arquivos de
teste do zip sao **identicos** aos do repo (55 arquivos de integracao = 55,
170 arquivos com `#[test]` = 170). O zip nao perdeu nada.

O defeito e do TESTE, nao do produto. `vizinho_da_base("dados")` — base
relativa, pai vazio — devolve `current_dir().join(carimbo)`, e o codigo esta
certo. A asserção `!v.starts_with(temp_dir())` embute uma premissa: *"o
diretorio de trabalho nunca esta sob o /tmp"*. Verdadeira no repo; falsa quando
alguem descompacta os fontes no `/tmp` e roda `cargo test` — que e exatamente o
que quem baixa o zip faz.

## 3. O que a medição disse

Panico exato: `restaurar.rs:1065:9 — assertion failed:
!v.starts_with(std::env::temp_dir())`. Como `current_dir()` estava sob o
`temp_dir()`, o vizinho (corretamente ao lado da base = sob o CWD = sob o
`/tmp`) disparava o `!starts_with(temp)`. A asserção de cima,
`v.parent() == aqui`, **passava** — ela ja prova que nao houve fallback para o
`/tmp`, porque um fallback poria o pai em `temp_dir()`, e nao em `aqui` (que e
um subdiretorio mais fundo). Conserto: rodar o `!starts_with(temp)` so quando
`aqui` nao esta sob o temp; a guarda real (`v.parent() == aqui`) fica em todos
os casos. Depois: **passa no /tmp e no local normal**, `fmt` limpo.

## 4. A regra

Teste que le `current_dir()` ou `temp_dir()` nao pode assumir onde o pacote foi
descompactado. **A prova real de um zip de fontes e extrair FORA do repo — de
preferencia no /tmp — e rodar `build --offline` e `test` de la.** O repo e o
unico lugar onde o CWD nunca cai sob o /tmp, e por isso e o unico lugar onde
esse defeito nao aparece.

## 5. Como está guardado hoje

Consertado no `restaurar.rs` (guarda condicional + comentario que explica o
porque). A instalacao offline do zip esta provada verde. O buraco que fica:
**nao ha catraca** que rode a suite a partir de um diretorio sob o /tmp — a
prova foi manual desta vez. Um portao que extrai o zip de fontes e roda
`cargo test` de fora do repo pegaria a proxima premissa-de-CWD antes de o zip
chegar a quem baixa; fica anotado como pendencia, nao imposto (guarda nova
entra pedida).
