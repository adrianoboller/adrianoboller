source "$(dirname "$0")/comum.sh"
titulo "PostgreSQL 16 real — instalação completa + isolamento entre clientes (RLS)" \
       "banco descartável: instala as 70 versões, confere, recusa reinstalar, roda 13 baterias SQL e 2 do motor Rust"
roda "psql -h /tmp -p 55432 -U postgres -tAc 'select version()' | cut -c1-40" 1
roda "tests/postgres/run_e2e.sh 2>/dev/null" 3
nota "340 tabelas, 275 com RLS; as provas de isolamento rodam como papel SEM superusuário" 4
