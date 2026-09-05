<?php
// Tela de emissao de fatura. HTML e regra misturados, como todo o sistema.
require_once __DIR__ . '/lib/db.php';
require_once __DIR__ . '/lib/regras.php';

$id_cliente = isset($_POST['id_cliente']) ? (int) $_POST['id_cliente'] : 0;
$forma      = isset($_POST['forma']) ? esc($_POST['forma']) : 'PZ';
$parcelas   = isset($_POST['parcelas']) ? (int) $_POST['parcelas'] : 1;
$erro = '';
$fatura = null;

if ($_SERVER['REQUEST_METHOD'] === 'POST' && $id_cliente > 0) {
    $situacao = situacao_do_cliente(maior_atraso_do_cliente($id_cliente));
    if ($situacao === 'BLOQUEADO') {
        // BR-103: nao fatura e nao grava nada.
        $erro = 'Cliente bloqueado por inadimplencia acima de 30 dias.';
    } else {
        $r = q("SELECT SUM(QUANTIDADE * VALOR_UNITARIO) AS subtotal
                  FROM pedido_item i JOIN pedido p ON p.ID = i.ID_PEDIDO
                 WHERE p.ID_CLIENTE = " . $id_cliente . " AND p.SITUACAO = 'A_FATURAR'");
        $row = mysqli_fetch_assoc($r);
        $subtotal = (float) $row['subtotal'];
        $desconto = desconto_por_forma($subtotal, $forma);
        $total = round($subtotal - $desconto, 2);
        $lista = gera_parcelas($total, $parcelas, date('Y-m-d', strtotime('+30 days')));
        if ($lista === false) {
            $erro = 'Numero de parcelas invalido (1 a 12).';
        } else {
            $fatura = array('subtotal' => $subtotal, 'desconto' => $desconto,
                            'total' => $total, 'parcelas' => $lista, 'aviso' => $situacao === 'AVISO');
        }
    }
}
?>
<html><head><title>Emissao de fatura</title>
<link rel="stylesheet" href="estilo.css"></head><body>
<h1>Emissao de fatura</h1>
<?php if ($erro) { echo '<p class="erro">' . htmlspecialchars($erro) . '</p>'; } ?>
<?php if ($fatura && $fatura['aviso']) { echo '<p class="aviso">Cliente com titulo vencido. Fature com atencao.</p>'; } ?>
<form method="post">
  Cliente: <input name="id_cliente" value="<?php echo $id_cliente; ?>">
  Forma:
  <select name="forma">
    <option value="AV">A vista</option>
    <option value="PZ">A prazo</option>
    <option value="BO">Boleto</option>
  </select>
  Parcelas: <input name="parcelas" size="2" value="<?php echo $parcelas; ?>">
  <input type="submit" value="Gerar">
</form>
<?php if ($fatura) { ?>
<table border="1">
  <tr><td>Subtotal</td><td><?php echo number_format($fatura['subtotal'], 2, ',', '.'); ?></td></tr>
  <tr><td>Desconto</td><td><?php echo number_format($fatura['desconto'], 2, ',', '.'); ?></td></tr>
  <tr><td><b>Total</b></td><td><b><?php echo number_format($fatura['total'], 2, ',', '.'); ?></b></td></tr>
</table>
<h2>Parcelas</h2>
<table border="1">
<?php foreach ($fatura['parcelas'] as $p) { ?>
  <tr><td><?php echo $p['numero']; ?></td>
      <td><?php echo date('d/m/Y', strtotime($p['vencimento'])); ?></td>
      <td><?php echo number_format($p['valor'], 2, ',', '.'); ?></td></tr>
<?php } ?>
</table>
<?php } ?>
</body></html>
