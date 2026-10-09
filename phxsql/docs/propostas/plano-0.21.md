# Plano da 0.21 — 707 (aquário), 454/455 (PhxZip) e 495/496 (IA)

Abertura da 0.21, 09/10/2026. Prioridade do dono: 454, 455, 495, 496 e 707
(o 333 está congelado). Desenho da IA em `ia-495-496-desenho.md`.

## Ordem

1. **A0 (J, só documento) — feita, 09/10/2026** (`aquario-707.md` §11): uma base só (Welford sobre
   `ln(µs)` de serviço, duas metades de 30 min, n ≥ 20, z ≥ 4 e ≥ 250 ms, sem `OPS_DE_REPLICACAO`,
   5.000 chaves + coringa, **ligada atrás do portão da telemetria**); `Alarme` = tipo da `Ocorrencia`,
   um produtor e dois arquivos com papéis; contagem por minuto no `aquario.log` e por **hora** em
   `aquario-horas.jsonl` fora do rodízio (o motor não tem fuso; a tela soma dia/semana/mês). Morreram:
   histograma `2 × p95`, Welford cumulativo/n ≥ 30/desligado, um log só, dia UTC. A1:
   `bancada/aquario/regra.py` a partir do Apêndice B.
2. **A2** — esqueleto do aquário, com o `enum Alarme` e o escritor `registrar_json` (partilhados por
   A3, A4, A6 e a F2 do 495): único que toca `servidor.rs`, `catalogo.rs`,
   `usuarios.rs`, `direito_coluna.rs` e as listas de op; deixa no `anotar` uma
   chamada única `telemetria.aquario.anotar(acesso)`.
3. Em paralelo, cada um no próprio arquivo: A3 (alarmes na origem), A4 (linha
   de base, ≡ F4 do 495), A6 (aquario.log), A8 (contagens — o item 8 do dono: os três
   gráficos), A9 (física em SVG contra retrato inventado); Z1 (HTTP no core),
   Z2 (fábrica de idiomas no core), Z3 (blocos), Z4 (tar), Z9 (PhxZipCmd).
4. Depois: A5 (classificar), A7 (matar), A10–A13 (tela, em série), Z5–Z8
   (servidor web do PhxZip, em série), Z10 (laço de textos), Z11–Z12 (manual
   e pacote), A14 (documentação).

## Fatias do 707

| # | o quê | prova (RED) |
|---|---|---|
| A0 | ☑ unificar linha de base, alarme/ocorrência e contagem com o 495/496 (`aquario-707.md` §11) | — |
| A1 | `bancada/aquario/regra.py` regera o §11 (Ha1 × Ha4: 1/9.317 cada; sem piso 39 × 40; sem exclusão 15 × 51; com exclusão 37) | — |
| A2 | `aquario/mod.rs`, `enum Alarme`, `registrar_json`, ops `aquario_log` e `aquario_contagens`, direito `Monitorar` | op sem direito declarado cai |
| A3 | `aquario/alarme.rs`: bits na origem (LOCK, DADO, PRAZO, E/S) | tirar o bit do reentrante → as duas classes empatam |
| A4 | `aquario/base.rs` (≡ F4 do 495): Welford `ln(µs)`, 2 × 30 min, n ≥ 20, z ≥ 4, ≥ 250 ms, sem replicação, atrás do `ligada()` | vítima da fila vira anormal; 20 × 1 ms exatos + 10 s não alarma; desligada custa 0 |
| A5 | `aquario::classificar` — retrato e log pela mesma função | tabela de casos |
| A6 | `aquario.log` 8 MiB × 8, grava sem cliente perguntando | `\n` no op; disco cheio contra o SO |
| A7 | ver ≠ matar; `servico` recusado; alvo na trilha | queda pelo soquete |
| A8 | contagem por minuto (`aquario.log`) e hora (`aquario-horas.jsonl`); categoria decidida pela op; `null` ≠ 0 | `excluir fisico` como suave cai; backup falho sobe só `erro`; 2 min fora = 2 `null` |
| A9 | `ui/aquario.js` SVG, colisão, faixas, 60 fps a 150 bolhas | tirar a colisão → sobreposição |
| A10 | `telaAquario()`, `?tela=aquario`, alça, `visibilitychange` | aba escondida faz 0 pedidos |
| A11 | log pesquisável + gráficos dia (ao vivo)/semana/mês à direita | `inserir` real sobe a barra do dia |
| A12 | cartão de matar na página | `monitorar` não vê nem consegue |
| A13 | modo TV, selo de frescor, volta de 5 min | servidor derrubado → VELHO em ≤ 3 s |
| A14 | TELEMETRIA.md, MENSAGENS.md, cognição | — |

## Fatias do 454/455

| # | o quê | prova (RED) |
|---|---|---|
| Z1 | `phxsql-core/src/http.rs` com corpo binário e `drenar` | 0xFF intacto; cliente lê o 413 |
| Z2 | fábrica de idiomas no core | célula vazia e idioma desconhecido caem no português |
| Z3 | `Entrada::bloco`, `Arquivo::blocos`, `Json::analisar_com_posicao` | sólido de 2 entradas = 1 bloco |
| Z4 | `phxzip/src/tar.rs` (ustar + pax) | nome de 150 bytes intacto |
| Z5 | `crates/phxzip-web`: só 127.0.0.1, estáticos embutidos, Host/Origin | Host alheio recusa; `/../` 404 |
| Z6–Z8 | listar, testar, compactar, espiar, extrair (`.tar`), 413 com dreno | Playwright contra o servidor real |
| Z9 | `crates/phxzip-cmd`: senha nunca por argumento; extração sem seguir link | `--senha x` recusado; zip-slip |
| Z10 | laço de textos da tela do PhxZip | chave tirada cai |
| Z11 | `docs/MANUAL-PHXZIP.md` com bancada contra o `7z` | — |
| Z12 | `empacotar-phxzip.sh` + `conferir` | byte trocado → vermelho |

## Decisões do dono, 09/10/2026

- **TV do aquário:** só operação, tabela e cor; o usuário pseudonimizado.
  Login e IP só na tela do administrador logado (LGPD).
- **Plataformas do PhxZip:** as mesmas do PhxSql — Linux x86_64, Windows
  x86_64 e ARM musl, com o mesmo empacotador e a mesma prova cruzada.
- **Manual do PhxZip:** só em português nesta versão; a tela já tem os seis.
- **Nomes:** o executável web é **PhxZipWeb** (`phxzipweb`), o de terminal
  **PhxZipCmd** (`phxzipcmd`); o pacote é `phxzip-<versão>-<plataforma>`.
- Ficam com o J, sem subir: a porta do PhxZipWeb (o contrato sugere 7700) e o
  PhxZipCmd falando pela fábrica de idiomas (recomendado: sim).
