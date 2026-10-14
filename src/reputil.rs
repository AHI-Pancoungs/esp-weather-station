pub use core::time::Duration;

pub type Rslt<T = ()> = anyhow::Result<T>;
pub type Err = anyhow::Error;

pub fn esp_init() {
    // It is necessary to call this function once. Otherwise, some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();
}

/// This init function addresses [this](https://github.com/esp-rs/esp-idf-hal/issues/200) ESP-IDF SDK quirk.<br/>
/// Allows console prompts to block and not immediately return nothing.<br/>
/// Without it, all reads on stdio without data already waiting will return nothing instead of blocking normally.<br/>
/// Initializes stdin(on top of VFS) on UART0, recommended to call directly after `reputil::esp_init()`.
pub fn enable_uart_blocking_read() {
    use esp_idf_svc::sys::*;
    unsafe {
        esp!(uart_driver_install(
            0,
            512,
            512,
            10,
            std::ptr::null_mut(),
            0
        ))
        .unwrap();
        esp_vfs_dev_uart_use_driver(0);
    }
}

pub fn delay(dur: core::time::Duration) {
    std::thread::sleep(dur);
}

pub fn simple_err(message: impl Into<String>) -> Err {
    self::Err::msg(message.into())
}

pub fn until_success_or<R, E>(mut f: impl FnMut() -> Result<R, E>, max_retries: u32) -> Option<R>
where
    E: core::fmt::Debug, // the error value has to be printable in debug which almost every sane error return type should be
{
    let mut retries: u32 = 0;
    while max_retries >= retries {
        match f() {
            Ok(r) => return Some(r),
            Err(err) => {
                log::warn!(
                    "{:?}\nAttempt {} error caught, retrying...",
                    err,
                    retries + 1
                );
                retries += 1;
                continue;
            }
        }
    }
    return None;
}

pub fn until_success<R, E>(mut f: impl FnMut() -> Result<R, E>, max_retries: u32) -> Result<R, Err>
where
    E: std::error::Error + Send + Sync + 'static,
{
    let mut retries: u32 = 0;
    while max_retries >= retries {
        match f() {
            Ok(r) => return Ok(r),
            Err(err) => {
                if max_retries >= retries - 1 {
                    log::error!(
                        "{:?}\nAttempt {} error caught, ran out of retries",
                        err,
                        retries + 1
                    );
                    return Err(anyhow::Error::new(err));
                } else {
                    log::warn!(
                        "{:?}\nAttempt {} error caught, retrying...",
                        err,
                        retries + 1
                    );
                }
                retries += 1;
                continue;
            }
        }
    }
    Err(anyhow::Error::msg("Invalid state"))
}

use std::fmt::Debug;
use std::sync::{Arc, Mutex, MutexGuard};

pub struct SharedMut<T> {
    inner: Arc<Mutex<T>>,
}

impl<T> SharedMut<T> {
    pub fn new(obj: T) -> Self {
        SharedMut {
            inner: Arc::new(Mutex::new(obj)),
        }
    }

    fn wrap(obj: Arc<Mutex<T>>) -> Self {
        SharedMut { inner: obj }
    }

    pub fn lock(&self) -> MutexGuard<'_, T> {
        self.inner.lock().unwrap()
    }

    pub fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        let mut guard = self.lock();
        f(&mut *guard)
    }
}

impl<T> Clone for SharedMut<T> {
    fn clone(&self) -> Self {
        Self::wrap(Arc::clone(&self.inner))
    }
}

use esp_idf_svc::sys::esp_log_level_t;
use std::sync::atomic::{AtomicU8, Ordering};

static CURRENT_LOG_LEVEL: AtomicU8 = AtomicU8::new(LogLevel::Info as u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    None,
    Error,
    Warn,
    Info,
    Debug,
    Verbose,
}

