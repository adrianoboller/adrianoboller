# Unify redirect error, keep spare error
# 29/08 17:19

import re, pathlib
p = pathlib.Path("crates/phxsql-core/src/error.rs")
t = p.read_text()

subs = [
# 1) A variante: fica o Redireciona do cluster (formato recortavel) e entra o
#    SpareEmEspera, que e conceito distinto (recusa ate leitura).
("""            PhxError::Redireciona(_) => 4003,
=======
            PhxError::EscritaNaReplica(_) => 4003,
            PhxError::SpareEmEspera(_) => 4004,
""", """            PhxError::Redireciona(_) => 4003,
            PhxError::SpareEmEspera(_) => 4004,
"""),
("""            PhxError::Redireciona(_) => "REDIRECIONA",
=======
            PhxError::EscritaNaReplica(_) => "ESCRITA_NA_REPLICA",
            PhxError::SpareEmEspera(_) => "SPARE_EM_ESPERA",
""", """            PhxError::Redireciona(_) => "REDIRECIONA",
            PhxError::SpareEmEspera(_) => "SPARE_EM_ESPERA",
"""),
("""            PhxError::Redireciona(String::new()),
=======
            PhxError::EscritaNaReplica(String::new()),
            PhxError::SpareEmEspera(String::new()),
""", """            PhxError::Redireciona(String::new()),
            PhxError::SpareEmEspera(String::new()),
"""),
("""        assert_eq!(PhxError::Redireciona(String::new()).codigo(), 4003);
=======
        assert_eq!(PhxError::EscritaNaReplica(String::new()).codigo(), 4003);
        assert_eq!(PhxError::SpareEmEspera(String::new()).codigo(), 4004);
""", """        assert_eq!(PhxError::Redireciona(String::new()).codigo(), 4003);
        assert_eq!(PhxError::SpareEmEspera(String::new()).codigo(), 4004);
"""),
]
for velho, novo in subs:
    assert velho in t, velho[:60]
    t = t.replace(velho, novo, 1)

# 2) A doc da variante e o Display: fica o texto do Redireciona, entra o do spare.
t = re.sub(r"<<<<<<< [^\n]*\n(    /// O pedido tem de ir para OUTRO.*?)=======\n.*?    SpareEmEspera\(String\),\n>>>>>>> [^\n]*\n",
           lambda m: m.group(1) + """    /// Este servidor e um SPARE de contingencia: nao atende cliente, nem
    /// para ler. Erro proprio porque a acao de quem recebe e outra --
    /// esperar ou promover, nunca insistir.
    SpareEmEspera(String),
""", t, flags=re.S)

t = re.sub(r"<<<<<<< [^\n]*\n(            // Sem prefixo: a mensagem ja comeca.*?)=======\n.*?            PhxError::SpareEmEspera\(m\) => write!\(f, \"spare em espera: \{m\}\"\),\n>>>>>>> [^\n]*\n",
           lambda m: m.group(1) + """            PhxError::SpareEmEspera(m) => write!(f, "spare em espera: {m}"),
""", t, flags=re.S)
p.write_text(t)
print("marcas restantes em error.rs:", t.count("<<<<<<<"))
