# Bridges

A simple UDP-Serial interface.

## Install :zap:
- :gear: Cargo Install: `cargo install bridges`

## Downloads :package:

- :computer: [Windows](https://github.com/patrickelectric/bridges/releases/latest/download/bridges-x86_64-pc-windows-msvc.exe)
- :apple: [MacOS](https://github.com/patrickelectric/bridges/releases/latest/download/bridges-x86_64-apple-darwin)
- :penguin: [Linux](https://github.com/patrickelectric/bridges/releases/latest/download/bridges-x86_64-unknown-linux-musl)
- :strawberry: [Raspberry](https://github.com/patrickelectric/bridges/latest/releases/continuous/bridges-armv7-unknown-linux-musleabihf)

# Example
## Running as server
Run bridges as server:

`bridges --port /dev/ttyACM0:115200 -u 0.0.0.0:1234`

Run your client:

`netcat -u 127.0.0.1 1234`

## Running as client
Run bridges in client mode:

`bridges --port /dev/ttyACM0:115200 -u 192.168.0.40:1234`
> Note that `192.168.0.40` should be your remote server address

Run your server:
`netcat -lp 1234`
