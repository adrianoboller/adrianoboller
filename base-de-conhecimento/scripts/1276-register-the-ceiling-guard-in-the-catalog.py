# Register the ceiling guard in the catalog
# 30/08 06:40

p='bancada/guardas/catalogo.py'
s=open(p,encoding='utf-8').read()
entrada = '''    # 33. O teto do registro do fio, que a integracao quase perdeu
    # -----------------------------------------------------------------------
    {
        "id": "fio-sem-teto-de-registro",
        "titulo": "a leitura do fio volta a ser ilimitada",
        "porque": (
            "o teto nasceu na replica, num `read_line` com `take`; a frente da "
            "cifra trocou aquele `read_line` pelo `Canal`, que lia sem teto. "
            "Juntar as duas sem olhar devolveria a leitura ilimitada, com quem "
            "escolhe a memoria deste lado sendo o outro lado do fio. O teto "
            "desceu para o `Canal` porque la ele vale para o caminho cifrado e "
            "para o claro. A assercao e sobre QUANTO foi lido: conferir so o "
            "veredito passava com o defeito reposto, porque a conferencia vem "
            "depois da leitura -- e ai a memoria ja foi gasta."
        ),
        "arquivo": "crates/phxsql-core/src/fio.rs",
        "trecho": """        let lidos = {
            let mut limitado = <&mut L as std::io::Read>::take(leitor, teto + 1);
            limitado.read_line(&mut linha)?
        };
""",
        "troca": """        // DEFEITO REPOSTO: a leitura volta a ser ilimitada.
        let lidos = leitor.read_line(&mut linha)?;
""",
        "pacote": "phxsql-core",
        "alvo": ["--lib"],
        "caem": [
            "fio::testes::o_teto_do_registro_para_a_leitura_e_nao_so_recusa_depois",
        ],
    },
'''
assert s.rstrip().endswith(']')
i=s.rstrip()[:-1].rstrip('\n')
open(p,'w',encoding='utf-8').write(i+'\n'+entrada+']\n')
print("guarda do teto do fio registrada")
