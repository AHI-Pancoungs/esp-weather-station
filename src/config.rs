use crate::reputil;

pub const SENSOR_PERIOD: reputil::Duration = reputil::Duration::from_secs(1);
pub const NETWORK_PERIOD: reputil::Duration = reputil::Duration::from_mins(1);

pub const MQTT_CLIENT_ID: &str = "esp32_01";
pub const MQTT_SENSOR_TOPIC: &str = "weather_station/esp32_01/measurement";
