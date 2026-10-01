//! Renderizador Flutter do mesmo UI-IR: um projeto com `lib/ui.dart` (tempo de execucao
//! fixo), `lib/telas.dart` (gerado, um widget por tela) e `test/telas_test.dart` (gerado:
//! o projeto nasce com a prova de que o mestre/detalhe soma). As regras de tela sao as
//! mesmas do HTML e do React: dinheiro e data com mascara pt-BR, data pela regra do
//! `DateValid` (juliano ate 04/10/1582), total recalculado a cada digito.

use crate::ir::{App, Screen};
use serde_json::{Value, json};

pub fn render(app: &App) -> Vec<(String, String)> {
    // o JSON embutido ja sai com secoes e colunas na ordem lida (v2): o app nao a refaz
    let app = &app.com_layout();
    vec![
        ("pubspec.yaml".into(), pubspec(app)),
        // proprio, para o `flutter create` nao escrever um que inclui o pacote
        // flutter_lints, do qual o projeto nao depende (o analyze acusava)
        (
            "analysis_options.yaml".into(),
            "# Regras padrao do analisador do Dart; sem pacote de lints externo.\n".into(),
        ),
        ("lib/ui.dart".into(), UI_DART.trim_start().into()),
        ("lib/main.dart".into(), MAIN_DART.trim_start().into()),
        ("lib/telas.dart".into(), telas(app)),
        ("test/telas_test.dart".into(), teste(app)),
        // `flutter create` escreve um widget_test.dart do contador de exemplo, que nao
        // compila contra este app; o arquivo ja existindo, o create nao o sobrescreve
        (
            "test/widget_test.dart".into(),
            "// O teste deste projeto e o telas_test.dart, gerado junto.\nvoid main() {}\n".into(),
        ),
        (
            "LEIA-ME.md".into(),
            "# Telas geradas do UI-IR\n\n```bash\nflutter create --platforms web,windows,linux,macos,android,ios .\nflutter test\nflutter run -d chrome\n```\n".into(),
        ),
    ]
}

fn nome_pacote(app: &App) -> String {
    let n: String = app
        .name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("{}_erp", n.trim_matches('_'))
}

fn pubspec(app: &App) -> String {
    format!(
        "name: {}\ndescription: Telas ERP geradas do UI-IR pelo PhxClaw.\npublish_to: none\nversion: 0.1.0\n\n\
         environment:\n  sdk: ^3.5.0\n\ndependencies:\n  flutter:\n    sdk: flutter\n\n\
         dev_dependencies:\n  flutter_test:\n    sdk: flutter\n\nflutter:\n  uses-material-design: true\n",
        nome_pacote(app)
    )
}

/// Literal Dart a partir de JSON: JSON valido e expressao Dart valida, exceto o `$`,
/// que em cadeia Dart interpola.
fn dart(v: &Value) -> String {
    serde_json::to_string(v)
        .unwrap_or_else(|_| "null".into())
        .replace('$', "\\$")
}

fn classe(id: &str) -> String {
    let mut s = String::from("Tela");
    for p in id.split(|c: char| !c.is_ascii_alphanumeric()) {
        let mut c = p.chars();
        if let Some(f) = c.next() {
            s.extend(f.to_uppercase());
            s.push_str(c.as_str());
        }
    }
    s
}