impl LogLevel {
    pub fn to_esp(&self) -> esp_log_level_t {
        use esp_idf_svc::sys::*;

        match self {
            Self::None => esp_log_level_t_ESP_LOG_NONE,
            Self::Error => esp_log_level_t_ESP_LOG_ERROR,
            Self::Warn => esp_log_level_t_ESP_LOG_WARN,
            Self::Info => esp_log_level_t_ESP_LOG_INFO,
            Self::Debug => esp_log_level_t_ESP_LOG_DEBUG,
            Self::Verbose => esp_log_level_t_ESP_LOG_VERBOSE,
        }
    }
}

impl From<u8> for LogLevel {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::None,
            1 => Self::Error,
            2 => Self::Warn,
            3 => Self::Info,
            4 => Self::Debug,
            5 => Self::Verbose,
            _ => panic!("Invalid u8 for LogLevel"),
        }
    }
}

pub fn set_log_level(level: LogLevel) {
    CURRENT_LOG_LEVEL.store(level as u8, Ordering::Relaxed);
    unsafe {
        esp_idf_svc::sys::esp_log_level_set(c"*".as_ptr(), level.to_esp());
    }
}

pub fn get_log_level() -> LogLevel {
    CURRENT_LOG_LEVEL.load(Ordering::Relaxed).into()
}

pub struct LogLevelGuard {
    previous: LogLevel,
}

impl LogLevelGuard {
    pub fn new(level: LogLevel) -> Self {
        let previous = get_log_level();
        set_log_level(level);

        LogLevelGuard { previous }
    }

    pub fn silence() -> Self {
        let previous = get_log_level();
        set_log_level(LogLevel::None);

        LogLevelGuard { previous }
    }
}

impl Drop for LogLevelGuard {
    fn drop(&mut self) {
        set_log_level(self.previous);
    }
}

pub fn with_silence<F, T>(f: F) -> T
where
    F: FnOnce() -> T,
{
    let _silencer = LogLevelGuard::silence();
    f()
}

use std::io;
use std::io::Write as IoWrite;

pub fn console_prompt(prompt: &str) -> anyhow::Result<String> {
    let _guard = LogLevelGuard::silence();
    print!("{prompt}: ");
    io::stdout().flush()?;

    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    println!();
    io::stdout().flush()?;

    Ok(line.trim().to_owned())
}

use std::{sync::mpsc, thread};

