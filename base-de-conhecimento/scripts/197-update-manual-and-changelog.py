# Update manual and changelog
# 27/08 21:23

s=open('CHANGELOG.md').read()
novo = '''## 0.3.0 — 2026-08-27

### Corrigido

- **O nível de usuário quase afrouxou todo `config.json` existente.** O padrão
  do campo novo `nivel` era `leitor`, e isso mudava o comportamento de quem já
  tinha config: base sem regra explícita passava de *nega tudo* para *lê tudo*.
  Um teste antigo (`sem_curinga_e_sem_base_nega_tudo`) quebrou e apontou o
  problema. Existe agora `Nivel::Nenhum`, que é o padrão, e o teste antigo
  passa sem alteração — que é a prova de que nada mudou para quem já tem
  config.

- **`phxsqld --usuarios` mentia sobre quem podia o quê.** Escrevia
  `(nenhuma)` para usuário sem regra de base, mesmo quando o nível dava poder,
  e mostrava `supervisor` numa coluna em vez do nível. Agora mostra o nível e
  o que ele concede.

### Adicionado

- **Nível de usuário:** `nenhum`, `leitor`, `operador`, `dono`, `admin`. Cada
  um contém o anterior, e há teste que percorre as dez atividades para
  garantir. A regra de uma base específica ganha do nível, inclusive para
  **tirar** poder — dá para dar `admin` a alguém e ainda assim fechar uma base.

- **Backup em ZIP**, com o DEFLATE (RFC 1951) escrito neste projeto — Huffman
  fixo mais casamento LZ77. Nome
  `BancoNome_Admin_Data_HoraMin.zip`, com o manifesto dentro.

  A prova não é o teste de ida e volta com o próprio código; é o mundo abrir:
  `unzip -t` passa todos os CRC, e o `zipfile` do Python extrai e confere byte
  a byte contra o original. **18.311 → 2.406 bytes, 87% menor.**

- **Backup agendado**, seção `backup` no `config.json`, desligada por padrão.
  `hora` (uma vez por dia) ou `cada_horas`, com `manter` para a retenção. O
  relógio confere de minuto em minuto em vez de dormir até a hora — dormir
  horas seguidas é frágil. A faxina só apaga arquivo com a cara dos nossos.
  Todo backup agendado entra no `acessos.log`.

### Sabido

Continua tudo da 0.2.0: replicação sem transporte, sem start/stop pela
interface, sem transações, sem TLS, sem compactação, sem SQL, sem MCP, sem
ODBC.

**276 testes**, clippy limpo, zero dependências externas.

---

'''
s = s.replace('## 0.2.0 — 2026-08-27', novo + '## 0.2.0 — 2026-08-27')
open('CHANGELOG.md','w').write(s)
