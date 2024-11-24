extends Camera3D

var rotation_speed: float = 0.1
var distance: float = 40
var angle_around_center : float = 0

# Called every frame. 'delta' is the elapsed time since the previous frame.
func _process(delta):
	angle_around_center += delta * rotation_speed
	position.x = cos(angle_around_center) * distance
	position.z = sin(angle_around_center) * distance
	look_at(Vector3.ZERO, Vector3.UP)