pub fn prompt_timeout(prompt: &str, timeout: Duration) -> anyhow::Result<Option<String>> {
    let _guard = LogLevelGuard::silence();
    print!("{prompt}: ");
    io::stdout().flush()?;

    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let mut line = String::new();
        let _ = io::stdin().read_line(&mut line);
        let _ = tx.send(line);
    });

    let result = rx.recv_timeout(timeout);

    println!();
    io::stdout().flush()?;

    match result {
        Ok(line) => Ok(Some(line.trim().to_owned())),
        Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

use core::convert::TryInto;
use esp_idf_svc::wifi::{AuthMethod, ClientConfiguration, Configuration};

use esp_idf_svc::nvs::EspNvs;
use esp_idf_svc::wifi::{BlockingWifi, EspWifi};
use esp_idf_svc::{eventloop::EspSystemEventLoop, nvs::EspDefaultNvsPartition};

pub fn provision_wifi(
    nvs: Option<EspDefaultNvsPartition>,
) -> anyhow::Result<(heapless::String<32>, heapless::String<64>)> {
    log::info!("provision_wifi started.");
    if let Some(c) = prompt_timeout("Try to use NVS creds?[Y/n]", Duration::from_secs(10))? {
        match c.trim().to_lowercase().as_str() {
            "n" => {
                let s = until_success_or(
                    || {
                        let s = console_prompt("SSID")?;
                        log::info!("Got SSID: \"{}\"", s);
                        if s.trim() != "" {
                            Ok(s)
                        } else {
                            Err(Err::msg("Invalid SSID"))
                        }
                    },
                    4,
                )
                .ok_or(simple_err("SSID fetch failed too many times."))?;
                let p = console_prompt("Password")?;
                let ssid: heapless::String<32> = s
                    .as_str()
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("SSID too long"))?;
                let pass: heapless::String<64> = p
                    .as_str()
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("Password too long"))?;
                Ok((ssid, pass))
            }
            _ => {
                if let Some(ref part) = nvs {
                    let nvs_storage = EspNvs::new(part.clone(), "wifi_creds", false)?;
                    let mut ssid_buf = [0u8; 32];
                    let mut pass_buf = [0u8; 64];

                    let s = nvs_storage.get_str("ssid", &mut ssid_buf)?;
                    let p = nvs_storage.get_str("password", &mut pass_buf)?;

                    match (s, p) {
                        (Some(s), Some(p)) => {
                            let ssid: heapless::String<32> =
                                s.try_into().map_err(|_| anyhow::anyhow!("SSID too long"))?;
                            let pass: heapless::String<64> = p
                                .try_into()
                                .map_err(|_| anyhow::anyhow!("Password too long"))?;
                            Ok((ssid, pass))
                        }
                        _ => {
                            let s = console_prompt("SSID")?;
                            let p = console_prompt("Password")?;
                            let ssid: heapless::String<32> = s
                                .as_str()
                                .try_into()
                                .map_err(|_| anyhow::anyhow!("SSID too long"))?;
                            let pass: heapless::String<64> = p
                                .as_str()
                                .try_into()
                                .map_err(|_| anyhow::anyhow!("Password too long"))?;
                            Ok((ssid, pass))
                        }
                    }
                } else {
                    let s = console_prompt("SSID")?;
                    let p = console_prompt("Password")?;
                    let ssid: heapless::String<32> = s
                        .as_str()
                        .try_into()
                        .map_err(|_| anyhow::anyhow!("SSID too long"))?;
                    let pass: heapless::String<64> = p
                        .as_str()
                        .try_into()
                        .map_err(|_| anyhow::anyhow!("Password too long"))?;
                    Ok((ssid, pass))
                }
            }
        }
    } else {
        if let Some(ref part) = nvs {
            let nvs_storage = EspNvs::new(part.clone(), "wifi_creds", false)?;
            let mut ssid_buf = [0u8; 32];
            let mut pass_buf = [0u8; 64];

            let s = nvs_storage.get_str("ssid", &mut ssid_buf)?;
            let p = nvs_storage.get_str("password", &mut pass_buf)?;

            match (s, p) {
                (Some(s), Some(p)) => {
                    let ssid: heapless::String<32> =
                        s.try_into().map_err(|_| anyhow::anyhow!("SSID too long"))?;
                    let pass: heapless::String<64> = p
                        .try_into()
                        .map_err(|_| anyhow::anyhow!("Password too long"))?;
                    Ok((ssid, pass))
                }
                _ => {
                    let s = console_prompt("SSID")?;
                    let p = console_prompt("Password")?;
                    let ssid: heapless::String<32> = s
                        .as_str()
                        .try_into()
                        .map_err(|_| anyhow::anyhow!("SSID too long"))?;
                    let pass: heapless::String<64> = p
                        .as_str()
                        .try_into()
                        .map_err(|_| anyhow::anyhow!("Password too long"))?;
                    Ok((ssid, pass))
                }
            }
        } else {
            let s = console_prompt("SSID")?;
            let p = console_prompt("Password")?;
            let ssid: heapless::String<32> = s
                .as_str()
                .try_into()
                .map_err(|_| anyhow::anyhow!("SSID too long"))?;
            let pass: heapless::String<64> = p
                .as_str()
                .try_into()
                .map_err(|_| anyhow::anyhow!("Password too long"))?;
            Ok((ssid, pass))
        }
    }
}

