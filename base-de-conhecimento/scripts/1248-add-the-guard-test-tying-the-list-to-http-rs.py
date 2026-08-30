# Add the guard test tying the list to http.rs
# 30/08 05:22

p='crates/phxsql-server/src/conferidor.rs'
s=open(p,encoding='utf-8').read()

ancora = '''    /// A prova real do conferidor, com o defeito reposto: um rotulo cravado
    /// tem de ser reprovado, e o MESMO rotulo pela fabrica tem de passar.'''
assert s.count(ancora)==1

guarda = '''    /// A lista do `FONTES` e digitada, e lista digitada envelhece calada: o
    /// `multitela.js` passou a ser servido pelo `http.rs` e ficou de fora
    /// daqui, entao 1.474 linhas de interface nao contavam para a catraca e
    /// ninguem via pelo numero.
    ///
    /// A guarda tira a lista de quem digita e poe em quem serve: le o fonte do
    /// `http.rs` e cobra cada `.js` e `.html` de interface que ele embute.
    /// Quando entrar a proxima tela, este teste reprova antes de a catraca
    /// medir errado.
    #[test]
    fn a_lista_cobre_tudo_que_o_http_serve() {
        const HTTP: &str = include_str!("http.rs");

        let servidos: Vec<&str> = HTTP
            .match_indices("include_str!(\\"../ui/")
            .filter_map(|(i, _)| {
                let resto = &HTTP[i + "include_str!(\\"".len()..];
                let fim = resto.find('"')?;
                let caminho = &resto[..fim];
                // O `.md` do changelog da grade e servido, mas nao e tela.
                (caminho.ends_with(".js") || caminho.ends_with(".html")).then_some(caminho)
            })
            .collect();

        assert!(
            !servidos.is_empty(),
            "nao achei nenhum include_str! de interface no http.rs -- a guarda \\
             ficou cega, conserte o reconhecimento antes de confiar nela"
        );

        let medidos: HashSet<&str> = FONTES.iter().map(|(nome, _)| *nome).collect();
        let faltando: Vec<&str> = servidos
            .iter()
            .filter(|c| !medidos.contains(c.trim_start_matches("../")))
            .copied()
            .collect();

        assert!(
            faltando.is_empty(),
            "o http.rs serve {faltando:?} e o FONTES nao mede -- texto cravado \\
             ali nao conta para a catraca. Acrescente ao FONTES e reveja o TETO"
        );
    }

'''
s=s.replace(ancora, guarda+ancora)
open(p,'w',encoding='utf-8').write(s)
print("guarda a_lista_cobre_tudo_que_o_http_serve acrescentada")
