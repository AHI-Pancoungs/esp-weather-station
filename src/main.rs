use bh1750::{Resolution, BH1750};
use bme280::i2c::BME280;
use embedded_hal_bus::i2c::RefCellDevice;
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{
        delay::Delay,
        gpio::{InterruptType, PinDriver, Pull},
        i2c::{I2cConfig, I2cDriver},
        peripherals::Peripherals,
        units::KiloHertz,
    },
    http::server::{Configuration as EspHttpServerConfig, EspHttpServer, Method as HttpMethod},
    mqtt::client::QoS,
    nvs::EspDefaultNvsPartition,
};
use heapless::Vec as HVec;
use serde::Serialize;
use std::{
    cell::RefCell,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
    time::Instant,
};

mod config;
mod reputil;

static RAIN_GAUGE_COUNTER: AtomicU32 = AtomicU32::new(0);
static RAIN_INTERRUPT_FIRED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy, Serialize)]
struct LuxMeasurement {
    measurement: f32,
    sample_timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
struct MeasurementBatch {
    temperature_c: f32,
    humidity_percent: f32,
    pressure_hpa: f32,
    rainfall_mm: f32,
    lux_batch: HVec<LuxMeasurement, { config::LUX_BATCH_CAP }>,
    batch_id: u64,
}

fn main() -> reputil::Rslt {
    reputil::esp_init();
    reputil::enable_uart_blocking_read();

    let peripherals = Peripherals::take()?;
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;
    let mut delay = Delay::new_default();
    // Circuit config: SDA at 21, SCL at 22, 100 KHz rate
    let i2c = I2cDriver::new(
        peripherals.i2c0,
        peripherals.pins.gpio21,
        peripherals.pins.gpio22,
        &I2cConfig::default().baudrate(KiloHertz(100).into()),
    )?;

    let i2c_bus = RefCell::new(i2c);

    let mut bme = BME280::new_primary(RefCellDevice::new(&i2c_bus));
    reputil::until_success(
        || {
            bme.init(&mut delay).map_err(|e| {
                reputil::Err::msg(format!("Error in initializing the BME280 sensor: {:?}", e))
            })
        },
        4,
    )?;

    log::info!("BME280 sensor successfully initialized.");

    let mut bh = BH1750::new(RefCellDevice::new(&i2c_bus), delay, false);

    bh.start_continuous_measurement(Resolution::High)
        .map_err(|_| {
            reputil::Err::msg(
                "Error in initializing the BH1750 sensor, please check the connection.",
            )
        })?;

    log::info!("BH1750 sensor successfully Initialized.");

    let (ssid, pass) = reputil::provision_wifi(Some(nvs.clone()))?;
    let wifi = reputil::setup_wifi(
        peripherals.modem,
        sys_loop,
        Some(nvs.clone()),
        &ssid,
        &pass,
        4,
    )?;
    let ip_info = wifi.wifi().sta_netif().get_ip_info()?;

    log::info!("Wi-Fi networking successfully initialized.");

    let mut led_pin = PinDriver::output(peripherals.pins.gpio17)?;
    let led_shared = reputil::SharedMut::new(led_pin);
    let led_mqtt = led_shared.clone();

    let (mqtt_url, mqtt_user, mqtt_pass) = reputil::provision_mqtt(Some(nvs.clone()))?;
    let mut mqtt = reputil::setup_mqtt(
        mqtt_url.as_str(),
        mqtt_user.as_deref(),
        mqtt_pass.as_deref(),
        config::MQTT_CLIENT_ID,
        move |event| match event.payload() {
            reputil::EventPayload::Connected(_) => {
                log::info!("MQTT Connected.");
                let led_shared_cl = led_mqtt.clone();
                std::thread::spawn(move || led_shared_cl.lock().set_high());
            }
            reputil::EventPayload::Disconnected => {
                log::warn!("MQTT Disconnected.")
            }

            reputil::EventPayload::Published(msg_id) => {
                log::info!("MQTT message with ID {} has been published.", msg_id);
                let led_shared_cl = led_mqtt.clone();
                std::thread::spawn(move || -> reputil::Rslt {
                    led_shared_cl.with(|led| -> reputil::Rslt {
                        led.set_low()?;
                        std::thread::sleep(config::LED_BLINK_PERIOD);
                        led.set_high()?;
                        Ok(())
                    })
                });
            }

            reputil::EventPayload::Error(err) => {
                log::error!("MQTT error: {:?}", err);
            }

            other => {
                log::debug!("MQTT event: {:?}", other);
            }
        },
    )?;

    let mut server = EspHttpServer::new(&EspHttpServerConfig::default())?;
    let led_server = led_shared.clone();

    let relay = PinDriver::output(peripherals.pins.gpio16)?;
    let relay_server = reputil::SharedMut::new(relay);

    server.fn_handler("/relay", HttpMethod::Get, move |request| -> reputil::Rslt {
        relay_server.with(|relay| -> reputil::Rslt {
            if relay.is_set_high() {
                relay.set_low()?;
            } else {
                relay.set_high()?;
            };
            // blink led as well
            let led_shared_cl = led_server.clone();
            std::thread::spawn(move || -> reputil::Rslt {
                led_shared_cl.with(|led| -> reputil::Rslt {
                    led.set_low()?;
                    std::thread::sleep(config::LED_BLINK_PERIOD);
                    led.set_high()?;
                    Ok(())
                })
            });
            request.into_response(
                200,
                Some(&format!("Relay is now set to `{}`.", relay.is_set_high())),
                &[("Content-Type", "text/plain")],
            )?;
            Ok(())
        })
    })?;

    log::info!(
        "HTTP Server init complete. \nIP: {}\nSSID: {}\nLAN relay control at GET http://HOSTNAME/relay.",
        ip_info.ip,
        ssid
    );

    let mut rain_gauge_sense = PinDriver::input(peripherals.pins.gpio4, Pull::Floating)?;

    rain_gauge_sense.set_interrupt_type(InterruptType::NegEdge)?;

    unsafe {
        rain_gauge_sense.subscribe(|| {
            // Inside ISR
            RAIN_GAUGE_COUNTER.fetch_add(1, Ordering::Relaxed);
            RAIN_INTERRUPT_FIRED.store(true, Ordering::Relaxed);
        })?;
    }
    rain_gauge_sense.enable_interrupt()?;

    log::info!("Rain Gauge sensor using interrupts successfully initialized.");

    let mut lux_measurement_batch: HVec<LuxMeasurement, { config::LUX_BATCH_CAP }> = HVec::new();
    let mut batch_start = Instant::now();
    let mut batch_id: u64 = 0;

    let mut next_sensor_read = Instant::now();
    let mut next_network_update = Instant::now() + config::NETWORK_PERIOD;

    log::info!("Running main superloop.");

    loop {
        reputil::delay(reputil::Duration::from_millis(1));
        let now = Instant::now();

        if now >= next_sensor_read {
            let sample_timestamp = batch_start.elapsed().as_millis();

            let bh_reading = match bh.get_current_measurement(Resolution::High) {
                Ok(reading) => reading,
                Err(_) => {
                    log::error!("Error in reading the BH1750 sensor, skipping this measurement...");
                    next_sensor_read += config::SENSOR_PERIOD;
                    if now >= next_sensor_read {
                        next_sensor_read = now + config::SENSOR_PERIOD;
                    }
                    continue;
                }
            };

            match lux_measurement_batch.push(LuxMeasurement {
                measurement: bh_reading,
                sample_timestamp_ms: sample_timestamp as u64,
            }) {
                Ok(_) => (),
                Err(_) => log::warn!(
                    "Can't insert current reading into array because it is already full, skipping..."
                ),
            };

            next_sensor_read += config::SENSOR_PERIOD;

            // Deadline resync
            if now >= next_sensor_read {
                next_sensor_read = now + config::SENSOR_PERIOD;
            }
        }

        if now >= next_network_update {
            let bme_reading = match reputil::until_success(
                || {
                    bme.measure(&mut delay)
                        .map_err(|_| reputil::Err::msg("Error in reading the BME280 sensor."))
                },
                4,
            ) {
                Ok(reading) => reading,
                Err(_) => {
                    log::error!("Error in reading the BME280 sensor, skipping this measurement and network update...");
                    next_network_update += config::NETWORK_PERIOD;
                    if now >= next_network_update {
                        next_network_update = now + config::NETWORK_PERIOD;
                    }
                    continue;
                }
            };
            log::info!("Starting network update...");

            let batch = MeasurementBatch {
                temperature_c: bme_reading.temperature,
                humidity_percent: bme_reading.humidity,
                pressure_hpa: bme_reading.pressure / 100f32, // Pa -> hPa scaling factor
                rainfall_mm: (RAIN_GAUGE_COUNTER.swap(0, Ordering::Relaxed) as f32
                    * config::MILIMETERS_PER_TIP),
                lux_batch: lux_measurement_batch.clone(),
                batch_id: batch_id,
            };

            let json = serde_json::to_string(&batch)
                .map_err(|_| reputil::Err::msg("JSON serialization failed."))?;

            mqtt.enqueue(
                config::MQTT_SENSOR_TOPIC,
                QoS::AtLeastOnce,
                false,
                json.as_bytes(),
            )?;

            lux_measurement_batch.clear();
            batch_start = Instant::now();
            batch_id += 1;

            next_network_update += config::NETWORK_PERIOD;
        }

        // ISR cleanup
        if RAIN_INTERRUPT_FIRED.swap(false, Ordering::Relaxed) {
            // Debounce delay
            reputil::delay(config::DEBOUNCE_DURATION);
            // Rearm interrrupt
            rain_gauge_sense.enable_interrupt()?;
            log::info!(
                "Rain gauge ISR has been re-enabled, tip counter is now: {}",
                RAIN_GAUGE_COUNTER.load(Ordering::Relaxed)
            )
        }

        //TODO: add resilient networking cleanup task (maybe?)
    }
}
