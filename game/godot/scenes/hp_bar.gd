## hp_bar.gd — barra de HP estilo LoL.
##
## Adiciona como filho de qualquer Node2D (herói, minion, torre).
## Chama set_health(current, max) a cada frame para atualizar.
##
## Características:
##   - Segmentos de 100 HP com divisórias
##   - Marcos em 25/50/75% com linha mais grossa
##   - Ghost bar animada: delay → desliza até o valor atual
##   - Cor da ghost bar: amarelo (dano leve) → laranja → vermelho (dano pesado)
##   - Barra principal muda de cor: verde → amarelo → vermelho por %

extends Control

const SEG_HP       := 100      # cada notch = 100 HP
const BAR_W        := 80.0
const BAR_H        := 8.0
const GHOST_DELAY  := 0.45     # segundos antes da sombra começar a mover
const GHOST_SPEED  := 0.30     # fração da barra por segundo

var max_hp     : int   = 1
var current_hp : int   = 1

var _displayed : float = 1.0   # ghost bar HP value
var _ghost_col : Color = Color(1.0, 0.9, 0.15, 0.85)
var _delay     : float = 0.0
var _ready_set : bool  = false

# ─── API pública ──────────────────────────────────────────────────────────────

func set_health(cur: int, mx: int) -> void:
	mx = max(mx, 1)

	if not _ready_set:
		_displayed = float(cur)
		_ready_set = true
	elif cur < current_hp:
		# Calcula cor da ghost com base no % de dano recebido
		var lost_pct := float(current_hp - cur) / float(mx)
		if lost_pct < 0.10:
			_ghost_col = Color(1.00, 0.90, 0.15, 0.85)  # amarelo
		elif lost_pct < 0.25:
			_ghost_col = Color(1.00, 0.55, 0.00, 0.85)  # laranja
		else:
			_ghost_col = Color(0.90, 0.15, 0.10, 0.85)  # vermelho
		_delay = GHOST_DELAY

	max_hp     = mx
	current_hp = cur
	queue_redraw()

# ─── Loop ─────────────────────────────────────────────────────────────────────

func _ready() -> void:
	custom_minimum_size = Vector2(BAR_W, BAR_H)
	size = Vector2(BAR_W, BAR_H)
	# Centraliza horizontalmente acima do personagem
	position = Vector2(-BAR_W / 2.0, -44.0)

func _process(delta: float) -> void:
	if _displayed <= float(current_hp) + 0.5:
		return
	if _delay > 0.0:
		_delay -= delta
		return
	_displayed = move_toward(_displayed, float(current_hp),
							 float(max_hp) * GHOST_SPEED * delta)
	queue_redraw()

# ─── Desenho ──────────────────────────────────────────────────────────────────

func _draw() -> void:
	var w := BAR_W
	var h := BAR_H

	# Fundo escuro
	draw_rect(Rect2(0, 0, w, h), Color(0.08, 0.08, 0.08, 0.92))

	# Ghost bar (sombra do dano recente)
	if _displayed > float(current_hp) + 0.5:
		var cur_x := clampf(float(current_hp) / float(max_hp), 0.0, 1.0) * w
		var ghost_x := clampf(_displayed / float(max_hp), 0.0, 1.0) * w
		if ghost_x > cur_x:
			draw_rect(Rect2(cur_x, 0, ghost_x - cur_x, h), _ghost_col)

	# Barra principal — verde → amarelo → vermelho
	var pct := clampf(float(current_hp) / float(max_hp), 0.0, 1.0)
	var bar_col: Color
	if pct > 0.50:
		bar_col = Color(0.18, 0.78, 0.25)   # verde
	elif pct > 0.25:
		bar_col = Color(0.92, 0.78, 0.10)   # amarelo
	else:
		bar_col = Color(0.88, 0.18, 0.12)   # vermelho
	draw_rect(Rect2(0, 0, pct * w, h), bar_col)

	# Divisórias de segmento
	var n_segs := max_hp / SEG_HP
	for i in range(1, n_segs + 1):
		var seg_pct := float(i * SEG_HP) / float(max_hp)
		if seg_pct >= 1.0:
			break
		var x := seg_pct * w
		var milestone := (
			abs(seg_pct - 0.25) < 0.005 or
			abs(seg_pct - 0.50) < 0.005 or
			abs(seg_pct - 0.75) < 0.005
		)
		var col   := Color(0, 0, 0, 0.90 if milestone else 0.50)
		var thick := 2.5 if milestone else 1.0
		draw_line(Vector2(x, 0.0), Vector2(x, h), col, thick)

	# Borda
	draw_rect(Rect2(0, 0, w, h), Color(0, 0, 0, 0.75), false, 1.0)
