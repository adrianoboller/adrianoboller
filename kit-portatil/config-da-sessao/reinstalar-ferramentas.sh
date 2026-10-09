#!/usr/bin/env bash
# Reinstala, numa sessao NOVA, o que o conteiner da sessao anterior tinha e o
# git nao leva: os plugins e o MCP do Claude Code, os alvos de compilacao
# cruzada e o ligador do Windows. Tudo o mais (CLAUDE.md, agentes, docs,
# codigo) ja vem do clone.
#
# Medido em 09/10/2026: cada linha abaixo rodou nesta casa e funcionou. Rodar
# de novo e inofensivo -- cada passo confere antes de instalar.
#
# Uso:  bash kit-portatil/config-da-sessao/reinstalar-ferramentas.sh
set -u

passo() { printf '\n== %s\n' "$*"; }

passo "alvos de compilacao cruzada (PhxSql e PhxZip: ARM musl e Windows)"
for alvo in aarch64-unknown-linux-musl armv7-unknown-linux-musleabihf x86_64-pc-windows-gnu; do
  if rustup target list --installed 2>/dev/null | grep -qx "$alvo"; then
    echo "   ja instalado: $alvo"
  else
    rustup target add "$alvo" || echo "   FALHOU: $alvo (rede? veja o LEIA-ME)"
  fi
done

passo "ligador do Windows (x86_64-w64-mingw32-gcc)"
if command -v x86_64-w64-mingw32-gcc >/dev/null; then
  echo "   ja instalado"
else
  apt-get install -y gcc-mingw-w64-x86-64 >/dev/null 2>&1 \
    || echo "   FALHOU: apt-get install gcc-mingw-w64-x86-64"
fi

passo "plugins do Claude Code (pedido do dono, 09/10/2026)"
claude plugin marketplace add https://github.com/vercel/vercel-plugin || true
claude plugin marketplace add https://github.com/Yeachan-Heo/oh-my-claudecode || true
claude plugin install vercel-plugin@vercel || true
claude plugin install oh-my-claudecode@omc || true
claude plugin install supabase@anthropic-plugin-directory || true

passo "MCP do Chrome DevTools"
if claude mcp list 2>/dev/null | grep -q '^chrome-devtools'; then
  echo "   ja configurado"
else
  claude mcp add chrome-devtools --scope user -- npx chrome-devtools-mcp@latest || true
fi

passo "estado final"
claude plugin list 2>&1 | grep -E '^\s+>' || true
rustup target list --installed
echo
echo "Plugins e MCP so carregam na PROXIMA sessao (ou depois de reiniciar o Claude Code)."
echo "Vercel e Supabase pedem login da conta do dono no primeiro uso."