pub fn setup_wifi<'a>(
    modem: esp_idf_svc::hal::modem::Modem<'a>,
    sys_loop: EspSystemEventLoop,
    nvs: Option<EspDefaultNvsPartition>,
    ssid: &str,
    pass: &str,
    max_retries: u32,
) -> Result<BlockingWifi<EspWifi<'a>>, anyhow::Error> {
    let ssid: heapless::String<32> = ssid
        .try_into()
        .map_err(|_| anyhow::anyhow!("SSID too long"))?;
    let pass: heapless::String<64> = pass
        .try_into()
        .map_err(|_| anyhow::anyhow!("Password too long"))?;

    if let Some(ref part) = nvs {
        let nvs_storage = EspNvs::new(part.clone(), "wifi_creds", true)?;
        nvs_storage.set_str("ssid", &ssid)?;
        nvs_storage.set_str("password", &pass)?;
    };

    let mut wifi = BlockingWifi::wrap(EspWifi::new(modem, sys_loop.clone(), nvs)?, sys_loop)?;

    let wifi_config = Configuration::Client(ClientConfiguration {
        ssid: ssid,
        bssid: None,
        auth_method: AuthMethod::WPA2Personal,
        password: pass,
        channel: None,
        ..Default::default()
    });

    let mut attempts: u32 = 1;
    let mut c = || -> anyhow::Result<()> {
        wifi.set_configuration(&wifi_config)?;
        wifi.start()?;
        wifi.connect()?;
        wifi.wait_netif_up()?;
        Ok(())
    };
    loop {
        match c() {
            Ok(_o) => {
                log::info!("setup_wifi successful. Took {} attempts.", attempts);
                return Ok(wifi);
            }
            Err(_e) => {
                if attempts > max_retries {
                    log::info!("setup_wifi failed {} times. Erroring out...", attempts);
                    return Err(anyhow::Error::msg("Setting up WiFi failed too many times."));
                } else {
                    log::info!("setup_wifi attempt {} failed. Retrying...", attempts);
                    attempts += 1;
                    continue;
                }
            }
        }
    }
}

use esp_idf_svc::mqtt::client::{EspMqttClient, EventPayload, MqttClientConfiguration};

