# Licença e serial de ativação

O plugin só roda com um serial válido instalado em `~/.wx-claude-code/licenca`
(ou no caminho de `$WX_LICENCA`). O serial é assinado com **RSA-2048** pela
chave privada de quem distribui; o plugin carrega só a chave pública
(`chave-publica.json`) e por isso **não consegue emitir nem forjar** um serial.
Tudo é `std` do Python, sem dependência, como o resto do plugin.

## O que isto protege, e o que não protege

Um plugin do Claude Code é texto: quem instala lê os agentes, os scripts e a
chave pública. O serial e os hooks são **dissuasão para o cliente honesto**
(licença vencida, máquina trocada, cópia passada adiante por descuido). Quem
quiser apagar duas linhas do `hooks.json` remove a trava. **A proteção real é
servir o corpus do Help, as referências e os agentes de um servidor seu**, com
o serial conferido a cada chamada; aí o plugin sem servidor não tem o que
consultar, e um serial vazado se revoga na hora. Este arquivo cobre a primeira
camada; a segunda é um MCP server, e o `licenca.py` já é o cliente dela.

## Como funciona

- `chave-publica.json`: `n` e `e` da chave RSA. Troque pelo seu par antes de
  distribuir: **a chave pública deste repositório é de demonstração**.
- Serial: `WX2.<payload>.<assinatura>`, com `payload` em JSON (id, cliente,
  e-mail, validade, impressão da máquina opcional, data de emissão) e a
  assinatura RSA/SHA-256 do payload. Um byte alterado e a verificação falha.
- Hook `SessionStart`: injeta no contexto «licenciado para X até Y» ou «sem
  licença válida: recuse os comandos». Hook `PreToolUse`: nega a execução dos
  scripts do plugin (`Bash` com `conversao-wx/scripts`) e toda escrita em
  `.wx-migration/` enquanto não houver serial válido. O resto do Claude Code
  continua funcionando: projeto que não usa o plugin não é afetado.
- Custo medido do hook por chamada de ferramenta: mediana 54 ms, e é quase todo subida do interpretador Python; a conferência RSA em si não aparece na medida (comando comum e comando do plugin dão o mesmo número). Gerar o par de chaves leva cerca de 1 s.

## Para quem distribui

```bash
# uma vez: par de chaves (a privada nasce com permissão 0600, fora do repositório)
python3 skills/conversao-wx/scripts/licenca.py chaves gerar --saida ~/.wx-claude-code/chaves
cp ~/.wx-claude-code/chaves/chave-publica.json licenca/chave-publica.json

# por cliente
python3 skills/conversao-wx/scripts/licenca.py gerar --cliente "Softhouse X" --email x@x.com \
  --validade 2027-12-31 --chave-privada ~/.wx-claude-code/chaves/chave-privada.json
# preso à máquina: peça ao cliente o resultado de `licenca.py maquina` e passe em --maquina
```

Guarde a chave privada como guarda a senha do banco: quem a tiver emite serial
para qualquer um. Se vazar, gere outro par, troque a chave pública numa versão
nova do plugin e reemita os seriais em vigor.

## Para o cliente

```bash
python3 "$CLAUDE_PLUGIN_ROOT/skills/conversao-wx/scripts/licenca.py" instalar "WX2.…"
python3 "$CLAUDE_PLUGIN_ROOT/skills/conversao-wx/scripts/licenca.py" verificar
```

Estados: `valida`, `ausente`, `vencida`, `maquina-diferente`,
`assinatura-invalida`, `formato-invalido`, `chave-ausente` (a distribuição veio sem a chave pública). O serial não guarda segredo nenhum
do cliente além do nome e do e-mail que você mesmo pôs nele.

## Marca d'água

Quando há licença válida, todo `CLAUDE.md` e `empresa.md` gerados pelo
questionário recebem a linha «Gerado sob a licença WX Claude Code nº ID para
CLIENTE». Cópia que aparecer em outro lugar diz de onde saiu.
