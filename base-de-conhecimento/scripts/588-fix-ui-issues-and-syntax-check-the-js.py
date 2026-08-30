# Fix UI issues and syntax-check the JS
# 28/08 17:46

import io
p='crates/phxsql-server/ui/index.html'
s=io.open(p,encoding='utf-8').read()

# 1. $$ nao aceita raiz
s=s.replace('$$("#modosExc .modo", fundo)','[...fundo.querySelectorAll(".modo")]')

# 2. celulaValor ja devolve o <td> inteiro
s=s.replace('''               : cols.map(c => `<td>${celulaValor((d_.linha || {})[c.nome])}</td>`).join("")}''',
            '''               : cols.map(c => celulaValor((d_.linha || {})[c.nome])).join("")}''',1)

# 3. o catch da lixeira, escrito direito
velho='''  } catch (e) {
    return folha(`Lixeira de ${tab}`, esc(db),
      `<div class="aviso mal">${esc(String(e))}</div>
       <p class="leg">O <code>.trash</code> guarda o dado que alguém mandou
         apagar. Ver o conteúdo dele exige <b>administrar</b> — quem só tem
         <code>ler</code> perdeu o direito àquela linha no instante em que ela
         foi excluída, e a lixeira devolveria o direito por outra porta.</p>
       <div class="acoes">
         <button class="botao secundario" id="btVoltaLix">← Gerir tabelas</button>
       </div>`) || ($("#btVoltaLix").onclick = () => gerirTabelasAtual());
  }'''
novo='''  } catch (e) {
    folha(`Lixeira de ${tab}`, esc(db),
      `<div class="aviso mal">${esc(String(e))}</div>
       <p class="leg">O <code>.trash</code> guarda o dado que alguém mandou
         apagar. Ver o conteúdo dele exige <b>administrar</b> — quem só tem
         <code>ler</code> perdeu o direito àquela linha no instante em que ela
         foi excluída, e a lixeira devolveria o direito por outra porta.</p>
       <div class="acoes">
         <button class="botao secundario" id="btVoltaLix">← Gerir tabelas</button>
       </div>`);
    $("#btVoltaLix").onclick = () => gerirTabelasAtual();
    return;
  }'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
