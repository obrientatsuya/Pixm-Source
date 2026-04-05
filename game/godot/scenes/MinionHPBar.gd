## MinionHPBar.gd — barra de HP simples para minions.
## Sem segmentos, sem mana, sem ghost.
## is_enemy = false → azul | is_enemy = true → vermelho

extends Control

const BAR_W := 28.0
const BAR_H := 3.0

var max_hp     : int  = 1
var current_hp : int  = 1
var is_enemy   : bool = false

# ─── API ──────────────────────────────────────────────────────────────────────

func set_health(cur: int, mx: int) -> void:
	max_hp     = max(mx, 1)
	current_hp = cur
	queue_redraw()

# ─── Setup ────────────────────────────────────────────────────────────────────

func _ready() -> void:
	custom_minimum_size = Vector2(BAR_W, BAR_H)
	size                = Vector2(BAR_W, BAR_H)
	position            = Vector2(-BAR_W / 2.0, -18.0)

# ─── Desenho ──────────────────────────────────────────────────────────────────

func _draw() -> void:
	var w   := BAR_W
	var h   := BAR_H
	var pct := clampf(float(current_hp) / float(max_hp), 0.0, 1.0)

	# Fundo
	draw_rect(Rect2(0, 0, w, h), Color(0.08, 0.08, 0.08, 0.85))

	# Preenchimento
	if pct > 0.0:
		var col: Color
		if is_enemy:
			col = Color(0.88, 0.20, 0.14) if pct > 0.25 else Color(0.60, 0.07, 0.05)
		else:
			col = Color(0.18, 0.52, 0.92) if pct > 0.50 else \
				  Color(0.14, 0.42, 0.80) if pct > 0.25 else \
				  Color(0.80, 0.15, 0.10)
		draw_rect(Rect2(0, 0, pct * w, h), col)

	# Contorno fino
	draw_rect(Rect2(0, 0, w, h), Color(0, 0, 0, 0.75), false)
