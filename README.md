# IoT_telem_mockup

This was done as a coding exercise for an interview.

The premise was to create a client, simulating something like an IoT device, and a server and develop some of the infrastructure for logging information from the client to the server.

I had a bit of extra fun with simulating devices to start messing with other aspects of the system.

## Approach

Given this is intended to be approximating an IoT system there are a variety of protocols and systems that exist that handle a lot of the nuance for you (ie MQTT or CoAP).
I decided to go with simple HTTP POSTs sending json payloads to try to elevate the decisions/tradeoffs without getting too far in the weeds of a specific protocol/system.

### Message Content

Given there will be many different devices and potentially different firmware versions I wanted to ensure that the messages communicated some identifying fields for both.

The messages types are defined in the `Message` struct in [`device/src/main.rs`](device/src/main.rs).

The device identification could absolutely be handled within the authentication of the device (which I distinctly did not include in this) and the firmware version could be left out if the software update processes was super robust such that there was no way to end up out of sync between the current firmware version loaded on the device and the version the server thinks is loaded on the device.

I envisioned the device's needing to be able to send a variety of message types.
Ideally these would be rigidly defined and specifically selected to optimize the efficiency and value of the logging of information.
To show this I created two main message types `Log` and `SensorData` to distinguish between more application type logging and more sensor telemetry which could be occurring at different rates.

### Simulation

Outside of the original scope I decided to play with autogenerating messages and simulating multiple devices publishing logs.

With this I tried to provide a couple different nobs to turn to change how the devices were behaving.
- `--log-interval`: The interval at which application log messages are generated
- `--sensor-interval`: The interval at which sensor data messages are generated
- `--write-interval`: The interval at which messages are sent from the device (this is a proxy for a slower/intermittent network connection)
- `--buffer-size`: The number of messages that can be held between each `write-interval`
- `-n`/`--number`: The number of devices to simulate simultaneously

With the simulated devices being able to generated different sets of messages I built out a baseline level of filtering/prioritization for the messages so that certain message types would be prioritized over others if the network wasn't able to keep up with the frequency of the messages being generated.
The implementation I ended up with here was a bit of a workaround due to the structure I had hacked together for managing the various message types.
Ideally the prioritization would happen in different structures within the device such that there wouldn't be a risk of overwriting an important message with a less important message, whereas my implementation puts them all in the same buffer and then filters them to set the priority.
With this I was using a larger buffer than `buffer-size` to allow for prioritizing them in one set of filters, truncating to just the `buffer-size`, logging the dropping of messages, and then sending out the remaining messages.

### Things left out

#### Authentication

In no capacity was I doing any authentication that the "device" was actually the device and not some other entity posting to the endpoint.
This is a critical feature but the authentication methodology will be strongly dependent on the ecosystem being used.

#### Encryption

I was using HTTP rather than HTTPS because I didn't want to set up SSL stuff but the messages should absolutely be encrypted.

## Running

### Server

The server was built as a python application solely using the standard libraries to avoid the need to install any dependencies.

The server can be run with `python server/server.py` and will spin up a web server on localhost listening on Port 8080.
The port can be changed by passing in the `--port` argument.

Once running the server will output any valid messages it receives to the console.

### Client

The client was built with Rust and can be built with `cargo build` from within the `device/` directory.

When running the client will default to sending to `localhost:8080` but the port can be changed using the `--port` command line flag.

#### File Based Operation

The client can be run such that it will read in an `ndjson` file and send each line of the file to the server one at a time.
To do this use the `-f/--file` argument to specify the filepath for the client to read.

An example ndjson file with the appropriate message structures is included [`device/example/data.ndjson`](device/example/data.ndjson).

You can run it with cargo from within the `device/` directory with:
```
cargo run -- -f example/data.ndjson
```

An additional argument `-i/--intervsal` can be provided along with the `-f` argument to specify the interval, in seconds, at which the messages will be send out.

```
cargo run -- -f example/data.ndjson -i 3
```

#### Simulated Devices

The client can also simulate the creation of messages and simulate multiple devices all sending messages to the same server.

The options for configuring the simulation are listed above as well as included in the `--help` for the device application.

Some interesting configuration settings:
- `cargo run -- -s --buffer-size 1`
  - This will result in lots of messages not being sent to the server because both types of messages are being generated at 2Hz and the messages are going out at 2Hz but the buffer can only hold 1 message at a time so one of the two messages must be dropped.
- `cargo run -- -s --sensor-interval 200`
  - This will result in some messages not being sent to the server because every other "write" there will be 3 sensor data messages in addition to the 1 log message which will overflow the `buffer-size`.

## Testing (or lack thereof)

Because this was a super vague prompt designed to flush out some of the nuances/trades around the messaging system I decided to punt on testing beyond my own use for the sake of focusing on "features." 
With the message prioritization, I would have liked to add in some tests around that functionality specifically to validate it was working as desired but I realized I should wrap up my work on this.
