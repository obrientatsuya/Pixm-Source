## main.gd — cena de teste para click-to-move + barra de HP.
##
## Hierarquia esperada:
##   Node2D (root, este script)
##   └── GameLoopNode
##       └── Hero (Node2D)
##           ├── Polygon2D  (visual)
##           └── HPBar      (Control com hp_bar.gd)

extends Node2D

@onready var game_loop : GameLoopNode = find_child("GameLoopNode")
@onready var hero      : Node2D       = find_child("Hero")
@onready var hp_bar    : Control      = find_child("HPBar")

func _ready() -> void:
	game_loop.start_match(42)
	game_loop.spawn_hero(hero.position.x, hero.position.y)
	game_loop.set_hero_node(hero)

func _process(_delta: float) -> void:
	if hp_bar == null:
		return
	var hp := game_loop.get_hero_hp()   # Vector2i(current, max)
	hp_bar.set_health(hp.x, hp.y)

func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventMouseButton:
		var mb := event as InputEventMouseButton
		if mb.button_index == MOUSE_BUTTON_RIGHT and mb.pressed:
			var world_pos := get_global_mouse_position()
			game_loop.on_ground_click(world_pos.x, world_pos.y)
