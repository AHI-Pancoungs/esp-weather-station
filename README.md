# ESP32 Weather Station Firmware

This repo contains the source code for an ESP32-based weather station with multiple types of sensors:
- BME280: [further reading](https://randomnerdtutorials.com/esp32-bme280-arduino-ide-pressure-temperature-humidity/), [crate used](https://crates.io/crates/bme280).
- BH1750: [further reading](https://randomnerdtutorials.com/arduino-bh1750-ambient-light-sensor/), [crate used](https://crates.io/crates/bh1750).
- Tipping Bucket Rain Gauge Sensor([sensor used](https://evoteknologi.com/product-detail/?id=112) is calibrated at 0.30 mm/tip)

# General Architecture
The station is built with the [ESP32 DEVKIT V1](https://www.espboards.dev/esp32/esp32doit-devkit-v1/) development board at its core, a wireless-capabable, powerful, and cheap IoT development board carrying the ESP32 chip made by Espressif.

The firmware is written using [embedded Rust](https://github.com/rust-embedded/awesome-embedded-rust) with a `std` enviroment provided by FreeRTOS and [`esp-idf-hal`](https://esp-rs.github.io/std-training/).

# Repo Licensing

This repo and all of the code within are used on the [Eureka ITB Science Project Competition](https://www.eurekaitb.com/).

Copyright © 2026 AHI Pancoungs. All rights reserved. No permission is granted to use, modify, or redistribute this software except as required or permitted by applicable law or competition rules.

This software's direct dependencies may use different licenses, you can check each one here:
- [crate bme280](https://crates.io/crates/bme280/0.5.1/code/LICENSE-MIT) (MIT License) - Copyright 2018 Sean Bruton <sean@uberfoo.net>
- [crate bh1750](https://crates.io/crates/bh1750/0.1.0/code/LICENSE-MIT) (MIT License) - Copyright to the author(s)
- [crates esp-idf-sys, esp-idf-hal, esp-idf-svc](https://github.com/esp-rs/esp-idf/blob/master/LICENSE-MIT) (MIT License) - Copyright 2019-2020 Contributors to xtensa-lx6-rt
- [crate log](https://crates.io/crates/log/0.4.34/code/LICENSE-MIT) (MIT License) - Copyright (c) 2014 The Rust Project Developers
- [crate embassy-time](https://github.com/embassy-rs/embassy/blob/main/LICENSE-MIT) (MIT License) - Copyright (c) Embassy project contributors
- [crate anyhow](https://github.com/dtolnay/anyhow/blob/HEAD/LICENSE-MIT) (MIT License) - Copyright to the author(s)
- [crate heapless](https://github.com/rust-embedded/heapless/blob/HEAD/LICENSE-MIT) (MIT License) - Copyright (c) 2017 Jorge Aparicio
- [crate serde_json](https://github.com/serde-rs/json/blob/HEAD/LICENSE-MIT) (MIT License) - Copyright to the author(s)
- [crate serde](https://crates.io/crates/serde/1.0.229/code/LICENSE-MIT) (MIT License) - Copyright to the author(s)
- [crate embedded-hal-bus](https://github.com/rust-embedded/embedded-hal/blob/HEAD/embedded-hal-bus/LICENSE-MIT) (MIT License) - Copyright (c) 2021-2026 The Rust embedded HAL team and contributors

