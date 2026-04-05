## TestArena.gd — arena de teste com bonecos, minions e obstáculo.
##
## Coordenadas em unidades de simulação (0-128).
## SCALE = 8 → 1 célula = 8 pixels → mapa 128×128 = 1024×1024 px.
##
## Controles:
##   Botão direito     — mover herói
##   Q / W / E / R     — habilidades
##   Scroll            — zoom

extends Node2D

const SCALE := 8.0

@onready var game : GameLoopNode = $GameLoopNode

func _ready() -> void:
	game.set_render_scale(SCALE)
	game.start_match(42)

	# ── Obstáculo: parede vertical em x=63, y=45..75 ────────────────────────
	# Bloqueia caminho direto herói → bonecos; força desvio pelo A*
	for cy in range(45, 76):
		game.set_obstacle(63, cy, true)
	_wall(63, 45, 1, 31)

	# ── Lane 0: linha reta y=100, x=0→120 ───────────────────────────────────
	for wx in range(0, 121, 10):
		game.add_lane_point(0, float(wx), 100.0)

	# ── Herói (azul, time 0) ─────────────────────────────────────────────────
	game.spawn_hero(20.0, 60.0)
	var hero := _cube(Color(0.25, 0.55, 1.00), 18)
	add_child(hero)
	game.set_hero_node(hero)

	# Câmera no herói
	var cam := Camera2D.new()
	cam.zoom = Vector2(1.5, 1.5)
	hero.add_child(cam)
	cam.make_current()

	# HP bar no herói
	var hp := load("res://scenes/hp_bar.gd")
	if hp:
		var bar : Control = hp.new()
		hero.add_child(bar)

	# ── Bonecos alvos (vermelho, time 1) ────────────────────────────────────
	var dummy_positions := [
		Vector2(90.0, 52.0),
		Vector2(90.0, 60.0),
		Vector2(90.0, 68.0),
	]
	for dp in dummy_positions:
		var eid := game.spawn_dummy(dp.x, dp.y)
		var node := _cube(Color(1.00, 0.28, 0.28), 16)
		add_child(node)
		game.link_node(eid, node)

	# ── Minions (verde, time 0, lane 0) ─────────────────────────────────────
	for i in range(3):
		var eid := game.spawn_minion(float(i * 15), 100.0, 0)
		var node := _cube(Color(0.35, 0.85, 0.35), 13)
		add_child(node)
		game.link_node(eid, node)

# ─── Input ────────────────────────────────────────────────────────────────────

func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventMouseButton:
		var mb := event as InputEventMouseButton
		if not mb.pressed: return
		match mb.button_index:
			MOUSE_BUTTON_RIGHT:
				var wp := get_global_mouse_position()
				game.on_ground_click(wp.x, wp.y)
			MOUSE_BUTTON_WHEEL_UP:
				_zoom(1.12)
			MOUSE_BUTTON_WHEEL_DOWN:
				_zoom(0.89)

	if event is InputEventKey and event.pressed:
		var wp := get_global_mouse_position()
		match event.keycode:
			KEY_Q: game.on_ability(0, wp.x, wp.y)
			KEY_W: game.on_ability(1, wp.x, wp.y)
			KEY_E: game.on_ability(2, wp.x, wp.y)
			KEY_R: game.on_ability(3, wp.x, wp.y)

func _zoom(factor: float) -> void:
	var cam := get_viewport().get_camera_2d()
	if cam:
		cam.zoom = (cam.zoom * factor).clamp(Vector2(0.3, 0.3), Vector2(8.0, 8.0))

# ─── Helpers visuais ──────────────────────────────────────────────────────────

## Cubo colorido centrado na origem do Node2D.
func _cube(color: Color, size: int) -> Node2D:
	var node := Node2D.new()
	var rect := ColorRect.new()
	rect.color    = color
	rect.size     = Vector2(size, size)
	rect.position = Vector2(-size * 0.5, -size * 0.5)
	node.add_child(rect)
	return node

## Retângulo visual de obstáculo (em pixels, sobre o mapa).
func _wall(cx: int, cy: int, w: int, h: int) -> void:
	var rect := ColorRect.new()
	rect.color    = Color(0.55, 0.45, 0.30, 0.85)
	rect.position = Vector2(cx * SCALE, cy * SCALE)
	rect.size     = Vector2(w  * SCALE, h  * SCALE)
	add_child(rect)
