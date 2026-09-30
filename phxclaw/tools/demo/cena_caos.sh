source "$(dirname "$0")/comum.sh"
titulo "Caos: SIGKILL no PostgreSQL enquanto o motor decide hipóteses" \
       "a cada 2–5 s o banco é morto sem aviso e religado; no fim, confere se alguma decisão confirmada sumiu"
digita "tests/chaos/pg_kill_loop.sh 20 &   # o matador: cada queda aparece abaixo"
tests/chaos/pg_kill_loop.sh 20 2>/dev/null | while IFS= read -r l; do printf '  \e[1;31m%s\e[0m\n' "$l"; done &
sleep 1
roda "PHXCLAW_CHAOS_SECONDS=20 cargo test -q -p phxclaw-hypothesis --test pg_chaos -- --ignored --nocapture 2>&1 | grep -E 'caos:|test result'" 1
sleep 4
nota "decisões confirmadas perdidas: 0 — diário e estado divergentes: 0" 4