pub fn provision_mqtt(
    nvs: Option<EspDefaultNvsPartition>,
) -> anyhow::Result<(
    heapless::String<128>,
    Option<heapless::String<64>>,
    Option<heapless::String<64>>,
)> {
    log::info!("provision_mqtt started.");
    if let Some(c) = prompt_timeout("Try to use NVS creds?[Y/n]", Duration::from_secs(10))? {
        match c.trim().to_lowercase().as_str() {
            "n" => {
                let ur = until_success_or(
                    || {
                        let s = console_prompt("Broker URL")?;
                        log::info!("Got URL: \"{}\"", s);
                        if s.trim() != "" {
                            Ok(s)
                        } else {
                            Err(Err::msg("Invalid URL"))
                        }
                    },
                    4,
                )
                .ok_or(simple_err("Broker URL fetch failed too many times."))?;
                let u = prompt_timeout("Broker Username(if any)", Duration::from_secs(10))?;
                let p = prompt_timeout("Broker Password(if any)", Duration::from_secs(10))?;
                let url: heapless::String<128> = ur
                    .as_str()
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("Username too long"))?;
                if let (Some(u), Some(p)) = (u, p) {
                    let user: heapless::String<64> = u
                        .as_str()
                        .try_into()
                        .map_err(|_| anyhow::anyhow!("Username too long"))?;
                    let pass: heapless::String<64> = p
                        .as_str()
                        .try_into()
                        .map_err(|_| anyhow::anyhow!("Password too long"))?;
                    Ok((url, Some(user), Some(pass)))
                } else {
                    Ok((url, None, None))
                }
            }
            _ => {
                if let Some(ref part) = nvs {
                    let nvs_storage = EspNvs::new(part.clone(), "mqtt_creds", false)?;
                    let mut url_buf = [0u8, 128];
                    let mut user_buf = [0u8; 64];
                    let mut pass_buf = [0u8; 64];

                    let u = nvs_storage.get_str("url", &mut url_buf)?;
                    let s = nvs_storage.get_str("ssid", &mut user_buf)?;
                    let p = nvs_storage.get_str("password", &mut pass_buf)?;

                    match (u, s, p) {
                        (Some(u), Some(s), Some(p)) => {
                            let url: heapless::String<128> =
                                u.try_into().map_err(|_| anyhow::anyhow!("URL too long"))?;
                            let user: heapless::String<64> = s
                                .try_into()
                                .map_err(|_| anyhow::anyhow!("Username too long"))?;
                            let pass: heapless::String<64> = p
                                .try_into()
                                .map_err(|_| anyhow::anyhow!("Password too long"))?;
                            Ok((url, Some(user), Some(pass)))
                        }
                        (Some(u), _, _) => {
                            log::info!(
                                "Username or password not set in NVS, logging in anonymously..."
                            );
                            let url: heapless::String<128> =
                                u.try_into().map_err(|_| anyhow::anyhow!("URL too long"))?;
                            Ok((url, None, None))
                        }
                        _ => {
                            let ur = until_success_or(
                                || {
                                    let s = console_prompt("Broker URL")?;
                                    log::info!("Got URL: \"{}\"", s);
                                    if s.trim() != "" {
                                        Ok(s)
                                    } else {
                                        Err(Err::msg("Invalid URL"))
                                    }
                                },
                                4,
                            )
                            .ok_or(simple_err("Broker URL fetch failed too many times."))?;
                            let u =
                                prompt_timeout("Broker Username(if any)", Duration::from_secs(10))?;
                            let p =
                                prompt_timeout("Broker Password(if any)", Duration::from_secs(10))?;
                            let url: heapless::String<128> = ur
                                .as_str()
                                .try_into()
                                .map_err(|_| anyhow::anyhow!("Username too long"))?;
                            if let (Some(u), Some(p)) = (u, p) {
                                let user: heapless::String<64> = u
                                    .as_str()
                                    .try_into()
                                    .map_err(|_| anyhow::anyhow!("Username too long"))?;
                                let pass: heapless::String<64> = p
                                    .as_str()
                                    .try_into()
                                    .map_err(|_| anyhow::anyhow!("Password too long"))?;
                                Ok((url, Some(user), Some(pass)))
                            } else {
                                Ok((url, None, None))
                            }
                        }
                    }
                } else {
                    let ur = until_success_or(
                        || {
                            let s = console_prompt("Broker URL")?;
                            log::info!("Got URL: \"{}\"", s);
                            if s.trim() != "" {
                                Ok(s)
                            } else {
                                Err(Err::msg("Invalid URL"))
                            }
                        },
                        4,
                    )
                    .ok_or(simple_err("Broker URL fetch failed too many times."))?;
                    let u = prompt_timeout("Broker Username(if any)", Duration::from_secs(10))?;
                    let p = prompt_timeout("Broker Password(if any)", Duration::from_secs(10))?;
                    let url: heapless::String<128> = ur
                        .as_str()
                        .try_into()
                        .map_err(|_| anyhow::anyhow!("Username too long"))?;
                    if let (Some(u), Some(p)) = (u, p) {
                        let user: heapless::String<64> = u
                            .as_str()
                            .try_into()
                            .map_err(|_| anyhow::anyhow!("Username too long"))?;
                        let pass: heapless::String<64> = p
                            .as_str()
                            .try_into()
                            .map_err(|_| anyhow::anyhow!("Password too long"))?;
                        Ok((url, Some(user), Some(pass)))
                    } else {
                        Ok((url, None, None))
                    }
                }
            }
        }
    } else {
        if let Some(ref part) = nvs {
            let nvs_storage = EspNvs::new(part.clone(), "mqtt_creds", false)?;
            let mut url_buf = [0u8, 128];
            let mut user_buf = [0u8; 64];
            let mut pass_buf = [0u8; 64];

            let u = nvs_storage.get_str("url", &mut url_buf)?;
            let s = nvs_storage.get_str("ssid", &mut user_buf)?;
            let p = nvs_storage.get_str("password", &mut pass_buf)?;

            match (u, s, p) {
                (Some(u), Some(s), Some(p)) => {
                    let url: heapless::String<128> =
                        u.try_into().map_err(|_| anyhow::anyhow!("URL too long"))?;
                    let user: heapless::String<64> = s
                        .try_into()
                        .map_err(|_| anyhow::anyhow!("Username too long"))?;
                    let pass: heapless::String<64> = p
                        .try_into()
                        .map_err(|_| anyhow::anyhow!("Password too long"))?;
                    Ok((url, Some(user), Some(pass)))
                }
                (Some(u), _, _) => {
                    log::info!("Username or password not set in NVS, logging in anonymously...");
                    let url: heapless::String<128> =
                        u.try_into().map_err(|_| anyhow::anyhow!("URL too long"))?;
                    Ok((url, None, None))
                }
                _ => {
                    let ur = until_success_or(
                        || {
                            let s = console_prompt("Broker URL")?;
                            log::info!("Got URL: \"{}\"", s);
                            if s.trim() != "" {
                                Ok(s)
                            } else {
                                Err(Err::msg("Invalid URL"))
                            }
                        },
                        4,
                    )
                    .ok_or(simple_err("Broker URL fetch failed too many times."))?;
                    let u = prompt_timeout("Broker Username(if any)", Duration::from_secs(10))?;
                    let p = prompt_timeout("Broker Password(if any)", Duration::from_secs(10))?;
                    let url: heapless::String<128> = ur
                        .as_str()
                        .try_into()
                        .map_err(|_| anyhow::anyhow!("Username too long"))?;
                    if let (Some(u), Some(p)) = (u, p) {
                        let user: heapless::String<64> = u
                            .as_str()
                            .try_into()
                            .map_err(|_| anyhow::anyhow!("Username too long"))?;
                        let pass: heapless::String<64> = p
                            .as_str()
                            .try_into()
                            .map_err(|_| anyhow::anyhow!("Password too long"))?;
                        Ok((url, Some(user), Some(pass)))
                    } else {
                        Ok((url, None, None))
                    }
                }
            }
        } else {
            let ur = until_success_or(
                || {
                    let s = console_prompt("Broker URL")?;
                    log::info!("Got URL: \"{}\"", s);
                    if s.trim() != "" {
                        Ok(s)
                    } else {
                        Err(Err::msg("Invalid URL"))
                    }
                },
                4,
            )
            .ok_or(simple_err("Broker URL fetch failed too many times."))?;
            let u = prompt_timeout("Broker Username(if any)", Duration::from_secs(10))?;
            let p = prompt_timeout("Broker Password(if any)", Duration::from_secs(10))?;
            let url: heapless::String<128> = ur
                .as_str()
                .try_into()
                .map_err(|_| anyhow::anyhow!("Username too long"))?;
            if let (Some(u), Some(p)) = (u, p) {
                let user: heapless::String<64> = u
                    .as_str()
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("Username too long"))?;
                let pass: heapless::String<64> = p
                    .as_str()
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("Password too long"))?;
                Ok((url, Some(user), Some(pass)))
            } else {
                Ok((url, None, None))
            }
        }
    }
}

pub fn setup_mqtt(
    url: &str,
    user: Option<&str>,
    pass: Option<&str>,
    client_id: &str,
) -> Result<EspMqttClient<'static>, Err> {
    let conf = MqttClientConfiguration {
        client_id: Some(client_id),
        username: user,
        password: pass,
        ..Default::default()
    };

    EspMqttClient::new_cb(url, &conf, move |event| match event.payload() {
        EventPayload::Connected(_) => {
            log::info!("MQTT Connected.")
        }
        EventPayload::Disconnected => {
            log::warn!("MQTT Disconnected.")
        }

        EventPayload::Error(err) => {
            log::error!("MQTT error: {:?}", err);
        }

        other => {
            log::debug!("MQTT event: {:?}", other);
        }
    })
    .map_err(|_| Err::msg("Failed initlalizing MQTT"))
}

macro_rules! map_option_heapless_string_as_str {
    ($x:expr) => {
        $x.as_ref().map(|s| s.as_str())
    };
}

pub(crate) use map_option_heapless_string_as_str;
