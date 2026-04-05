## TestArena.gd — arena de teste com bonecos, minions e obstáculo.
##
## Coordenadas em unidades de simulação (0-128).
## SCALE = 8 → 1 célula = 8 pixels → mapa 128×128 = 1024×1024 px.
##
## Controles:
##   Botão direito     — mover herói
##   Q / W / E / R     — habilidades
##   Scroll            — zoom (limite 1x – 4x)
##   Y                 — travar / soltar câmera no herói
##   (câmera solta)    — mouse nas bordas rola o mapa

extends Node2D

const SCALE        := 8.0
const ZOOM_MIN     := Vector2(0.5,  0.5)
const ZOOM_MAX     := Vector2(4.0,  4.0)
const EDGE_MARGIN  := 60        # px da borda para iniciar scroll
const SCROLL_SPEED := 500.0     # px/s em espaço de mundo (ajusta pelo zoom)

@onready var game : GameLoopNode = $GameLoopNode

var _cam        : Camera2D
var _hero_node  : Node2D
var _cam_locked : bool = true

# ─── Setup ────────────────────────────────────────────────────────────────────

func _ready() -> void:
	game.set_render_scale(SCALE)
	game.start_match(42)

	# Obstáculo: parede vertical em x=63, y=45..75
	for cy in range(45, 76):
		game.set_obstacle(63, cy, true)
	_wall(63, 45, 1, 31)

	# Lane 0: linha reta y=100, x=0→120
	for wx in range(0, 121, 10):
		game.add_lane_point(0, float(wx), 100.0)

	# Herói
	game.spawn_hero(20.0, 60.0)
	_hero_node = _cube(Color(0.25, 0.55, 1.00), 18)
	add_child(_hero_node)
	game.set_hero_node(_hero_node)

	# HP bar no herói
	var hp_script := load("res://scenes/hp_bar.gd")
	if hp_script:
		_hero_node.add_child(hp_script.new())

	# Câmera como filho da cena — posição controlada manualmente
	_cam = Camera2D.new()
	_cam.zoom     = Vector2(2.0, 2.0)
	_cam.position = Vector2(20.0 * SCALE, 60.0 * SCALE)
	add_child(_cam)
	_cam.make_current()

	# Bonecos alvos (vermelho, time 1)
	for dp in [Vector2(90, 52), Vector2(90, 60), Vector2(90, 68)]:
		var eid := game.spawn_dummy(dp.x, dp.y)
		game.link_node(eid, _spawn_linked(Color(1.00, 0.28, 0.28), 16))

	# Minions (verde, time 0, lane 0)
	for i in range(3):
		var eid := game.spawn_minion(float(i * 15), 100.0, 0)
		game.link_node(eid, _spawn_linked(Color(0.35, 0.85, 0.35), 13))

# ─── Loop ─────────────────────────────────────────────────────────────────────

func _process(delta: float) -> void:
	if _cam == null: return

	if _cam_locked:
		# Câmera presa: segue o herói
		_cam.global_position = _hero_node.global_position
	else:
		# Câmera solta: scroll pelas bordas da tela
		var vp   := get_viewport_rect().size
		var mouse := get_viewport().get_mouse_position()
		var dir  := Vector2.ZERO

		if   mouse.x < EDGE_MARGIN:              dir.x = -1.0
		elif mouse.x > vp.x - EDGE_MARGIN:       dir.x =  1.0
		if   mouse.y < EDGE_MARGIN:              dir.y = -1.0
		elif mouse.y > vp.y - EDGE_MARGIN:       dir.y =  1.0

		if dir != Vector2.ZERO:
			_cam.position += dir.normalized() * SCROLL_SPEED * delta / _cam.zoom.x

	# HP bar
	var hp := game.get_hero_hp()
	var bar := _hero_node.get_node_or_null("HPBar") as Control
	if bar and bar.has_method("set_health"):
		bar.set_health(hp.x, hp.y)

# ─── Input ────────────────────────────────────────────────────────────────────

func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventMouseButton:
		var mb := event as InputEventMouseButton
		if not mb.pressed: return
		match mb.button_index:
			MOUSE_BUTTON_RIGHT:
				var wp := get_global_mouse_position()
				game.on_ground_click(wp.x, wp.y)
			MOUSE_BUTTON_WHEEL_UP:   _zoom(1.12)
			MOUSE_BUTTON_WHEEL_DOWN: _zoom(0.89)

	if event is InputEventKey and event.pressed:
		var wp := get_global_mouse_position()
		match event.keycode:
			KEY_Q: game.on_ability(0, wp.x, wp.y)
			KEY_W: game.on_ability(1, wp.x, wp.y)
			KEY_E: game.on_ability(2, wp.x, wp.y)
			KEY_R: game.on_ability(3, wp.x, wp.y)
			KEY_Y: _toggle_camera_lock()

func _toggle_camera_lock() -> void:
	_cam_locked = !_cam_locked
	if _cam_locked:
		_cam.global_position = _hero_node.global_position

func _zoom(factor: float) -> void:
	_cam.zoom = (_cam.zoom * factor).clamp(ZOOM_MIN, ZOOM_MAX)

# ─── Helpers ──────────────────────────────────────────────────────────────────

func _spawn_linked(color: Color, size: int) -> Node2D:
	var node := _cube(color, size)
	add_child(node)
	return node

func _cube(color: Color, size: int) -> Node2D:
	var node := Node2D.new()
	var rect := ColorRect.new()
	rect.color    = color
	rect.size     = Vector2(size, size)
	rect.position = Vector2(-size * 0.5, -size * 0.5)
	node.add_child(rect)
	return node

func _wall(cx: int, cy: int, w: int, h: int) -> void:
	var rect := ColorRect.new()
	rect.color    = Color(0.55, 0.45, 0.30, 0.85)
	rect.position = Vector2(cx * SCALE, cy * SCALE)
	rect.size     = Vector2(w  * SCALE, h  * SCALE)
	add_child(rect)
