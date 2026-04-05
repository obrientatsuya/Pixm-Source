## TestArena.gd — arena de teste com bonecos, minions e obstáculo.

extends Node2D

const SCALE        := 8.0
const ZOOM_MIN     := Vector2(0.5,  0.5)
const ZOOM_MAX     := Vector2(2.5,  2.5)
const EDGE_MARGIN  := 60
const SCROLL_SPEED := 500.0

@onready var game : GameLoopNode = $GameLoopNode

var _cam        : Camera2D
var _hero_node  : Node2D
var _cam_locked : bool = true
var _fps_label  : Label
var _stats_label: Label
var _stats_bg   : ColorRect

var _hp_nodes   : Dictionary = {}   # entity_id → Node2D
var _dummy_ids  : Array[int]  = []
var _minion_ids : Array[int]  = []

# ─── Setup ────────────────────────────────────────────────────────────────────

func _ready() -> void:
	game.set_render_scale(SCALE)
	game.start_match(42)

	for cy in range(45, 76):
		game.set_obstacle(63, cy, true)
	_wall(63, 45, 1, 31)

	for wx in range(0, 121, 10):
		game.add_lane_point(0, float(wx), 100.0)

	# Herói
	game.spawn_hero(20.0, 60.0)
	_hero_node         = _cube(Color(0.25, 0.55, 1.00), 18)
	_hero_node.z_index = 100
	add_child(_hero_node)
	game.set_hero_node(_hero_node)
	_add_hp_bar(_hero_node, false)

	# HUD
	var hud := CanvasLayer.new()
	hud.layer = 10
	add_child(hud)
	_build_fps_label(hud)
	_build_stats_panel(hud)

	# Câmera
	_cam          = Camera2D.new()
	_cam.zoom     = Vector2(2.0, 2.0)
	_cam.position = Vector2(20.0 * SCALE, 60.0 * SCALE)
	add_child(_cam)
	_cam.make_current()

	# Bonecos alvo (inimigo)
	for dp in [Vector2(90, 52), Vector2(90, 60), Vector2(90, 68)]:
		var eid  := game.spawn_dummy(dp.x, dp.y)
		var node := _cube(Color(1.00, 0.28, 0.28), 16)
		add_child(node)
		game.link_node(eid, node)
		_add_hp_bar(node, true)
		_hp_nodes[eid] = node
		_dummy_ids.append(eid)

	# Minions (aliado)
	for i in range(3):
		var eid  := game.spawn_minion(float(i * 15), 100.0, 0)
		var node := _cube(Color(0.35, 0.85, 0.35), 13)
		add_child(node)
		game.link_node(eid, node)
		_add_minion_bar(node, false)
		_hp_nodes[eid] = node
		_minion_ids.append(eid)

# ─── Loop ─────────────────────────────────────────────────────────────────────

func _process(delta: float) -> void:
	if _cam == null: return

	# Câmera
	if _cam_locked:
		_cam.global_position = _hero_node.global_position
	else:
		var vp    := get_viewport_rect().size
		var mouse := get_viewport().get_mouse_position()
		var dir   := Vector2.ZERO
		if   mouse.x < EDGE_MARGIN:        dir.x = -1.0
		elif mouse.x > vp.x - EDGE_MARGIN: dir.x =  1.0
		if   mouse.y < EDGE_MARGIN:        dir.y = -1.0
		elif mouse.y > vp.y - EDGE_MARGIN: dir.y =  1.0
		if dir != Vector2.ZERO:
			_cam.position += dir.normalized() * SCROLL_SPEED * delta / _cam.zoom.x

	# FPS
	var fps := Engine.get_frames_per_second()
	var ms  := 1000.0 / float(fps) if fps > 0 else 0.0
	_fps_label.text = "%d fps  |  %.1f ms" % [fps, ms]
	var vps := get_viewport().get_visible_rect().size
	_fps_label.position = Vector2(vps.x - _fps_label.size.x - 8, 6)

	# HP bars
	var hero_hp : Vector2i = game.get_hero_hp()
	_update_bar(_hero_node, hero_hp.x, hero_hp.y)
	for eid: int in _hp_nodes:
		var node := _hp_nodes[eid] as Node2D
		if node: _update_bar(node, game.get_entity_hp(eid).x, game.get_entity_hp(eid).y)

	# Painel de stats
	_refresh_stats()

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
	if _cam_locked: _cam.global_position = _hero_node.global_position

