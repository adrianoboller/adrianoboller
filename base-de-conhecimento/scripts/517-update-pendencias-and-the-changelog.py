# Update PENDENCIAS and the changelog
# 28/08 16:43

p='CHANGELOG.md'
s=open(p).read()
a='''### Sabido

- **Não há TLS em lugar nenhum**'''
b='''- **Erro com código estável.** A resposta traz `codigo`, `nome`, `classe` e
  `repetir` além do texto. Sem código, integrar exige comparar **texto** — e
  melhorar a redação de uma mensagem quebraria o cliente sem ninguém perceber.
  Número publicado não muda, e há teste que falha se mudar.

- **`sessoes` e `encerrar_sessao`** — quem está falando com o servidor agora, o
  que cada um executa e há quanto tempo, e como derrubar. Porta de dados e
  sessões do navegador na mesma lista.

- **`estatisticas`** — percentis, histograma de faixas que dobram, as mais
  demoradas, e uso por tabela, operação, usuário e código de erro. A média some
  de propósito: mil respostas de 1 ms e uma de 30 s dão média de 30 ms.

- **`checksum` de tabela** e **tempo no ar** no `ping`.

### Sabido

- **Não há TLS em lugar nenhum**'''
assert a in s; s=s.replace(a,b,1)
a='''- **Junção é de duas tabelas por vez, e só por igualdade.**'''
b='''- **Não há compactação (`OPTIMIZE TABLE`).** O `.reg` nunca reaproveita slot
  excluído, e compactar significaria reescrever `rowid` — que é endereço. Uma
  tabela com muitas exclusões cresce e não encolhe: é consequência aceita da
  ordem de digitação ser garantida, não esquecimento. Detalhes em
  `docs/COMPARACAO.md`.

- **O código de erro é por variante, não por situação.** `ESQUEMA_INVALIDO`
  cobre desde config errado até chave de junção incompatível.

- **Junção é de duas tabelas por vez, e só por igualdade.**'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
