# gbat

A one-shot macOS CLI that reads the battery status of a Logitech G Pro Wireless 2 over HID++ and prints one line.

## Language

### Devices

**Receiver**:
The LIGHTSPEED USB dongle that relays HID++ traffic to the wireless mouse over its pairing slots.
_Avoid_: Dongle, adapter

**Device index**:
The HID++ address of a device: a receiver pairing slot (1–6), or 0xFF for a mouse connected directly over USB.
_Avoid_: Slot number, device ID

**Offline**:
The state of a mouse that is paired to the receiver but not currently connected to it, because it is switched off or in deep sleep; the receiver answers requests for its device index with an unknown-device error, and only moving the mouse brings it back.
_Avoid_: Unresponsive, disconnected, missing, asleep

**Idle**:
The state of a connected mouse whose radio is in power saving after a short rest; it still answers, but only after waking, so the first read is a cold read. A mouse left idle long enough goes offline.
_Avoid_: Asleep, sleeping

**Cold read**:
A read that has to wake an idle mouse first, and so is slower than a normal read.
_Avoid_: First read, slow read

### Speed and cost

**Read latency**:
Time from invoking gbat to the battery status line on stdout, while the mouse is reachable.
_Avoid_: Reaction speed, response speed, responsiveness

**Failure latency**:
Time from invoking gbat to it giving up with an error, when the mouse is asleep, disconnected, or otherwise unreachable.
_Avoid_: Timeout (that names a mechanism, not the user-visible wait)

**Footprint**:
The resources one invocation costs: binary size, CPU time, and peak memory.
_Avoid_: Performance (too broad; say which of the three terms you mean)
