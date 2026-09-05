<?php
// Captura o golden master RODANDO o legado, em vez de digitar o esperado a
// mao. E a regra do projeto: numero visivel sai de medicao. Se alguem mexer
// nas regras do PHP, o esperado muda junto e a diferenca fica visivel no diff.
// Uso: php capturar-golden.php > inputs/dados-de-amostra/resultados-esperados.json
require_once __DIR__ . '/inputs/legado-php/lib/regras.php';

$casos = array();
function caso(&$casos, $id, $regra, $entrada, $esperado, $obs = null) {
    $c = array('id' => $id, 'regra' => $regra, 'entrada' => $entrada, 'esperado' => $esperado);
    if ($obs !== null) { $c['observacao'] = $obs; }
    $casos[] = $c;
}

$e = calcula_encargos(1000.00, '2026-08-01', '2026-08-31');
caso($casos, 'TST-BR-101-a', 'BR-101 calcula_encargos',
     array('valor' => 1000.00, 'vencimento' => '2026-08-01', 'pagamento' => '2026-08-31'), $e,
     'trinta dias de atraso: multa de 2% mais um mes de juros de 1%');
$e = calcula_encargos(1000.00, '2026-08-01', '2026-08-01');
caso($casos, 'TST-BR-101-b', 'BR-101 calcula_encargos',
     array('valor' => 1000.00, 'vencimento' => '2026-08-01', 'pagamento' => '2026-08-01'), $e,
     'em dia nao tem acrescimo nenhum');
$e = calcula_encargos(748.14, '2026-08-01', '2026-08-16');
caso($casos, 'TST-BR-101-c', 'BR-101 calcula_encargos',
     array('valor' => 748.14, 'vencimento' => '2026-08-01', 'pagamento' => '2026-08-16'), $e,
     'meio mes: juros pro rata die');

caso($casos, 'TST-BR-102-a', 'BR-102 desconto_por_forma',
     array('subtotal' => 3954.00, 'forma' => 'AV'), desconto_por_forma(3954.00, 'AV'));
caso($casos, 'TST-BR-102-b', 'BR-102 desconto_por_forma',
     array('subtotal' => 3954.00, 'forma' => 'PZ'), desconto_por_forma(3954.00, 'PZ'));
caso($casos, 'TST-BR-102-c', 'BR-102 desconto_por_forma',
     array('subtotal' => 3954.00, 'forma' => 'XX'), desconto_por_forma(3954.00, 'XX'),
     'forma desconhecida nao ganha desconto (decisao de 2014)');

caso($casos, 'TST-BR-103-a', 'BR-103 situacao_do_cliente',
     array('maior_atraso_em_dias' => 45), situacao_do_cliente(45));
caso($casos, 'TST-BR-103-b', 'BR-103 situacao_do_cliente',
     array('maior_atraso_em_dias' => 30), situacao_do_cliente(30),
     'exatamente 30 dias ainda fatura, com aviso: o limite e maior que 30');
caso($casos, 'TST-BR-103-c', 'BR-103 situacao_do_cliente',
     array('maior_atraso_em_dias' => 0), situacao_do_cliente(0));

caso($casos, 'TST-BR-104-a', 'BR-104 gera_parcelas',
     array('total' => 100.00, 'n' => 3, 'primeira' => '2026-10-01'), gera_parcelas(100.00, 3, '2026-10-01'),
     'a diferenca de arredondamento vai toda na ultima parcela');
caso($casos, 'TST-BR-104-b', 'BR-104 gera_parcelas',
     array('total' => 3756.30, 'n' => 13, 'primeira' => '2026-10-01'), gera_parcelas(3756.30, 13, '2026-10-01'),
     'acima de 12 parcelas o legado devolve false');

caso($casos, 'TST-BR-105-a', 'BR-105 valida_cnpj',
     array('cnpj' => '11.222.333/0001-81'), valida_cnpj('11.222.333/0001-81'));
caso($casos, 'TST-BR-105-b', 'BR-105 valida_cnpj',
     array('cnpj' => '11.111.111/1111-11'), valida_cnpj('11.111.111/1111-11'));
caso($casos, 'TST-BR-105-c', 'BR-105 valida_cnpj',
     array('cnpj' => '11222333000182'), valida_cnpj('11222333000182'),
     'digito verificador errado de proposito');

echo json_encode(array(
  'descricao' => 'Golden master capturado RODANDO o legado PHP (capturar-golden.php), com os dados de amostra. Nao foi digitado a mao.',
  'capturado_em' => date('Y-m-d'),
  'php' => PHP_VERSION,
  'origem' => 'inputs/legado-php/lib/regras.php',
  'casos' => $casos,
), JSON_PRETTY_PRINT | JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES) . "\n";
