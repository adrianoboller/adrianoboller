# Apply the self-observation fix
# 28/08 23:02

import pathlib
p = pathlib.Path("crates/phxsql-server/src/profiler.rs")
s = p.read_text()
antigo = """    fn aceita(&self, op: &str, usuario: &str, database: &str) -> bool {
        if !self.database.is_empty()"""
novo = """    fn aceita(&self, op: &str, usuario: &str, database: &str) -> bool {
        // A LEITURA do proprio profiler nunca entra. A tela pergunta uma vez
        // por segundo enquanto esta aberta, e sem esta linha o profiler
        // encheria de si mesmo -- em poucos minutos o anel seria so ele, e o
        // pedido que alguem estava procurando teria saido pela borda.
        // `profiler_ligar` e `profiler_desligar` entram: sao raros e dizem
        // quem mexeu na observacao.
        if op == "profiler" {
            return false;
        }
        if !self.database.is_empty()"""
assert antigo in s
s = s.replace(antigo, novo)
antigo = """    #[test]
    fn o_filtro_separa_por_banco_usuario_e_operacao() {"""
novo = """    /// O profiler nao observa a si mesmo. Sem isto, a tela aberta enche o
    /// anel com as proprias perguntas e empurra para fora o que se procurava.
    #[test]
    fn a_leitura_do_profiler_nao_entra_no_anel() {
        let mut p = Profiler::default();
        p.ligar(Filtro::default(), "", 100, 0).unwrap();
        assert!(p.chegou("{}", "profiler", "adm", "", "", "ip", 0).is_none());
        assert!(p.chegou("{}", "profiler_ligar", "adm", "", "", "ip", 0).is_some());
        assert!(p.chegou("{}", "profiler_desligar", "adm", "", "", "ip", 0).is_some());
        assert_eq!(p.observados(), 2);
    }

    #[test]
    fn o_filtro_separa_por_banco_usuario_e_operacao() {"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
