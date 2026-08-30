# Add 0.17.0 changelog entry
# 29/08 00:05

import pathlib
p = pathlib.Path("CHANGELOG.md")
s = p.read_text()
alvo = "## 0.16.0 — 2026-08-28"
novo = '''## 0.17.0 — 2026-08-29

Os gaps. Esta versão fecha itens que estavam na lista do que falta, e não
recursos novos inventados aqui.

### Adicionado

- **Janela de conflito de escrita** (pedido 123), a ideia que a leitura do
  HFSQL(R) apontou como a mais valiosa da lista. Duas pessoas com a mesma ficha
  aberta terminavam com a segunda gravação apagando o trabalho da primeira —
  sem erro, sem registro, sem ninguém perceber até faltar o dado.

  **Não mudou formato**: a versão por registro existe no cabeçalho do slot do
  `.reg` desde a v1 e ninguém a usava. `ler` devolve a versão com
  `"com_versao": true`; `atualizar`, `excluir` e `restaurar` conferem a versão
  que o cliente mandar; a recusa é o erro novo **3004 `CONFLITO`**. Conferir
  custa 24 bytes de leitura — o cabeçalho do slot, não a linha.

  A janela mostra as três colunas do PDF deles — «valor anterior», «o outro
  escreveu», «você escreve» — e vai um passo além: **já vem marcado quem mexeu
  em cada coluna**. Dois que editaram campos diferentes da mesma linha saem
  dali com os dois trabalhos preservados, sem escolher nada. Marcar tudo como
  «o meu» por omissão desfaria em silêncio o trabalho do outro nas colunas que
  eu nem toquei — o mesmo estrago de antes, com mais cliques.

  Três decisões que valem registro:

  - **Não é trava.** Travar na leitura prenderia a linha toda vez que alguém
    fechasse o navegador com a ficha aberta, e duas sessões que travam em ordem
    trocada se abraçariam.
  - **A conferência é pedida, não imposta.** Quem manda `"versao"` ganha a
    garantia; quem não manda continua com a última gravação vencendo. Imposta,
    todo cliente anterior a esta versão pararia de gravar de um dia para o
    outro. A interface web manda sempre.
  - **Excluída de vez é conflito**, e não «não encontrado»: quem leu a linha há
    um minuto precisa saber que ela foi apagada, e não que o rowid nunca
    existiu.

  17 testes novos — 10 no motor, 7 no protocolo —, e a tela conferida no
  navegador: com a ficha aberta, uma gravação alheia na cidade e a minha no
  telefone, o registro terminou com **as duas**.

### Mudado

- **O erro do protocolo chega inteiro à tela.** O `api()` da interface jogava
  fora `nome`, `codigo` e `classe` e guardava só o texto — então distinguir um
  conflito de qualquer outra recusa exigiria comparar a **redação** da
  mensagem, e melhorar essa redação quebraria a tela sem ninguém notar.

---

'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo + alvo, 1)
p.write_text(s)
print("ok")
