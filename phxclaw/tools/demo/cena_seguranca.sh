source "$(dirname "$0")/comum.sh"
titulo "Segurança: dependências auditadas e ataques reais barrados" \
       "cada prova abaixo falhava antes do conserto desta rodada e passa agora"
roda "cargo audit 2>&1 | grep -E 'Scanning|vulnerabilit|allowed warn' | head -3" 2
roda "cargo test -p phxclaw-egress-broker 2>&1 | grep -E '^test tests'" 2
roda "cargo test -p phxclaw-channel-providers 2>&1 | grep -E '^test tests'" 2
roda "cargo test -p phxclaw-sandbox 2>&1 | grep -E '^test tests'" 2
nota "SSRF por redirecionamento, token em erro, segredo na lista de processos: barrados" 4
