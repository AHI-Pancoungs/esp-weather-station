use crate::reputil;

pub const SENSOR_PERIOD: reputil::Duration = reputil::Duration::from_secs(1);
pub const NETWORK_PERIOD: reputil::Duration = reputil::Duration::from_mins(1);
pub const LED_BLINK_PERIOD: reputil::Duration = reputil::Duration::from_millis(200);

pub const MQTT_CLIENT_ID: &str = "esp32_01";
pub const MQTT_SENSOR_TOPIC: &str = "weather_station/esp32_01/measurement";

pub const MILIMETERS_PER_TIP: f32 = 0.30; // Design umber for this sensor
