# Add Nivel::Nenhum to preserve deny-by-default
# 27/08 21:18

p='crates/phxsql-server/src/usuarios.rs'
s=open(p).read()
s=s.replace('''#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Nivel {
    /// So le. E o padrao quando nao se diz nada: nega por omissao.
    #[default]
    Leitor,''','''#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Nivel {
    /// Nada. E o padrao quando o `config.json` nao diz nivel nenhum.
    ///
    /// Existe porque a regra do projeto e negar por omissao, e sem este nivel
    /// o padrao viraria "le tudo" -- todo config que ja existe passaria a dar
    /// leitura em base que antes negava. Um teste antigo pegou exatamente
    /// isso, e este nivel e a correcao.
    #[default]
    Nenhum,
    /// So le.
    Leitor,''')
s=s.replace('''            "" | "leitor" | "consulta" | "leitura" => Nivel::Leitor,''',
'''            "" | "nenhum" | "nada" => Nivel::Nenhum,
            "leitor" | "consulta" | "leitura" => Nivel::Leitor,''')
s=s.replace('''    pub fn nome(self) -> &'static str {
        match self {
            Nivel::Leitor => "leitor",''','''    pub fn nome(self) -> &'static str {
        match self {
            Nivel::Nenhum => "nenhum",
            Nivel::Leitor => "leitor",''')
s=s.replace('''    /// O que este nivel pode, numa base.
    pub fn permissoes(self) -> Permissoes {
        let mut p = Permissoes {
            ler: true,
            diario: true,
            verificar: true,
            ..Permissoes::default()
        };''','''    /// O que este nivel pode, numa base.
    pub fn permissoes(self) -> Permissoes {
        if self == Nivel::Nenhum {
            return Permissoes::default();
        }
        let mut p = Permissoes {
            ler: true,
            diario: true,
            verificar: true,
            ..Permissoes::default()
        };''')
# o teste do nivel: sem nivel agora e Nenhum
s=s.replace('''    #[test]
    fn sem_nivel_o_padrao_e_o_menor() {''','''    #[test]
    fn sem_nivel_o_padrao_nega_tudo() {''')
s=s.replace('''        assert_eq!(ze.nivel, Nivel::Leitor);
        assert!(!ze.e_admin());
        assert!(ze.pode("Qualquer", Atividade::Ler));
        assert!(!ze.pode("Qualquer", Atividade::Excluir));''','''        assert_eq!(ze.nivel, Nivel::Nenhum);
        assert!(!ze.e_admin());
        // Nada. Config que nao diz nivel nao ganha poder nenhum de brinde --
        // e o que faz esta mudanca nao alterar nenhum config que ja existe.
        for a in Atividade::TODAS {
            assert!(!ze.pode("Qualquer", a), "sem nivel deu {}", a.nome());
        }''')
s=s.replace('''    fn cada_nivel_contem_o_anterior() {
        let leitor = Nivel::Leitor.permissoes();''','''    fn cada_nivel_contem_o_anterior() {
        let nenhum = Nivel::Nenhum.permissoes();
        let leitor = Nivel::Leitor.permissoes();''')
s=s.replace('''        for a in Atividade::TODAS {
            if leitor.pode(a) {''','''        for a in Atividade::TODAS {
            assert!(!nenhum.pode(a), "nenhum deu {}", a.nome());
            if leitor.pode(a) {''')
s=s.replace('''        assert_eq!(Nivel::de_texto("").unwrap(), Nivel::Leitor);''',
            '''        assert_eq!(Nivel::de_texto("").unwrap(), Nivel::Nenhum);
        assert_eq!(Nivel::de_texto("nenhum").unwrap(), Nivel::Nenhum);
        assert_eq!(Nivel::de_texto("leitor").unwrap(), Nivel::Leitor);''')
open(p,'w').write(s)
