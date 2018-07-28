The `absM` protocol is the protocol used to exchange event and information between
the client and server.
It should not depend on the underlying transport protocol, and as such can run
over TCP, UDP, WebSocket or any other transport layer.
Reliability is not guaranteed.

## Endianness

__All__ numbers are encoded in network endianness, also known as big endian.

## Connections

The `absM` protocol defines 'connections', with an opening point and a closing
point in time.
This points might not match between client and server.
When running over TCP, the TCP socket connection is mapped directly to an `absM`
connection.
When running over UDP, the connection opens on the first packet sent/received
and closes at will.

## Packets

`absM` data is sent over discrete packets, each with an associated length and payload.
When running over TCP the payload length is sent first as a 4-byte unsigned integer.
This length does not include the 4-byte length marker.
When running over UDP every packet corresponds to a single datagram.

All packets have a common 4-byte header indicating their type.
Unknown packet types should be ignored.

## Connection handshake

### Handshake-open (`'absM'`)

When a connection is first opened it is in a `disconnected` stage.
In this `disconnected` stage the client sends the server an `'absM'` handshake-open
message to advance the connection to a `connecting` stage.
Note that the `disconnected` stage might be very short, even nonexistent under UDP,
where the handshake-open packet opens the connection.

The `'absM'` handshake-open packet consists of:

```
[packet type (4 bytes representing "absM" in ASCII)]
[client absM major version (2-byte unsigned integer)]
[client absM minor version (2-byte unsigned integer)]
[header fields (0+ bytes)]
```

If the major `absM` version does not match between client and server, the connection
should be aborted as there is no compatibility guarantee.
The `header fields` region consists of any number of repetitions of the pattern:

```
[key string length (4 unsigned bytes)]
[key string raw bytes]
[value string length (4 unsigned bytes)]
[value string raw bytes]
```

A string extending beyond the end of a packet is a malformed `'absM'` packet and
should cause an error.

Different header fields are defined.
Unknown header fields should be ignored as more might be defined in minor versions.
Currently defined header fields:

```
'password' = [raw byte string]
Rudimentary security. Defaults to the empty string ("") if not present.
Since v1.0
```

```
'frame_delay' = [IEEE 754 binary32]
A number indicating the time between two consecutive render frames, in seconds.
Used to modify server FPS.
Since v1.0
```

```
'update_delay' = [IEEE 754 binary32]
A number indicating the time between two consecutive update ticks, in seconds.
Used to modify server event poll frequency.
Since v1.0
```

Upon receival the server should reply with a `'sInf'` server-info message.

### Server-info (`'sInf'`)

The server-info message allows the server to send some preliminary information to
the client.
The `'sInf'` packet format is similar to the `'absM'` packet:

```
[packet type (4 bytes representing the ASCII string "sInf")]
[server absM major version (2 unsigned bytes)]
[server absM minor version (2 unsigned bytes)]
[header fields (0+ bytes)]
```

On `absM` major version mismatch the client should abort the connection.
Unknown fields should be ignored similarly to the `'absM'` message.
Some fields must be present for the packet to be well-formed.
If they are not available the connection should be aborted.
The currently defined fields are:

```
'screen_res' = [IEEE 754 binary32] [IEEE 754 binary32]
Two positive floating-point values representing the total width and height of the source screen.
Extra bytes should be ignored.
REQUIRED ON HANDSHAKE
Since v1.0
```

To this packet the client should reply with a `'setp'` message.

### Setup-info (`'setp'`)

This message allows the client to send some final data before beggining communication.
The packet consists of only header fields:

```
[packet type (4 bytes representing the ASCII string "setp")]
[header fields (0+ bytes)]
```

Header fields are sent in the same format as `'absM'` and `'sInf'` messages.
Currently defined header fields in a `'setp'` message:

```
No 'setp' fields are currently defined.
```

Once the setup-info message is received by the server the connection advances to
the `connected` stage and communication can begin.

## Communication packets

Once the connection has been established any packet type can be sent, even the
initial `'absM'`, `'sInf'` and `'setp'` messages.
These messages no longer have required fields but only override previous headers
if possible.

If any unknown packet is received it should be ignored.
Apart from these other packet types have currently been defined:

```
Ping request 'ping'
Upon receiving a ping a 'repl' packet should be sent containing the same data as
the original ping message.
Since v1.0
```

```
Ping reply 'repl'
Sent upon receiving a 'ping' message.
Contains the same data as the originating 'ping' message.
Since v1.0
```
