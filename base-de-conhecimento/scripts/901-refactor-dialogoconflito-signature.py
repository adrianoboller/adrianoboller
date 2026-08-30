# Refactor dialogoConflito signature
# 28/08 23:56

import pathlib
p = pathlib.Path("crates/phxsql-server/ui/index.html")
s = p.read_text()

s = s.replace(
'''/** Mostra os três valores e grava a escolha. */
async function dialogoConflito(db, tab, rowid, editaveis, lida, meus, aoTerminar) {''',
'''/** Mostra os três valores e grava a escolha.
 *
 * `ctx`: `{ editaveis, lida, versaoLida, meus, aoTerminar }` — as colunas do
 * formulário, a linha como você a leu, a versão que ela tinha, o que você ia
 * gravar, e o que fazer depois. Vai num objeto e não em sete parâmetros
 * soltos porque a caixa se chama de novo quando um terceiro grava no meio, e
 * uma lista posicional dessa altura erra calada na recursão. */
async function dialogoConflito(db, tab, rowid, ctx) {
  const { editaveis, lida, versaoLida, meus, aoTerminar } = ctx;''', 1)

s = s.replace('''        ${lida.__v || "anterior"} e a atual é a ${atual.versao}</div>''',
              '''        ${versaoLida} e a atual é a ${atual.versao}</div>''', 1)

s = s.replace(
'''      if (err.nome === "CONFLITO") {
        fechar();
        return dialogoConflito(db, tab, rowid, editaveis, outra, escolhido, aoTerminar);
      }''',
'''      if (err.nome === "CONFLITO") {
        fechar();
        return dialogoConflito(db, tab, rowid, { editaveis, lida: outra,
          versaoLida: atual.versao, meus: escolhido, aoTerminar });
      }''', 1)

s = s.replace(
'''        return dialogoConflito(db, tab, rowid, editaveis, linha, valores(), () => {
          est.esquemaAtual = null;
          verConteudoEditavel(db, tab);
        });''',
'''        return dialogoConflito(db, tab, rowid, {
          editaveis, lida: linha, versaoLida: versao, meus: valores(),
          aoTerminar: () => { est.esquemaAtual = null; verConteudoEditavel(db, tab); }
        });''', 1)
p.write_text(s)
print("ok")
