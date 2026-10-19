use crate::reputil;

pub const SENSOR_PERIOD: reputil::Duration = reputil::Duration::from_secs(1);
pub const NETWORK_PERIOD: reputil::Duration = reputil::Duration::from_mins(1);
pub const LUX_BATCH_CAP: usize =
    (NETWORK_PERIOD.as_secs_f32() / SENSOR_PERIOD.as_secs_f32()) as usize; // this operation truncates

pub const LED_BLINK_PERIOD: reputil::Duration = reputil::Duration::from_millis(200);

pub const MQTT_CLIENT_ID: &str = "esp32_01";
pub const MQTT_SENSOR_TOPIC: &str = "weather_station/esp32_01/measurement";

pub const MILIMETERS_PER_TIP: f32 = 0.30; // Design number for this sensor

// build checks
macro_rules! const_assert {
    ($($t:tt)*) => {
        const _: () = assert!($($t)*);
    };
}

// We have a total of 16KB of stack in main task, 12KB wasn't enough for the normal application to run

// (4 (f32) + 8 (u64) + 4 (alignment)) * 200 = 3200 bytes(3.2KB) on the stack
const_assert!(
    LUX_BATCH_CAP <= 200,
    "Calculated LUX_BATCH_CAP exceeds 200."
);