fn telas(app: &App) -> String {
    let rotulos: serde_json::Map<String, Value> = app
        .entities
        .iter()
        .map(|e| (e.name.clone(), Value::String(e.label.clone())))
        .collect();
    let campos: serde_json::Map<String, Value> = app
        .entities
        .iter()
        .map(|e| {
            let m: serde_json::Map<String, Value> = e
                .fields
                .iter()
                .map(|f| {
                    (
                        f.name.clone(),
                        serde_json::to_value(f).unwrap_or(Value::Null),
                    )
                })
                .collect();
            (e.name.clone(), Value::Object(m))
        })
        .collect();
    let mut s = format!(
        "// GERADO do UI-IR v{} pelo PhxClaw. Um widget por tela.\n\
         import 'package:flutter/widgets.dart';\nimport 'ui.dart';\n\n\
         const nomeApp = {};\nconst versaoIr = {};\n\
         const Map<String, dynamic> rotulos = {};\n\
         const Map<String, dynamic> campos = {};\n\n",
        app.ir_version,
        dart(&json!(app.name)),
        app.ir_version,
        dart(&Value::Object(rotulos)),
        dart(&Value::Object(campos))
    );
    let mut lista = vec![];
    for t in &app.screens {
        let c = classe(t.id());
        let desc = dart(&serde_json::to_value(t).unwrap_or(Value::Null));
        let corpo = match t {
            Screen::List { entity, .. } => format!(
                "Consulta(tela: tela, campos: campos[{}], aoIncluir: aoAbrir)",
                dart(&json!(entity))
            ),
            Screen::Form { entity, .. } => format!(
                "Cadastro(tela: tela, campos: campos[{}], rotulos: rotulos)",
                dart(&json!(entity))
            ),
            Screen::MasterDetail { master, detail, .. } => {
                let itens = app
                    .entities
                    .iter()
                    .find(|e| &e.name == detail)
                    .map(|e| e.label_plural.clone())
                    .unwrap_or_else(|| detail.clone());
                format!(
                    "MestreDetalhe(tela: tela, campos: campos[{}], camposDet: campos[{}], rotulos: rotulos, rotuloItens: {})",
                    dart(&json!(master)),
                    dart(&json!(detail)),
                    dart(&json!(itens))
                )
            }
        };
        s.push_str(&format!(
            "class {c} extends StatelessWidget {{\n  const {c}({{super.key, required this.aoAbrir}});\n  \
             final void Function(String) aoAbrir;\n  static const Map<String, dynamic> tela = {desc};\n  \
             @override\n  Widget build(BuildContext context) => {corpo};\n}}\n\n"
        ));
        lista.push(format!(
            "  Tela({}, {}, (abrir) => {c}(aoAbrir: abrir)),",
            dart(&json!(t.id())),
            dart(&json!(t.title()))
        ));
    }
    let menu: Vec<Value> = app
        .menu
        .iter()
        .map(|g| json!({"title": g.title, "itens": g.screens}))
        .collect();
    s.push_str(&format!(
        "final List<Tela> telas = [\n{}\n];\n\nconst List<dynamic> menu = {};\n",
        lista.join("\n"),
        dart(&json!(menu))
    ));
    s
}

/// Teste gerado: para cada mestre/detalhe com total, dois itens somados e a data
/// inexistente recusada. O projeto se prova sozinho, sem depender deste repositorio.
fn teste(app: &App) -> String {
    let pacote = nome_pacote(app);
    let mut casos = String::new();
    for t in &app.screens {
        if let Screen::MasterDetail {
            id,
            totals,
            header,
            master,
            ..
        } = t
            && let Some(tot) = totals.first()
        {
            let data = header.iter().flat_map(|s| &s.fields).find(|f| {
                app.entities
                    .iter()
                    .find(|e| &e.name == master)
                    .and_then(|e| e.fields.iter().find(|x| &x.name == *f))
                    .is_some_and(|x| matches!(x.widget, crate::ir::Widget::Date))
            });
            let prova_data = data
                .map(|d| {
                    format!(
                        "    await tester.enterText(find.byKey(const Key('{id}-{d}')), '31022026');\n    \
                         await tester.pump();\n    \
                         expect(find.text('31/02/2026'), findsOneWidget);\n    \
                         expect(find.text('Data inexistente'), findsOneWidget);\n"
                    )
                })
                .unwrap_or_default();
            casos.push_str(&format!(
                "  testWidgets('{id}: itens somam e data inexistente e recusada', (tester) async {{\n    \
                 tester.view.physicalSize = const Size(1400, 1600);\n    \
                 tester.view.devicePixelRatio = 1.0;\n    \
                 addTearDown(tester.view.reset);\n    \
                 await tester.pumpWidget(const AppErp(inicial: '{id}'));\n    \
                 await tester.tap(find.byKey(const Key('{id}-add-item')));\n    await tester.pump();\n    \
                 await tester.tap(find.byKey(const Key('{id}-add-item')));\n    await tester.pump();\n    \
                 await tester.enterText(find.byKey(const Key('{id}-det-{c}-1')), '10,50');\n    \
                 await tester.enterText(find.byKey(const Key('{id}-det-{c}-2')), '1.234,25');\n    \
                 await tester.pump();\n    \
                 expect(find.text(brl(1244.75)), findsOneWidget);\n    \
                 expect(brl(1244.75), 'R\\$ 1.244,75');\n    \
                 await tester.tap(find.byKey(const Key('{id}-del-1')));\n    await tester.pump();\n    \
                 expect(find.text('R\\$ 1.234,25'), findsOneWidget);\n{prova_data}  }});\n\n",
                c = tot.sum_of
            ));
        }
    }
    format!(
        "// GERADO pelo PhxClaw: prova do projeto gerado.\n\
         import 'package:flutter/material.dart';\nimport 'package:flutter_test/flutter_test.dart';\n\
         import 'package:{pacote}/main.dart';\nimport 'package:{pacote}/ui.dart';\n\n\
         void main() {{\n  test('calendario segue o DateValid', () {{\n    \
         expect(dataOk('29/02/1500', false), isTrue);\n    \
         expect(dataOk('10/10/1582', false), isFalse);\n    \
         expect(dataOk('29/02/1900', false), isFalse);\n    \
         expect(dataOk('29/02/2000', false), isTrue);\n    \
         expect(mascaraData('15032026', false), '15/03/2026');\n  }});\n\n{casos}}}\n"
    )
}

