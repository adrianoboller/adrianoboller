# abrirFicha keeps and sends version
# 28/08 23:54

import pathlib
p = pathlib.Path("crates/phxsql-server/ui/index.html")
s = p.read_text()

alvo = '''  let linha = {};
  if (!novo) {
    const r = await api("ler", { database: db, tabela: tab, rowid });
    if (!r) {
      avisar(`o registro ${rowid} não existe mais`, true);
      return verConteudoEditavel(db, tab);
    }
    linha = r;
  }'''
novo = '''  // A versão da linha no instante da leitura. Ela volta no `atualizar` e é o
  // que faz o servidor recusar a gravação quando outra sessão mexeu no meio —
  // a janela de conflito de escrita. Numa linha nova não há versão: não há
  // nada de ninguém para atropelar.
  let linha = {}, versao = 0;
  if (!novo) {
    const r = await api("ler", { database: db, tabela: tab, rowid, com_versao: true });
    if (!r) {
      avisar(`o registro ${rowid} não existe mais`, true);
      return verConteudoEditavel(db, tab);
    }
    linha = r.linha;
    versao = r.versao;
  }'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

alvo = '''      } else {
        await api("atualizar", { database: db, tabela: tab, rowid, valores: valores() });
        avisar(`registro ${rowid} salvo`);
      }
      est.esquemaAtual = null;
      verConteudoEditavel(db, tab);
    } catch (err) {
      rec.textContent = "";
      avisar(String(err), true);
    }
  };'''
novo = '''      } else {
        await api("atualizar",
          { database: db, tabela: tab, rowid, valores: valores(), versao });
        avisar(`registro ${rowid} salvo`);
      }
      est.esquemaAtual = null;
      verConteudoEditavel(db, tab);
    } catch (err) {
      rec.textContent = "";
      // Conflito não é erro de digitação: alguém gravou primeiro, e o que
      // falta é uma DECISÃO, não uma correção. Em vez do recado vermelho, a
      // comparação lado a lado.
      if (err.nome === "CONFLITO" && !novo) {
        return dialogoConflito(db, tab, rowid, editaveis, linha, valores(), () => {
          est.esquemaAtual = null;
          verConteudoEditavel(db, tab);
        });
      }
      avisar(String(err), true);
    }
  };'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

# o excluir da ficha manda a versão junto
alvo = '''  if (!novo && !marcada) $("#btExcluir").onclick = ev => {
    ev.preventDefault();
    dialogoExcluir(db, tab, rowid, () => {
      est.esquemaAtual = null;
      verConteudoEditavel(db, tab);
    });
  };'''
novo = '''  if (!novo && !marcada) $("#btExcluir").onclick = ev => {
    ev.preventDefault();
    // A versão vai junto: excluir uma linha que outra pessoa acabou de
    // alterar é a mesma janela de conflito, e apagar o trabalho dela sem
    // avisar seria pior do que sobrescrever.
    dialogoExcluir(db, tab, rowid, () => {
      est.esquemaAtual = null;
      verConteudoEditavel(db, tab);
    }, versao);
  };'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ficha ok")
