# Add visao to op_varrer
# 28/08 17:49

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()
velho='''        let rowids: Vec<u64> = if indice.is_empty() {
            t.varrer()?.into_iter().map(|(r, _)| r).collect()
        } else {
            t.varrer_indice(&indice)?
        };'''
novo='''        // `visao` decide o que a varredura enxerga. O padrao e "ativas": a
        // linha marcada como excluida some das listas, senao marcar nao teria
        // efeito nenhum.
        let visao = match p.texto_ou("visao", "ativas").trim() {
            "" | "ativas" | "ativos" => Visao::Ativas,
            "excluidas" | "excluidos" => Visao::Excluidas,
            "todas" | "todos" => Visao::Todas,
            outro => {
                return Err(PhxError::Esquema(format!(
                    "visao {outro:?} nao existe; use ativas, excluidas ou todas"
                )))
            }
        };

        let rowids: Vec<u64> = if indice.is_empty() {
            t.varrer_com(visao)?.into_iter().map(|(r, _)| r).collect()
        } else {
            // O indice devolve rowid, e a marca esta no registro: pela ordem
            // do indice a filtragem custa uma leitura por linha. E o preco de
            // pedir ordenado -- e por isso `Todas` nao paga nada.
            let todos = t.varrer_indice(&indice)?;
            match visao {
                Visao::Todas => todos,
                Visao::Ativas => t.filtrar_ativos(&todos)?,
                Visao::Excluidas => {
                    let ativos = t.filtrar_ativos(&todos)?;
                    todos.into_iter().filter(|r| !ativos.contains(r)).collect()
                }
            }
        };'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
