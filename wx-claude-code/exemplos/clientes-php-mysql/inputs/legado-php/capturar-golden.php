<?php
// Golden master no formato do golden.py: um caso por REGRA, cada um com a
// propria sequencia, rodada do zero (TRUNCATE) contra o legado no MySQL.
// O Rust recebe {id, regra, entrada} pela entrada padrao e tem de devolver
// a MESMA lista de respostas.
require_once __DIR__ . '/clientes.php';

function rodar($passos) {
  mysqli_query(conectar(), 'TRUNCATE TABLE clientes');
  $saida = array();
  foreach ($passos as $p) {
    switch ($p[0]) {
      case 'incluir': $r = incluir($p[1], $p[2], $p[3]); break;
      case 'alterar': $r = alterar($p[1], $p[2], $p[3], $p[4]); break;
      case 'excluir': $r = excluir($p[1]); break;
      case 'listar':  $r = listar(true); break;
      case 'listar_todos': $r = listar(false); break;
    }
    $saida[] = $r;
  }
  return $saida;
}

$casos = array(
  array('id' => 'BR-001', 'regra' => 'e-mail unico, minusculo, sem espacos nas pontas', 'passos' => array(
    array('incluir', 'Maria da Silva', 'MARIA@Loja.com ', '100'),
    array('incluir', 'Joao Souza', 'maria@loja.com', '200'),
    array('listar'))),
  array('id' => 'BR-002', 'regra' => 'limite de credito nao negativo, 2 casas, meio para cima', 'passos' => array(
    array('incluir', 'Joao Souza', 'joao@loja.com', '-5'),
    array('incluir', 'Maria da Silva', 'maria@loja.com', '1500.005'),
    array('incluir', 'Joao Souza', 'joao@loja.com', '999.994'),
    array('listar'))),
  array('id' => 'BR-003', 'regra' => 'nome entre 3 e 80, sem espaco duplo', 'passos' => array(
    array('incluir', 'Jo', 'jo@x.com', '10'),
    array('incluir', '  Maria   da  Silva ', 'maria@loja.com', '10'),
    array('listar'))),
  array('id' => 'BR-004', 'regra' => 'excluir desativa; nao apaga; segunda vez nao faz nada', 'passos' => array(
    array('incluir', 'Maria da Silva', 'maria@loja.com', '10'),
    array('incluir', 'Joao Souza', 'joao@loja.com', '20'),
    array('excluir', 1),
    array('excluir', 1),
    array('alterar', 1, 'Maria Outra', 'maria@loja.com', '99'),
    array('listar'),
    array('listar_todos'))),
  array('id' => 'QRY-001', 'regra' => 'ordem por nome e id; auto-increment do MySQL pula o id do INSERT recusado', 'passos' => array(
    array('incluir', 'Zeca Pagodinho', 'zeca@loja.com', '1'),
    array('incluir', 'Outro', 'zeca@loja.com', '1'),
    array('incluir', 'Ana Lima', 'ana@loja.com', '2'),
    array('incluir', 'Ana Lima', 'ana2@loja.com', '3'),
    array('listar'))),
);
$saida = array();
foreach ($casos as $c) {
  $saida[] = array('id' => $c['id'], 'regra' => $c['regra'],
                   'entrada' => array('passos' => $c['passos']),
                   'esperado' => rodar($c['passos']));
}
echo json_encode($saida, JSON_PRETTY_PRINT | JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES), "\n";