const MAIN_DART: &str = r##"
import 'package:flutter/material.dart';
import 'telas.dart';
import 'ui.dart';

void main() {
  // na web, #id_da_tela abre direto nela (como o HTML e o React gerados)
  final hash = Uri.base.fragment;
  runApp(AppErp(inicial: hash.isEmpty ? null : hash));
}

class AppErp extends StatefulWidget {
  const AppErp({super.key, this.inicial});
  final String? inicial;
  @override
  State<AppErp> createState() => _AppErpState();
}

class _AppErpState extends State<AppErp> {
  late String atual = telas.any((t) => t.id == widget.inicial) ? widget.inicial! : telas.first.id;

  void abrir(String id) => setState(() => atual = id);

  @override
  Widget build(BuildContext context) {
    final tela = telas.firstWhere((t) => t.id == atual, orElse: () => telas.first);
    return MaterialApp(
      title: nomeApp,
      debugShowCheckedModeBanner: false,
      theme: temaErp(Brightness.light),
      darkTheme: temaErp(Brightness.dark),
      home: Scaffold(
        body: Row(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
          SizedBox(
            width: 240,
            child: Builder(builder: (context) => ListView(padding: const EdgeInsets.all(16), children: [
              Text(nomeApp, style: TextStyle(fontSize: 18, fontWeight: FontWeight.w700, color: Cores.ouro(context))),
              for (final g in menu) ...[
                Padding(
                  padding: const EdgeInsets.only(top: 16, bottom: 6),
                  child: Text((g['title'] as String).toUpperCase(),
                      style: TextStyle(fontSize: 11, letterSpacing: 1.3, color: Cores.fraco(context))),
                ),
                for (final id in (g['itens'] as List))
                  for (final t in telas.where((t) => t.id == id))
                    ListTile(
                      dense: true,
                      selected: t.id == atual,
                      title: Text(t.titulo),
                      onTap: () => abrir(t.id),
                    ),
              ],
              Padding(
                padding: const EdgeInsets.only(top: 24),
                child: Text('Protótipo Flutter gerado do UI-IR v$versaoIr pelo PhxClaw',
                    style: TextStyle(fontSize: 11, color: Cores.fraco(context))),
              ),
            ])),
          ),
          Expanded(
            child: SingleChildScrollView(
              key: ValueKey(tela.id),
              padding: const EdgeInsets.fromLTRB(28, 24, 28, 24),
              child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                Text(tela.titulo, style: const TextStyle(fontSize: 22, fontWeight: FontWeight.w700)),
                const SizedBox(height: 16),
                tela.construir(abrir),
              ]),
            ),
          ),
        ]),
      ),
    );
  }
}
"##;

const UI_DART: &str = r##"
// Tempo de execucao das telas geradas. Mesmas regras do HTML e do React gerados.
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

