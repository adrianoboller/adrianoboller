# Portões únicos antes do commit

**Regra.** Tudo o que reprova um commit roda num comando só, que sai com UM
código: formatação, linter com aviso virando erro, suíte inteira e as catracas
que moram fora da suíte. Roda os passos todos mesmo depois de um vermelho, e
imprime o quadro inteiro. Suíte verde com catraca vermelha é **vermelho**.

**Cicatriz.** Uma árvore passou por formatação, linter com zero avisos e 2.739
testes verdes — e subiu com uma catraca reprovada (25 contra teto 24). As
catracas escritas como teste rodavam dentro da suíte; as escritas como script
moravam fora e só rodavam se alguém lembrasse. E o linter sem `-D warnings`
imprime o aviso e sai 0: o «zero avisos» dependia de alguém ler.

**Como aplicar.**
- Um script na raiz (`scripts/portoes.sh` deste kit) com a lista dos passos.
- Opção `--raiz DIR` para rodar na **árvore exata do commit** (montada por
  `git archive`), não na de trabalho, que pode ter arquivo não versionado que
  faz passar.
- O linter com a flag que transforma aviso em código de saída.
