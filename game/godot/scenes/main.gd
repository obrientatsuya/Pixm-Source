## main.gd — cena de teste para click-to-move.
##
## Hierarquia esperada:
##   Node2D (root, este script)
##   ├── GameLoopNode
##   └── Hero (Node2D com ColorRect filho)

extends Node2D

@onready var game_loop: GameLoopNode = $GameLoopNode
@onready var hero: Node2D = $Hero

func _ready() -> void:
	game_loop.start_match(42)
	game_loop.spawn_hero(hero.position.x, hero.position.y)
	game_loop.set_hero_node(hero)

func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventMouseButton:
		var mb := event as InputEventMouseButton
		if mb.button_index == MOUSE_BUTTON_RIGHT and mb.pressed:
			var world_pos := get_global_mouse_position()
			game_loop.on_ground_click(world_pos.x, world_pos.y)
