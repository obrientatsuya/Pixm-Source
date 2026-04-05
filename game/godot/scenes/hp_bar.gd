## hp_bar.gd — barra de HP estilo LoL.

extends Control

const SEG_HP      := 50
const BAR_W       := 80.0
const BAR_H       := 8.0
const MANA_H      := 3.0
const GAP         := 2.0
const GHOST_DELAY := 0.45
const GHOST_SPEED := 0.30

var max_hp     : int   = 1
var current_hp : int   = 1
var _displayed : float = 1.0
var _ghost_col : Color = Color(1.0, 0.9, 0.15, 0.85)
var _delay     : float = 0.0
var _ready_set : bool  = false

# ─── API ──────────────────────────────────────────────────────────────────────

func set_health(cur: int, mx: int) -> void:
	mx = max(mx, 1)
	if not _ready_set:
		_displayed = float(cur)
		_ready_set = true
	elif cur < current_hp:
		var lost_pct := float(current_hp - cur) / float(mx)
		if lost_pct < 0.10:
			_ghost_col = Color(1.00, 0.90, 0.15, 0.85)
		elif lost_pct < 0.25:
			_ghost_col = Color(1.00, 0.55, 0.00, 0.85)
		else:
			_ghost_col = Color(0.90, 0.15, 0.10, 0.85)
		_delay = GHOST_DELAY
	max_hp     = mx
	current_hp = cur
	queue_redraw()

# ─── Loop ─────────────────────────────────────────────────────────────────────

func _ready() -> void:
	var total_h := BAR_H + GAP + MANA_H
	custom_minimum_size = Vector2(BAR_W, total_h)
	size                = Vector2(BAR_W, total_h)
	position            = Vector2(-BAR_W / 2.0, -62.0)

func _process(delta: float) -> void:
	if _displayed <= float(current_hp) + 0.5:
		return
	if _delay > 0.0:
		_delay -= delta
		return
	_displayed = move_toward(_displayed, float(current_hp),
							 float(max_hp) * GHOST_SPEED * delta)
	queue_redraw()

# ─── Helpers de desenho ───────────────────────────────────────────────────────

func _sbox(color: Color) -> StyleBoxFlat:
	var s := StyleBoxFlat.new()
	s.bg_color = color
	return s

func _sbox_border(color: Color, width: float) -> StyleBoxFlat:
	var s := StyleBoxFlat.new()
	s.bg_color = Color(0, 0, 0, 0)
	s.set_border_width_all(int(width))
	s.border_color = color
	return s

# ─── Desenho ──────────────────────────────────────────────────────────────────

func _draw() -> void:
	var w := BAR_W
	var h := BAR_H

	# Fundo
	draw_style_box(_sbox(Color(0.08, 0.08, 0.08, 0.92)), Rect2(0, 0, w, h))

	# Ghost bar
	if _displayed > float(current_hp) + 0.5:
		var cur_x   := clampf(float(current_hp) / float(max_hp), 0.0, 1.0) * w
		var ghost_x := clampf(_displayed        / float(max_hp), 0.0, 1.0) * w
		if ghost_x > cur_x:
			draw_rect(Rect2(cur_x, 1, ghost_x - cur_x, h - 2), _ghost_col)

	# Preenchimento HP
	var pct := clampf(float(current_hp) / float(max_hp), 0.0, 1.0)
	if pct > 0.0:
		var bar_col: Color
		if pct > 0.50:
			bar_col = Color(0.18, 0.78, 0.25)
		elif pct > 0.25:
			bar_col = Color(0.92, 0.78, 0.10)
		else:
			bar_col = Color(0.88, 0.18, 0.12)
		draw_style_box(_sbox(bar_col), Rect2(0, 0, pct * w, h))

	# Divisórias finas (dentro do contorno)
	var n_segs := max_hp / SEG_HP
	for i in range(1, n_segs + 1):
		var seg_pct := float(i * SEG_HP) / float(max_hp)
		if seg_pct >= 1.0:
			break
		var x := seg_pct * w
		draw_line(Vector2(x, 1.5), Vector2(x, h - 1.5), Color(0, 0, 0, 0.38), 1.0)

	# Marcos 25 / 50 / 75% — pretos, grossos
	for mpct: float in [0.25, 0.50, 0.75]:
		var x: float = mpct * w
		draw_line(Vector2(x, 1.0), Vector2(x, h - 1.0), Color(0, 0, 0, 0.92), 3.0)

	# Contorno
	draw_style_box(_sbox_border(Color(0, 0, 0, 0.90), 2.0), Rect2(0, 0, w, h))

	# ── Barra de mana ──────────────────────────────────────────────────────────
	var my := h + GAP
	draw_style_box(_sbox(Color(0.08, 0.08, 0.08, 0.88)), Rect2(0, my, w, MANA_H))
	draw_style_box(_sbox(Color(0.95, 0.80, 0.10, 0.90)), Rect2(0, my, w, MANA_H))

	var mn_segs := max_hp / SEG_HP
	for i in range(1, mn_segs + 1):
		var seg_pct := float(i * SEG_HP) / float(max_hp)
		if seg_pct >= 1.0:
			break
		var x := seg_pct * w
		draw_line(Vector2(x, my + 0.5), Vector2(x, my + MANA_H - 0.5),
				  Color(0, 0, 0, 0.45), 1.0)

	draw_style_box(_sbox_border(Color(0, 0, 0, 0.80), 1.0), Rect2(0, my, w, MANA_H))
