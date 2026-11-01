use crate::reputil;

pub const SENSOR_PERIOD: reputil::Duration = reputil::Duration::from_secs(1);
pub const NETWORK_PERIOD: reputil::Duration = reputil::Duration::from_mins(1);
pub const LUX_BATCH_CAP: usize =
    (NETWORK_PERIOD.as_secs_f32() / SENSOR_PERIOD.as_secs_f32()) as usize; // this operation truncates

pub const LED_BLINK_PERIOD: reputil::Duration = reputil::Duration::from_millis(200);

pub const MQTT_CLIENT_ID: &str = "esp32_01";
pub const MQTT_SENSOR_TOPIC: &str = "weather_station/esp32_01/measurement";

// Numbers for this rain gauge sensor
pub const MILIMETERS_PER_TIP: f32 = 0.30;
pub const DEBOUNCE_DURATION: reputil::Duration = reputil::Duration::from_millis(300);

// use sane ranges for the deployment spot, also consider the sensor's capabiliites
pub const MAX_TEMP_ALLOWED: f32 = 50.0; // C
pub const MIN_TEMP_ALLOWED: f32 = 10.0;
pub const MAX_PRESSURE_ALLOWED: f32 = 1100.0; // hPA, max pressure measurable by the sensor
pub const MIN_PRESSURE_ALLOWED: f32 = 700.0;
pub const MAX_LUX_ALLOWED: f32 = 10000.0; // in a shaded spot, no direct sun

// build checks
macro_rules! const_assert {
    ($($t:tt)*) => {
        const _: () = assert!($($t)*);
    };
}

// (4 (f32) + 8 (u64) + 4 (alignment)) * 200 = 3200 bytes(3.2KB) on the stack
const_assert!(
    LUX_BATCH_CAP <= 200,
    "Calculated LUX_BATCH_CAP exceeds 200."
);