class Tela {
  const Tela(this.id, this.titulo, this.construir);
  final String id;
  final String titulo;
  final Widget Function(void Function(String)) construir;
}

double numBr(String? v) =>
    double.tryParse((v ?? '').replaceAll('.', '').replaceAll(',', '.')) ?? 0;

/// R$ 1.234,56 -- feito a mao: sem o pacote intl, e com arredondamento em centavos.
String brl(double n) {
  final c = (n.abs() * 100).round();
  final inteiro = (c ~/ 100).toString();
  final cent = (c % 100).toString().padLeft(2, '0');
  final b = StringBuffer();
  for (var i = 0; i < inteiro.length; i++) {
    if (i > 0 && (inteiro.length - i) % 3 == 0) b.write('.');
    b.write(inteiro[i]);
  }
  return 'R\$ ${n < 0 ? '-' : ''}$b,$cent';
}

String mascaraData(String v, bool dh) {
  var d = v.replaceAll(RegExp(r'\D'), '');
  if (d.length > (dh ? 12 : 8)) d = d.substring(0, dh ? 12 : 8);
  var r = d.substring(0, d.length < 2 ? d.length : 2);
  if (d.length > 2) r += '/${d.substring(2, d.length < 4 ? d.length : 4)}';
  if (d.length > 4) r += '/${d.substring(4, d.length < 8 ? d.length : 8)}';
  if (dh && d.length > 8) {
    r += ' ${d.substring(8, d.length < 10 ? d.length : 10)}';
    if (d.length > 10) r += ':${d.substring(10)}';
  }
  return r;
}

/// A regra do DateValid do WLanguage (Help 3027003), a mesma do Rust gerado: anos 1 a
/// 9999, juliano ate 04/10/1582, os dias 05-14/10/1582 nao existem, gregoriano depois.
bool dataValida(int dia, int mes, int ano) {
  if (ano < 1 || ano > 9999) return false;
  final k = ano * 10000 + mes * 100 + dia;
  if (k >= 15821005 && k <= 15821014) return false;
  final bis = k < 15821005 ? ano % 4 == 0 : (ano % 4 == 0 && ano % 100 != 0) || ano % 400 == 0;
  const dias = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
  if (mes < 1 || mes > 12) return false;
  final max = mes == 2 && bis ? 29 : dias[mes - 1];
  return dia >= 1 && dia <= max;
}

bool dataOk(String v, bool dh) {
  final m = RegExp(dh ? r'^(\d{2})/(\d{2})/(\d{4}) (\d{2}):(\d{2})$' : r'^(\d{2})/(\d{2})/(\d{4})$')
      .firstMatch(v);
  if (m == null) return false;
  final ok = dataValida(int.parse(m[1]!), int.parse(m[2]!), int.parse(m[3]!));
  return ok && (!dh || (int.parse(m[4]!) < 24 && int.parse(m[5]!) < 60));
}

class MascaraDataFormatter extends TextInputFormatter {
  MascaraDataFormatter(this.dh);
  final bool dh;
  @override
  TextEditingValue formatEditUpdate(TextEditingValue a, TextEditingValue n) {
    final t = mascaraData(n.text, dh);
    return TextEditingValue(text: t, selection: TextSelection.collapsed(offset: t.length));
  }
}

class Cores {
  static bool _escuro(BuildContext c) => Theme.of(c).brightness == Brightness.dark;
  static Color inclui(BuildContext c) => _escuro(c) ? const Color(0xFF3ECF8E) : const Color(0xFF137A4B);
  static Color altera(BuildContext c) => _escuro(c) ? const Color(0xFFF2C14E) : const Color(0xFF8A6100);
  static Color exclui(BuildContext c) => _escuro(c) ? const Color(0xFFFF5D5D) : const Color(0xFFB3261E);
  static Color consulta(BuildContext c) => _escuro(c) ? const Color(0xFF4FB3FF) : const Color(0xFF0B5CAD);
  static Color ouro(BuildContext c) => _escuro(c) ? const Color(0xFFF5C64D) : const Color(0xFF8A5A00);
  static Color fraco(BuildContext c) => _escuro(c) ? const Color(0xFF8AA4B8) : const Color(0xFF51606E);
  static Color porTipo(BuildContext c, String kind) => switch (kind) {
        'include' => inclui(c),
        'alter' => altera(c),
        'delete' => exclui(c),
        _ => consulta(c),
      };
}

