extends Node3D

var send_identity = true
var tcp_connection = StreamPeerTCP.new()
var drone_a_tscn = preload("res://scenes/drone_a.tscn")
var drone_b_tscn = preload("res://scenes/drone_b.tscn")
var drone_a1 : Node3D
var drone_a2 : Node3D
var drone_b1 : Node3D
var drone_b2 : Node3D
var obstacles_multimesh: MultiMesh
var json = JSON.new()

func _ready():
	var error = tcp_connection.connect_to_host("127.0.0.1", 44556)
	if error == OK:
		print("Connecting to server...")
	else: 
		print("Couldn't connect to server")
	drone_a1 = drone_a_tscn.instantiate()
	add_child(drone_a1)
	drone_a2 = drone_a_tscn.instantiate()
	add_child(drone_a2)
	drone_b1 = drone_b_tscn.instantiate()
	add_child(drone_b1)
	drone_b2 = drone_b_tscn.instantiate()
	add_child(drone_b2)
	obstacles_multimesh = $Obstacles.multimesh

func _process(delta):
	tcp_connection.poll()
	if send_identity:
		tcp_connection.put_data("SPECTATOR\n".to_utf8_buffer())
	if tcp_connection.get_available_bytes() > 0:
		if send_identity:
			send_identity = false
		var received_data = tcp_connection.get_utf8_string(tcp_connection.get_available_bytes())
		update_gamestate(received_data)

func update_gamestate(gamestate: String):
	var error = json.parse(gamestate)
	if error == OK:
		var data_recieved = json.data # returns variant
		var i = 0
		for obstacle in data_recieved["obstacles"].values():
			var pos = obstacle["position"]
			var offset = Vector3(pos[0],pos[1],pos[2])
			var scale = obstacle["radius"]
			obstacles_multimesh.set_instance_transform(i,Transform3D.IDENTITY.translated(offset).scaled(Vector3.ONE * scale))
			i += 1
		drone_a1.position.x =  data_recieved["player_a1"]["position"][0]
		drone_a1.position.y =  data_recieved["player_a1"]["position"][1]
		drone_a1.position.z =  data_recieved["player_a1"]["position"][2]
		drone_a2.position.x =  data_recieved["player_a2"]["position"][0]
		drone_a2.position.y =  data_recieved["player_a2"]["position"][1]
		drone_a2.position.z =  data_recieved["player_a2"]["position"][2]
		drone_b1.position.x =  data_recieved["player_b1"]["position"][0]
		drone_b1.position.y =  data_recieved["player_b1"]["position"][1]
		drone_b1.position.z =  data_recieved["player_b1"]["position"][2]
		drone_b2.position.x =  data_recieved["player_b2"]["position"][0]
		drone_b2.position.y =  data_recieved["player_b2"]["position"][1]
		drone_b2.position.z =  data_recieved["player_b2"]["position"][2]
	else:
		print("Gamestate was not valid JSON: %s" % gamestate)
