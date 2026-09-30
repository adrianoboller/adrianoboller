#!/usr/bin/env bash
# E2E nativo do PostgreSQL: banco descartavel -> FULL_INSTALL -> POST_INSTALL_VERIFY ->
# baterias de tests/postgres e tests/sql, cada uma com o papel que ela exige.
#
# Por que dois papeis: superusuario ignora RLS, entao prova de isolamento rodada como
# admin passa por engano. As provas de RLS rodam como phx_rls (NOSUPERUSER, NOBYPASSRLS,
# nao-dono das tabelas); as de fluxo que o proprio teste abre como admin rodam como admin.
#
# Uso: PGHOST=/tmp PGPORT=55432 tests/postgres/run_e2e.sh
# Sai 0 so se TUDO passar; imprime uma linha por bateria e o placar.
set -u
cd "$(dirname "$0")/../.."
: "${PGHOST:=/tmp}" "${PGPORT:=55432}" "${PGADMIN:=postgres}"
DB="${PHXCLAW_E2E_DB:-phxclaw_e2e}"
export PGHOST PGPORT
ADMIN=(psql -U "$PGADMIN" -v ON_ERROR_STOP=1 -qtA)
RLS=(psql -U phx_rls -v ON_ERROR_STOP=1 -qtA)

"${ADMIN[@]}" -d postgres -c "DROP DATABASE IF EXISTS $DB" -c "CREATE DATABASE $DB" >/dev/null || exit 2
"${ADMIN[@]}" -d "$DB" -f database/PhxClaw_PostgreSQL_FULL_INSTALL_v0.70.sql >/dev/null 2>"$DB.install.err" \
  || { echo "FALHA install"; grep ERROR "$DB.install.err"; exit 1; }
rm -f "$DB.install.err"
"${ADMIN[@]}" -d "$DB" -f database/PhxClaw_PostgreSQL_POST_INSTALL_VERIFY_v0.70.sql >/dev/null 2>&1 \
  || { echo "FALHA post_install_verify"; exit 1; }
echo "ok   install + post_install_verify ($("${ADMIN[@]}" -d "$DB" -c 'select count(*) from phxclaw_schema_migrations') versoes)"

# Reinstalar tem de ser RECUSADO: FULL_INSTALL e so para banco novo.
if "${ADMIN[@]}" -d "$DB" -f database/PhxClaw_PostgreSQL_FULL_INSTALL_v0.70.sql >/dev/null 2>&1; then
  echo "FALHA reinstalacao foi aceita"; exit 1
fi
echo "ok   reinstalacao recusada"

"${ADMIN[@]}" -d "$DB" >/dev/null <<'SQL'
DO $r$ BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'phx_rls') THEN
    CREATE ROLE phx_rls LOGIN NOSUPERUSER NOBYPASSRLS;
  END IF;
END $r$;
GRANT USAGE ON SCHEMA public, phxclaw TO phx_rls;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public, phxclaw TO phx_rls;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public, phxclaw TO phx_rls;
GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA public, phxclaw TO phx_rls;
-- Fixture v0.34 que o v035 exige para nao passar no vazio: um portfolio do tenant A.
INSERT INTO phxclaw_ai_portfolios(tenant_uuid, portfolio_uuid, policy_sha256, source_arena_sha256,
  document_sha256, signer_id, signature_hex, starts_at, expires_at, portfolio_json)
VALUES ('11111111-1111-7111-8111-111111111111', '11111111-1111-7111-8111-1111111111a1',
  repeat('a', 64), repeat('b', 64), repeat('c', 64), 'e2e-fixture', repeat('d', 128),
  clock_timestamp(), clock_timestamp() + interval '1 day', '{}'::jsonb);
SQL

passou=0; falhou=0
roda() { # papel arquivo [tenant]
  local papel=$1 f=$2 tenant=${3:-} saida
  if [ -n "$tenant" ]; then export PGOPTIONS="-c phxclaw.tenant_uuid=$tenant"; else unset PGOPTIONS; fi
  if [ "$papel" = rls ]; then saida=$("${RLS[@]}" -d "$DB" -f "$f" 2>&1); else saida=$("${ADMIN[@]}" -d "$DB" -f "$f" 2>&1); fi
  if [ $? -eq 0 ]; then passou=$((passou + 1)); echo "ok   [$papel] $f"
  else falhou=$((falhou + 1)); echo "FALHA [$papel] $f :: $(echo "$saida" | grep -m1 ERROR)"; fi
  unset PGOPTIONS
}
roda rls   tests/postgres/v025_rls_role_prereq.sql
roda rls   tests/postgres/v024_rls_cross_tenant.sql
roda admin tests/postgres/v025_release_attestation.sql
roda admin tests/postgres/v062_knowledge_promotion_e2e.sql
roda admin tests/postgres/v065_installer_recovery_e2e.sql
roda admin tests/postgres/v066_research_hypothesis_e2e.sql
roda admin tests/postgres/v067_bpm_replay_e2e.sql
roda rls   tests/postgres/v070_rls_tenant_guc.sql
roda rls   tests/sql/v035_rls_e2e.sql
roda rls   tests/sql/v036_rls_e2e.sql
roda rls   tests/sql/v037_rls_e2e.sql
roda rls   tests/sql/v038_rls_e2e.sql
roda rls   tests/sql/v039_rls_e2e.sql 11111111-1111-7111-8111-111111111111
# Motor Rust contra o mesmo banco, como papel sujeito a RLS.
for crate in phxclaw-hypothesis phxclaw-bpm; do
  if PHXCLAW_E2E_RLS_URL="host=$PGHOST port=$PGPORT user=phx_rls dbname=$DB" \
     cargo test -q -p "$crate" --test pg_e2e -- --ignored >/dev/null 2>&1; then
    passou=$((passou + 1)); echo "ok   [rust/rls] $crate pg_e2e"
  else falhou=$((falhou + 1)); echo "FALHA [rust/rls] $crate pg_e2e"; fi
done
# Todo SQL literal do motor Rust tem de PREPARAR contra o esquema instalado.
if python3 tools/pg_prepare_sweep.py "host=$PGHOST port=$PGPORT dbname=$DB user=$PGADMIN" >"$DB.prep" 2>&1; then
  passou=$((passou + 1)); echo "ok   [prepare] $(tail -1 "$DB.prep")"
else falhou=$((falhou + 1)); echo "FALHA [prepare]"; cat "$DB.prep"; fi
rm -f "$DB.prep"
echo "placar: $passou ok, $falhou falha(s)"
[ "$falhou" -eq 0 ]
