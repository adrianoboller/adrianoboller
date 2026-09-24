# A gerência do OpenVPN: `kill CN` não avisa o cliente; `client-kill` avisa

## O que aconteceu

Frente de segurança do phxvpn (limites 6 e 12 da revisão do MFA): mudar senha,
autenticador ou `ativo` de um usuário tinha de derrubar a conexão VPN dele em
segundos. O painel passou a falar com o `openvpn` de cada rede pela gerência
(`management dados/gerencia/<rede>.sock unix`) e a derrubar as conexões do CN
do membro (`phxvpn/src/credencial.rs`, `derrubar`).

## O que eu concluí primeiro, e estava errado

1. **«`kill <CN>` derruba a conexão.»** É o comando óbvio da gerência e
   responde `SUCCESS: common name ... found, 1 client(s) killed`. Mas no
   2.6.19 ele chama `multi_signal_instance(SIGTERM)`: fecha a instância **no
   servidor** e não manda nada ao cliente. O cliente continua achando que está
   conectado até o `ping-restart` (60 s no perfil do phxvpn). O
   `client-kill <CID>` chama `send_restart`, que manda `RESTART` pelo canal de
   controle — e o CID só sai do `status 2` (coluna «Client ID»).
2. **«Encurtar o `auth-gen-token` resolve o limite 6.»** A vida e a renovação
   do token dizem quando o *token* vence; a conta mudada não vence token
   nenhum. O que revoga é `external-auth` (o verificador é chamado também com
   token válido, `session_state=Authenticated`) mais uma sessão presa à
   credencial com que nasceu.
3. Na prova, a primeira corrida mostrou a queda em 0,05 s mas **nenhuma
   reconexão** em 17 s: não era o servidor, era o `--management-hold` do
   cliente de prova segurando a reconexão até alguém mandar `hold release`.
   E a conexão levou 6,5 s em vez de 1,2 s porque o binário era o de
   depuração (PBKDF2 lento → o primeiro `PUSH_REQUEST` chega antes da
   resposta adiada e o cliente só repete 5 s depois).

## O que a medição disse

`provas/mfa/rodar.sh`, `openvpn` 2.6.19 real, binário release, 24/09/2026:

| Comando na gerência | Caso | Queda no cliente |
|---|---|---|
| `client-kill CID` | admin zera o autenticador | **0,06 s**; reconexão pelo token recusada (`auth-failure (auth-token)`) |
| `client-kill CID` | admin desativa o usuário | **0,05 s**; reconexão recusada |
| `kill CN` | as duas mudanças | **não caiu em 20 s** (servidor: «client-instance exiting») |
| nenhum (RED: soquete fora do lugar) | admin desativa | não caiu em 15 s |

## A regra

Para derrubar um cliente do OpenVPN pela gerência, use `client-kill` com o CID
do `status 2`, nunca `kill CN` — e prove a queda **no cliente**, não pela
resposta `SUCCESS` do servidor.

## Como está guardado hoje

- `phxvpn/src/credencial.rs::derrubar` usa `status 2` + `client-kill`, com o
  porquê no comentário; `cids_do_cn` lê a coluna do CID.
- `teste:derrubar_pede_o_status_e_mata_pelo_cid` (gerência de mentira: exige a
  sequência `status 2` → `client-kill 7` → `quit`).
- `provas/mfa/rodar.sh` mede a queda pelo estado do cliente (`caiu_em`) e tem
  o RED sem gerência; `provas/mfa/resultados.json` → `revogacao`.
- **Buraco que fica:** o `UPDATE` feito à mão no banco não chama a gerência
  (só as rotas do painel chamam); ali a queda espera a renegociação (até 1 h).
  No Windows não há gerência por soquete Unix.

## Estado

- **Estado:** FRUTÍFERO
- **Evidência:** `phxvpn/provas/mfa/resultados.json`, `phxvpn/src/credencial.rs`, `phxvpn/tests/postgres_real.rs`
  (os testes `derrubar_pede_o_status_e_mata_pelo_cid` e `sessoes_caem_quando_a_credencial_muda` moram nesses dois `.rs`; o `classificar.py` só procura `teste:` dentro de `phxsql/`)
- **Validado em:** 24/09/2026