ThemeData temaErp(Brightness b) {
  final escuro = b == Brightness.dark;
  return ThemeData(
    brightness: b,
    scaffoldBackgroundColor: escuro ? const Color(0xFF07121D) : const Color(0xFFF4F6F9),
    colorScheme: ColorScheme.fromSeed(seedColor: const Color(0xFF4FB3FF), brightness: b),
    inputDecorationTheme: const InputDecorationTheme(isDense: true, border: OutlineInputBorder()),
  );
}

/// Botao de acao: contorno, nunca fundo cheio (a convencao das cores de acao).
Widget botaoAcao(BuildContext c, String rotulo, String kind, VoidCallback? f, {Key? key}) =>
    OutlinedButton(
      key: key,
      onPressed: f ?? () {},
      style: OutlinedButton.styleFrom(
        foregroundColor: Cores.porTipo(c, kind),
        side: BorderSide(color: Cores.porTipo(c, kind)),
      ),
      child: Text(rotulo),
    );

class Controle extends StatelessWidget {
  const Controle({super.key, required this.f, required this.rotulos, required this.valor,
      required this.muda, this.semRotulo = false});
  final Map<String, dynamic> f;
  final Map<String, dynamic> rotulos;
  final TextEditingController valor;
  final VoidCallback muda;
  final bool semRotulo;

  @override
  Widget build(BuildContext context) {
    final w = f['widget'] as Map<String, dynamic>;
    final obrig = f['required'] == true;
    final so = f['readonly'] == true;
    final rot = semRotulo ? null : '${f['label']}${obrig ? ' *' : ''}';
    InputDecoration dec({String? hint, String? prefix}) =>
        InputDecoration(labelText: rot, hintText: hint, prefixText: prefix);
    switch (w['kind']) {
      case 'select':
        return DropdownButtonFormField<String>(
          isExpanded: true,
          decoration: dec(),
          hint: const Text('Selecione'),
          items: [for (final o in (w['options'] as List)) DropdownMenuItem(value: o as String, child: Text(o))],
          onChanged: so ? null : (v) {
            valor.text = v ?? '';
            muda();
          },
        );
      case 'lookup':
        final r = (rotulos[w['entity']] ?? w['entity']).toString().toLowerCase();
        return Row(children: [
          Expanded(
            child: DropdownButtonFormField<String>(
              isExpanded: true,
              decoration: dec(),
              hint: Text('Selecione $r'),
              items: const [],
              onChanged: null,
            ),
          ),
          IconButton(tooltip: 'Pesquisar $r', onPressed: () {}, icon: const Icon(Icons.search)),
        ]);
      case 'checkbox':
        return StatefulBuilder(builder: (context, set) => CheckboxListTile(
              contentPadding: EdgeInsets.zero,
              title: Text(f['label'] as String),
              value: valor.text == 'true',
              onChanged: so ? null : (v) => set(() {
                    valor.text = '${v == true}';
                    muda();
                  }),
            ));
      case 'date':
      case 'date_time':
        final dh = w['kind'] == 'date_time';
        return TextFormField(
          controller: valor,
          readOnly: so,
          keyboardType: TextInputType.number,
          inputFormatters: [MascaraDataFormatter(dh)],
          autovalidateMode: AutovalidateMode.onUserInteraction,
          validator: (v) => (v ?? '').length == (dh ? 16 : 10) && !dataOk(v!, dh) ? 'Data inexistente' : null,
          decoration: dec(hint: dh ? 'dd/mm/aaaa hh:mm' : 'dd/mm/aaaa'),
          onChanged: (_) => muda(),
        );
      case 'money':
        return TextFormField(
          controller: valor,
          readOnly: so,
          textAlign: TextAlign.right,
          keyboardType: const TextInputType.numberWithOptions(decimal: true),
          decoration: dec(hint: '0,00', prefix: 'R\$ '),
          onChanged: (_) => muda(),
        );
      default:
        final longo = w['kind'] == 'text_area';
        return TextFormField(
          controller: valor,
          readOnly: so,
          maxLines: longo ? 3 : 1,
          maxLength: f['max_len'] as int?,
          keyboardType: switch (w['kind']) {
            'integer' || 'decimal' => TextInputType.number,
            'email' => TextInputType.emailAddress,
            'phone' => TextInputType.phone,
            _ => TextInputType.text,
          },
          decoration: dec().copyWith(counterText: ''),
          onChanged: (_) => muda(),
        );
    }
  }
}

