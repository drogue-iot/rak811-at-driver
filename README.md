# rak811-at-driver

[![CI](https://github.com/drogue-iot/rak811-at-driver/actions/workflows/ci.yaml/badge.svg)](https://github.com/drogue-iot/rak811-at-driver/actions/workflows/ci.yaml)
[![crates.io](https://img.shields.io/crates/v/rak811-at-driver.svg)](https://crates.io/crates/rak811-at-driver)
[![docs.rs](https://docs.rs/rak811-at-driver/badge.svg)](https://docs.rs/rak811-at-driver)
[![Matrix](https://img.shields.io/matrix/drogue-iot:matrix.org)](https://matrix.to/#/#drogue-iot:matrix.org)

A network driver for a RAK811 attached via a UART.

Requires the RAK811 to be flashed with a 2.x version of the AT firmware.

The adapter has a serial interface, therefore this driver can be used with any UART driver that implements the `embedded-io` traits.

## Features

* Implements `embedded-nal-async` traits
* Implements `embedded-io` traits
* Full async support, based on `embassy` libraries

## Examples

See [examples/std](examples/std) for an example that works in a STD environment.

