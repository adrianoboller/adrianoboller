<?php
// Baixa de titulo com multa e juros calculados na hora (BR-101).
require_once __DIR__ . '/lib/db.php';
require_once __DIR__ . '/lib/regras.php';

$id = isset($_GET['id']) ? (int) $_GET['id'] : 0;
$row = mysqli_fetch_assoc(q("SELECT * FROM titulo WHERE ID = " . $id));
if (!$row) {
    die('Titulo nao encontrado.');
}
$hoje = date('Y-m-d');
$e = calcula_encargos((float) $row['VALOR'], $row['VENCIMENTO'], $hoje);

if ($_SERVER['REQUEST_METHOD'] === 'POST') {
    // Nao ha transacao aqui: o sistema grava a baixa e depois o historico.
    // Se cair entre as duas, o titulo fica baixado sem historico - acontece.
    q("UPDATE titulo SET SITUACAO = 'PAGO', DATA_PAGAMENTO = '" . esc($hoje) . "',
              VALOR_MULTA = " . $e['multa'] . ", VALOR_JUROS = " . $e['juros'] . ",
              VALOR_PAGO = " . $e['total'] . " WHERE ID = " . $id);
    q("INSERT INTO titulo_historico (ID_TITULO, EVENTO, QUANDO)
            VALUES (" . $id . ", 'BAIXA', NOW())");
    header('Location: titulo_listar.php');
    exit;
}
?>
<html><body>
<h1>Baixa do titulo <?php echo $id; ?></h1>
<p>Valor: <?php echo number_format($row['VALOR'], 2, ',', '.'); ?></p>
<p>Multa: <?php echo number_format($e['multa'], 2, ',', '.'); ?></p>
<p>Juros: <?php echo number_format($e['juros'], 2, ',', '.'); ?></p>
<p><b>Total: <?php echo number_format($e['total'], 2, ',', '.'); ?></b></p>
<form method="post"><input type="submit" value="Confirmar baixa"></form>
</body></html>
