extends Node3D

var sent_identity = false

var tcp_connection = StreamPeerTCP.new()

func _ready():
	var error = tcp_connection.connect_to_host("127.0.0.1", 44556)
	if error == OK:
		print("Connecting to server...")
	else: 
		print("Couldn't connect to server")

func _process(delta):
	tcp_connection.poll()
	tcp_connection.put_data("SPECTATOR\n".to_utf8_buffer())
	if tcp_connection.get_available_bytes() > 0:
		print("Got some bytes!")
		var received_data = tcp_connection.get_utf8_string(tcp_connection.get_available_bytes())
		print("Received message: ", received_data)