class Secoes extends StatelessWidget {
  const Secoes({super.key, required this.tela, required this.secoes, required this.campos,
      required this.rotulos, required this.valores, required this.muda});
  final String tela;
  final List secoes;
  final Map<String, dynamic> campos;
  final Map<String, dynamic> rotulos;
  final TextEditingController Function(String) valores;
  final VoidCallback muda;

  @override
  Widget build(BuildContext context) => Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        for (final s in secoes)
          Card(
            margin: const EdgeInsets.only(bottom: 14),
            child: Padding(
              padding: const EdgeInsets.all(14),
              child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                Text(s['title'] as String, style: TextStyle(fontSize: 12, color: Cores.fraco(context))),
                const SizedBox(height: 10),
                Wrap(spacing: 14, runSpacing: 12, children: [
                  for (final n in (s['fields'] as List))
                    if (campos[n] != null)
                      SizedBox(
                        width: (campos[n]['widget']['kind'] == 'text_area') ? 860 : 260,
                        child: Controle(
                          key: Key('$tela-$n'),
                          f: campos[n] as Map<String, dynamic>,
                          rotulos: rotulos,
                          valor: valores(n as String),
                          muda: muda,
                        ),
                      ),
                ]),
              ]),
            ),
          ),
      ]);
}

class Consulta extends StatelessWidget {
  const Consulta({super.key, required this.tela, required this.campos, required this.aoIncluir});
  final Map<String, dynamic> tela;
  final Map<String, dynamic> campos;
  final void Function(String) aoIncluir;

  String rot(String n) => (campos[n]?['label'] ?? n) as String;

  @override
  Widget build(BuildContext context) {
    final cols = (tela['columns'] as List).cast<String>();
    return Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
      Wrap(spacing: 14, runSpacing: 12, children: [
        for (final n in (tela['search_fields'] as List))
          SizedBox(width: 260, child: TextField(decoration: InputDecoration(labelText: rot(n as String), prefixIcon: const Icon(Icons.search)))),
      ]),
      const SizedBox(height: 10),
      Wrap(spacing: 8, children: [
        botaoAcao(context, 'Pesquisar', 'query', null),
        botaoAcao(context, 'Incluir', 'include', () => aoIncluir(tela['opens'] as String)),
      ]),
      const SizedBox(height: 10),
      SingleChildScrollView(
        scrollDirection: Axis.horizontal,
        child: DataTable(
          columns: [for (final c in cols) DataColumn(label: Text(rot(c)))],
          rows: const [],
        ),
      ),
      const Padding(padding: EdgeInsets.all(18), child: Text('Nenhum registro. Use Pesquisar ou Incluir.')),
    ]);
  }
}

class Cadastro extends StatefulWidget {
  const Cadastro({super.key, required this.tela, required this.campos, required this.rotulos});
  final Map<String, dynamic> tela;
  final Map<String, dynamic> campos;
  final Map<String, dynamic> rotulos;
  @override
  State<Cadastro> createState() => _CadastroState();
}

class _CadastroState extends State<Cadastro> {
  final Map<String, TextEditingController> _v = {};
  TextEditingController valor(String n) => _v.putIfAbsent(n, TextEditingController.new);

  @override
  void dispose() {
    for (final c in _v.values) {
      c.dispose();
    }
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => Form(
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Secoes(tela: widget.tela['id'] as String, secoes: widget.tela['sections'] as List,
              campos: widget.campos, rotulos: widget.rotulos, valores: valor, muda: () {}),
          Wrap(spacing: 8, children: [
            for (final a in (widget.tela['actions'] as List))
              botaoAcao(context, a['label'] as String, a['kind'] as String, null),
          ]),
        ]),
      );
}