func _zoom(factor: float) -> void:
	_cam.zoom = (_cam.zoom * factor).clamp(ZOOM_MIN, ZOOM_MAX)

# ─── Stats panel ──────────────────────────────────────────────────────────────

func _refresh_stats() -> void:
	var hero_hp : Vector2i = game.get_hero_hp()
	var lines   := PackedStringArray()
	lines.append("── HERÓI ─────────────────")
	lines.append("  HP  %d / %d" % [hero_hp.x, hero_hp.y])

	lines.append("── BONECOS ───────────────")
	for i in range(_dummy_ids.size()):
		var eid : int      = _dummy_ids[i]
		var hp  : Vector2i = game.get_entity_hp(eid)
		var pos : Vector2  = game.get_entity_pos(eid)
		lines.append("  [%d] HP %d/%d  (%d,%d)" % [i+1, hp.x, hp.y, int(pos.x), int(pos.y)])

	lines.append("── MINIONS ───────────────")
	for i in range(_minion_ids.size()):
		var eid : int      = _minion_ids[i]
		var hp  : Vector2i = game.get_entity_hp(eid)
		lines.append("  [%d] HP %d/%d" % [i+1, hp.x, hp.y])

	_stats_label.text = "\n".join(lines)
	# Ajusta tamanho do fundo ao conteúdo
	_stats_bg.size = _stats_label.size + Vector2(16, 12)
	var vph := get_viewport().get_visible_rect().size.y
	_stats_bg.position = Vector2(8, vph - _stats_bg.size.y - 8)
	_stats_label.position = Vector2(8, 6)

# ─── Helpers de construção ────────────────────────────────────────────────────

func _build_fps_label(parent: CanvasLayer) -> void:
	_fps_label = Label.new()
	_fps_label.add_theme_font_size_override("font_size", 14)
	_fps_label.add_theme_color_override("font_color",        Color(1, 1, 1, 0.95))
	_fps_label.add_theme_color_override("font_shadow_color", Color(0, 0, 0, 0.80))
	_fps_label.add_theme_constant_override("shadow_offset_x", 1)
	_fps_label.add_theme_constant_override("shadow_offset_y", 1)
	parent.add_child(_fps_label)

func _build_stats_panel(parent: CanvasLayer) -> void:
	_stats_bg = ColorRect.new()
	_stats_bg.color = Color(0, 0, 0, 0.45)
	parent.add_child(_stats_bg)
	_stats_label = Label.new()
	_stats_label.add_theme_font_size_override("font_size", 13)
	_stats_label.add_theme_color_override("font_color",        Color(0.92, 0.92, 0.92, 1.0))
	_stats_label.add_theme_color_override("font_shadow_color", Color(0,    0,    0,    0.70))
	_stats_label.add_theme_constant_override("shadow_offset_x", 1)
	_stats_label.add_theme_constant_override("shadow_offset_y", 1)
	_stats_bg.add_child(_stats_label)

func _add_hp_bar(node: Node2D, enemy: bool) -> void:
	var script := load("res://scenes/hp_bar.gd")
	if not script: return
	var bar : Control  = script.new()
	bar.name           = "HpBar"
	bar.is_enemy       = enemy
	node.add_child(bar)

func _add_minion_bar(node: Node2D, enemy: bool) -> void:
	var script := load("res://scenes/MinionHPBar.gd")
	if not script: return
	var bar : Control  = script.new()
	bar.name           = "HpBar"
	bar.is_enemy       = enemy
	node.add_child(bar)

func _update_bar(node: Node2D, cur: int, mx: int) -> void:
	var bar := node.get_node_or_null("HpBar") as Control
	if bar and bar.has_method("set_health"): bar.set_health(cur, mx)

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
