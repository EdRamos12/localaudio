# LocalAudio protocol

Version 1. Original protocol for this project. 48 kHz, stereo, 20 ms Opus frames.

## Discovery

mDNS service type `_localaudio._udp.local`.

- Service port: UDP audio port (`47322` by default).
- TXT `tcp`: TCP control port (`47321` by default).
- TXT `name`: display name.

A player may also connect to a known host and TCP port with no discovery.

## TCP control

UTF-8 lines ending in `\n`. The player connects to the desktop.

Player to desktop:

- `HELLO <name> <token>` — token is `-` when no passphrase is set. The desktop drops the connection with `DENY` when the token does not match a configured passphrase.
- `LISTEN <udpPort>` — send audio to the TCP peer's address on this UDP port.
- `STOP` — stop sending audio to this player. The TCP connection stays open.
- `PING` — keepalive.

Desktop to player:

- `WELCOME <name> 48000 2 opus 20`
- `DENY`
- `PONG`
- `VOLUME <0-100>` — player output gain.

The jitter buffer size is a player setting. It is not negotiated.

## UDP audio

Each datagram is one Opus packet:

| Offset | Type | Field |
| --- | --- | --- |
| 0 | 4 bytes | Magic `LAU1` |
| 4 | u32 LE | Sequence number, starting at 0 |
| 8 | u32 LE | Capture time in milliseconds |
| 12 | u16 LE | Opus payload length |
| 14 | bytes | Opus payload |

Encoder settings: Opus, 48 kHz, stereo, 20 ms, low-delay, 128 kbit/s. The player decodes with libopus and plays 16-bit PCM. The default jitter target is 60 ms.
