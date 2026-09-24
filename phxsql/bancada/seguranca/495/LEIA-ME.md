# Medidores do pedido 495

São os medidores com que o papel J decidiu o desenho do detector de injeção
(`docs/propostas/pesquisa-495-ia-seguranca-2026-09-24.md`). Vieram do scratch
da sessão para não morrerem com ela.

| arquivo | o que mede | como refazer |
|---|---|---|
| `extrair_legitimo.py` | o corpo de SQL legítimo do repositório inteiro (crates, bancadas, tela, `comando.sql`) | `python3 extrair_legitimo.py` → `legitimo.jsonl` |
| `extrair_deteccao.py` | as cargas de ataque do `ARSENAL` e as mesmas cargas gravadas como DADO | `python3 extrair_deteccao.py` → `deteccao.jsonl`, `escapado.jsonl` |
| `detector/` | falso positivo, detecção e custo por comando: analisando o léxico × recortando o texto | `cargo run --release --manifest-path detector/Cargo.toml -- .` depois dos dois extratores |
| `premissa_sec.py` | se a tautologia do `ARSENAL` executa no binário atual | `python3 premissa_sec.py` (sobe servidor próprio, porta 6795) |
| `prova_215.py` | se o 215 ligado bloqueia o IP por SQL legítimo sem `;` | `python3 prova_215.py` (porta 6797) |
| `rtt_sql.py` | ida e volta de um pedido `sql` pelo soquete | `python3 rtt_sql.py` (porta 6799) |
| `medida-1.txt`, `medida-2.txt` | as saídas das duas corridas de 24/09/2026 | — |

Os `.jsonl` são saídas e não se versionam: os extratores os refazem, e
refeitos acompanham o repositório de hoje em vez de congelar o de 24/09.