class MestreDetalhe extends StatefulWidget {
  const MestreDetalhe({super.key, required this.tela, required this.campos, required this.camposDet,
      required this.rotulos, required this.rotuloItens});
  final Map<String, dynamic> tela;
  final Map<String, dynamic> campos;
  final Map<String, dynamic> camposDet;
  final Map<String, dynamic> rotulos;
  final String rotuloItens;
  @override
  State<MestreDetalhe> createState() => _MestreDetalheState();
}

class _MestreDetalheState extends State<MestreDetalhe> {
  final Map<String, TextEditingController> _v = {};
  final List<(int, Map<String, TextEditingController>)> _itens = [];
  var _seq = 0;
  TextEditingController valor(String n) => _v.putIfAbsent(n, TextEditingController.new);

  @override
  void dispose() {
    for (final c in _v.values) {
      c.dispose();
    }
    for (final (_, m) in _itens) {
      for (final c in m.values) {
        c.dispose();
      }
    }
    super.dispose();
  }

  double soma(String campo) => _itens.fold(0.0, (s, i) => s + numBr(i.$2[campo]?.text));

  @override
  Widget build(BuildContext context) {
    final id = widget.tela['id'] as String;
    final cols = [
      for (final c in (widget.tela['detail_columns'] as List))
        if (widget.camposDet[c] != null) widget.camposDet[c] as Map<String, dynamic>
    ];
    return Form(
      child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        Secoes(tela: id, secoes: widget.tela['header'] as List, campos: widget.campos,
            rotulos: widget.rotulos, valores: valor, muda: () => setState(() {})),
        Card(
          margin: const EdgeInsets.only(bottom: 14),
          child: Padding(
            padding: const EdgeInsets.all(14),
            child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text(widget.rotuloItens, style: TextStyle(fontSize: 12, color: Cores.fraco(context))),
              const SizedBox(height: 8),
              Row(children: [
                for (final f in cols)
                  Expanded(child: Text(f['label'] as String, style: const TextStyle(fontWeight: FontWeight.w600))),
                const SizedBox(width: 48),
              ]),
              for (final (n, m) in _itens)
                Padding(
                  padding: const EdgeInsets.only(top: 8),
                  child: Row(children: [
                    for (final f in cols)
                      Expanded(
                        child: Padding(
                          padding: const EdgeInsets.only(right: 8),
                          child: Controle(
                            key: Key('$id-det-${f['name']}-$n'),
                            f: f,
                            rotulos: widget.rotulos,
                            valor: m.putIfAbsent(f['name'] as String, TextEditingController.new),
                            muda: () => setState(() {}),
                            semRotulo: true,
                          ),
                        ),
                      ),
                    IconButton(
                      key: Key('$id-del-$n'),
                      tooltip: 'Remover item',
                      color: Cores.exclui(context),
                      icon: const Icon(Icons.close),
                      onPressed: () => setState(() => _itens.removeWhere((i) => i.$1 == n)),
                    ),
                  ]),
                ),
              const SizedBox(height: 10),
              botaoAcao(context, 'Adicionar item', 'include',
                  () => setState(() => _itens.add((++_seq, {}))), key: Key('$id-add-item')),
              Align(
                alignment: Alignment.centerRight,
                child: Wrap(spacing: 24, children: [
                  for (final t in (widget.tela['totals'] as List))
                    Column(crossAxisAlignment: CrossAxisAlignment.end, children: [
                      Text(t['label'] as String, style: TextStyle(fontSize: 12, color: Cores.fraco(context))),
                      Text(brl(soma(t['sum_of'] as String)),
                          key: Key('$id-total-${t['sum_of']}'),
                          style: const TextStyle(fontSize: 18, fontWeight: FontWeight.w700)),
                    ]),
                ]),
              ),
            ]),
          ),
        ),
        const SizedBox(height: 10),
        Wrap(spacing: 8, children: [
          for (final a in (widget.tela['actions'] as List))
            botaoAcao(context, a['label'] as String, a['kind'] as String, null),
        ]),
      ]),
    );
  }
}
"##;
