## main.gd — cena de teste.
## Scroll do mouse: zoom in/out.
## Botão direito: mover herói.

extends Node2D

@onready var game_loop : GameLoopNode = find_child("GameLoopNode")
@onready var hero      : Node2D       = find_child("Hero")
@onready var hp_bar    : Control      = find_child("HPBar")

var _camera : Camera2D

const ZOOM_MIN := Vector2(0.5, 0.5)
const ZOOM_MAX := Vector2(6.0, 6.0)

func _ready() -> void:
	_camera = Camera2D.new()
	_camera.zoom = Vector2(2.5, 2.5)   # começa com zoom in
	add_child(_camera)

	game_loop.start_match(42)
	game_loop.spawn_hero(hero.position.x, hero.position.y)
	game_loop.set_hero_node(hero)

func _process(_delta: float) -> void:
	if hp_bar == null:
		return
	var hp := game_loop.get_hero_hp()
	hp_bar.set_health(hp.x, hp.y)

func _unhandled_input(event: InputEvent) -> void:
	if not event is InputEventMouseButton:
		return
	var mb := event as InputEventMouseButton
	if not mb.pressed:
		return
	match mb.button_index:
		MOUSE_BUTTON_WHEEL_UP:
			_camera.zoom = (_camera.zoom * 1.12).clamp(ZOOM_MIN, ZOOM_MAX)
		MOUSE_BUTTON_WHEEL_DOWN:
			_camera.zoom = (_camera.zoom * 0.89).clamp(ZOOM_MIN, ZOOM_MAX)
		MOUSE_BUTTON_RIGHT:
			var world_pos := get_global_mouse_position()
			game_loop.on_ground_click(world_pos.x, world_pos.y)
