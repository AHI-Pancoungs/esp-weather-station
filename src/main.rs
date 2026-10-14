use bh1750::{Resolution, BH1750};
use bme280::i2c::BME280;
use embedded_hal_bus::i2c::RefCellDevice;
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{
        delay::Delay,
        i2c::{I2cConfig, I2cDriver},
        peripherals::Peripherals,
        units::KiloHertz,
    },
    mqtt::client::QoS,
    nvs::EspDefaultNvsPartition,
};
use heapless::Vec as HVec;
use serde::Serialize;
use std::{cell::RefCell, time::Instant};

mod config;
mod reputil;

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
    lux_batch: HVec<LuxMeasurement, 60>,
    batch_id: u64,
}

fn main() -> reputil::Rslt {
    reputil::esp_init();
    reputil::enable_uart_blocking_read();

    // TODO: add LED(GPIO 17) turning on when finishing setup and blinking when sending data
    // TODO: add tipping bucket rain sensor(GPIO 18) count tips on every batch minute and send count in batch
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
    reputil::until_success_or(
        || {
            bme.init(&mut delay).map_err(|e| {
                reputil::Err::msg(format!("Error in initializing the BME280 sensor: {:?}", e))
            })
        },
        4,
    );

    log::info!("BME280 sensor successfully Initialized");

    let (ssid, pass) = reputil::provision_wifi(Some(nvs.clone()))?;
    let _wifi = reputil::setup_wifi(
        peripherals.modem,
        sys_loop,
        Some(nvs.clone()),
        &ssid,
        &pass,
        4,
    )?;

    log::info!("Wi-Fi networking successfully Initialized");

    let (mqtt_url, mqtt_user, mqtt_pass) = reputil::provision_mqtt(Some(nvs.clone()))?;
    let mut mqtt = reputil::setup_mqtt(
        mqtt_url.as_str(),
        reputil::map_option_heapless_string_as_str!(mqtt_user),
        reputil::map_option_heapless_string_as_str!(mqtt_pass),
        config::MQTT_CLIENT_ID,
    )?;

    let mut bh = BH1750::new(RefCellDevice::new(&i2c_bus), delay, false);

    bh.start_continuous_measurement(Resolution::High)
        .map_err(|_| {
            reputil::Err::msg(
                "Error in initializing the BH1750 sensor, please check the connection.",
            )
        })?;

    let mut lux_measurement_batch: HVec<LuxMeasurement, 60> = HVec::new();
    let mut batch_start = Instant::now();
    let mut batch_id: u64 = 0;

    let mut next_sensor_read = Instant::now();
    let mut next_network_update = Instant::now();

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
            let bme_reading = match bme.measure(&mut delay) {
                Ok(reading) => reading,
                Err(_) => {
                    log::error!("Error in reading the BME280 sensor, skipping this measurement...");
                    next_sensor_read += config::SENSOR_PERIOD;
                    if now >= next_sensor_read {
                        next_sensor_read = now + config::SENSOR_PERIOD;
                    }
                    continue;
                }
            };
            log::info!("Starting network update...");

            let batch = MeasurementBatch {
                temperature_c: bme_reading.temperature,
                humidity_percent: bme_reading.humidity,
                pressure_hpa: bme_reading.pressure / 100f32, // Pa -> hPa scaling factor
                lux_batch: lux_measurement_batch.clone(),
                batch_id: batch_id,
            };

            let json = serde_json::to_string(&batch)
                .map_err(|_| reputil::Err::msg("JSON serialixation failed"))?;

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

        //TODO: add resilient networking cleanup task if nessaccary
    }
}
