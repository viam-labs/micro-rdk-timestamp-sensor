use micro_rdk::common::config::ConfigType;
use micro_rdk::common::generic::DoCommand;
use micro_rdk::common::registry::{ComponentRegistry, Dependency, RegistryError};
use micro_rdk::common::status::{Status, StatusError};
use micro_rdk::google::protobuf::value::Kind;
use micro_rdk::google::protobuf::{ListValue, Value};
use micro_rdk::DoCommand;
use std::cell::RefCell;
use std::collections::HashMap;
use std::net::TcpStream;
use std::sync::{Arc, Mutex};

use micro_rdk::common::sensor::{
    GenericReadingsResult, Readings, Sensor, SensorError, SensorResult, SensorT, SensorType,
};

struct TimestampSensor;

#[derive(DoCommand)]
struct RandoSensors {
    inner: u64,
}

impl RandoSensors {
    pub fn from_config(_: ConfigType, _: Vec<Dependency>) -> Result<SensorType, SensorError> {
        Ok(Arc::new(Mutex::new(Self { inner: 0 })))
    }
}

#[derive(DoCommand)]
struct FatMessages {
    buf_len: i32,
}

impl FatMessages {
    pub fn from_config(cfg: ConfigType, _: Vec<Dependency>) -> Result<SensorType, SensorError> {
        let len = cfg.get_attribute::<i32>("len").unwrap_or(10);

        Ok(Arc::new(Mutex::new(Self { buf_len: len })))
    }
}

impl Sensor for FatMessages {}
impl Readings for FatMessages {
    fn get_generic_readings(
        &mut self,
    ) -> Result<micro_rdk::common::sensor::GenericReadingsResult, SensorError> {
        use base64::{engine::general_purpose::STANDARD, Engine as _};

        let len: usize = self.buf_len.try_into().unwrap();
        #[cfg(target_os = "espidf")]
        {
            use micro_rdk::esp32::esp_idf_svc::sys::{esp_fill_random, esp_timer_get_time};
            let mut buf = vec![0_u8; len];
            unsafe { esp_fill_random(buf.as_mut_ptr() as *mut _, len.try_into().unwrap()) };
            let enc = STANDARD.encode(buf);
            let res = GenericReadingsResult::from([(
                "blob".to_owned(),
                Value {
                    kind: Some(Kind::StringValue(enc)),
                },
            )]);
            Ok(res)
        }
        #[cfg(not(target_os = "espidf"))]
        {
            let mut buf = Vec::with_capacity(len);

            for _ in 0..buf.capacity() {
                buf.push(rand::random());
            }

            let enc = STANDARD.encode(buf);
            let res = GenericReadingsResult::from([(
                "blob".to_owned(),
                Value {
                    kind: Some(Kind::StringValue(enc)),
                },
            )]);
            Ok(res)
        }
    }
}

impl Sensor for RandoSensors {}
impl Readings for RandoSensors {
    fn get_generic_readings(
        &mut self,
    ) -> Result<micro_rdk::common::sensor::GenericReadingsResult, SensorError> {
        self.inner = self.inner + 1;
        #[cfg(target_os = "espidf")]
        let time: i64 = unsafe { micro_rdk::esp32::esp_idf_svc::sys::esp_timer_get_time() };
        #[cfg(not(target_os = "espidf"))]
        let time: u128 = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let time = (time as f64) * 1e-6; // in seconds
        let sin1 = 100.0 * f64::sin(std::f64::consts::PI * 2.0 * time * 0.000277777);
        let sin2 = 100.0 * f64::sin(std::f64::consts::PI * 2.0 * time * 0.016666666666667);

        let res = GenericReadingsResult::from([
            (
                "sin1".to_owned(),
                Value {
                    kind: Some(Kind::NumberValue(sin1)),
                },
            ),
            (
                "sin2".to_owned(),
                Value {
                    kind: Some(Kind::NumberValue(sin2)),
                },
            ),
            (
                "inner".to_owned(),
                Value {
                    kind: Some(Kind::NumberValue(self.inner as f64)),
                },
            ),
        ]);
        Ok(res)
    }
}
#[derive(DoCommand)]
struct InternetSensor {
    inet: u32,
    online: bool,
}
impl InternetSensor {
    pub fn from_config(_: ConfigType, _: Vec<Dependency>) -> Result<SensorType, SensorError> {
        Ok(Arc::new(Mutex::new(InternetSensor {
            inet: 0,
            online: false,
        })))
    }
}

impl DoCommand for TimestampSensor {
    fn do_command(
        &mut self,
        command_struct: Option<micro_rdk::google::protobuf::Struct>,
    ) -> Result<Option<micro_rdk::google::protobuf::Struct>, micro_rdk::common::generic::GenericError>
    {
        if let Some(cmd) = command_struct {
            log::info!("cmd : {:?}", cmd);
            if let Some(_) = cmd.fields.get("panic") {
                panic!("bye");
            }
        }
        Ok(None)
    }
}

impl Sensor for InternetSensor {}
impl Readings for InternetSensor {
    fn get_generic_readings(
        &mut self,
    ) -> Result<micro_rdk::common::sensor::GenericReadingsResult, SensorError> {
        let access = TcpStream::connect("8.8.8.8:53");
        if let Err(e) = access {
            log::error!("inet sens failed with {:?}", e);
            self.inet = self.inet + 1;
            self.online = false;
        } else {
            self.inet = 0;
            self.online = true;
        }
        let res = GenericReadingsResult::from([
            (
                "online".to_owned(),
                Value {
                    kind: Some(Kind::BoolValue(self.online)),
                },
            ),
            (
                "inet".to_owned(),
                Value {
                    kind: Some(Kind::NumberValue(self.inet as f64)),
                },
            ),
        ]);
        Ok(res)
    }
}

pub fn register_models(registry: &mut ComponentRegistry) -> Result<(), RegistryError> {
    registry.register_sensor("esp32-data", &TimestampSensor::from_config)?;
    registry.register_sensor("esp32-internet", &InternetSensor::from_config)?;
    registry.register_sensor("esp32-fat", &FatMessages::from_config)?;
    registry.register_sensor("esp32-blobber", &RandoSensors::from_config)
}

impl TimestampSensor {
    pub fn from_config(_: ConfigType, _: Vec<Dependency>) -> Result<SensorType, SensorError> {
        Ok(Arc::new(Mutex::new(Self)))
    }
}

impl Sensor for TimestampSensor {}

impl Readings for TimestampSensor {
    fn get_generic_readings(
        &mut self,
    ) -> Result<micro_rdk::common::sensor::GenericReadingsResult, SensorError> {
        #[cfg(target_os = "espidf")]
        {
            use micro_rdk::esp32::esp_idf_svc::sys::{
                esp_timer_get_time, heap_caps_get_free_size, uxTaskGetStackHighWaterMark,
                MALLOC_CAP_8BIT, MALLOC_CAP_INTERNAL, MALLOC_CAP_SPIRAM,
            };
            let reading: i64 = unsafe { esp_timer_get_time() };
            let free_internal =
                unsafe { heap_caps_get_free_size(MALLOC_CAP_INTERNAL | MALLOC_CAP_8BIT) };
            let free_spiram =
                unsafe { heap_caps_get_free_size(MALLOC_CAP_SPIRAM | MALLOC_CAP_8BIT) };
            let stack_wmark = unsafe { uxTaskGetStackHighWaterMark(std::ptr::null_mut()) };
            let res = GenericReadingsResult::from([
                (
                    "timestamp".to_owned(),
                    Value {
                        kind: Some(Kind::NumberValue(reading as f64)),
                    },
                ),
                (
                    "internal".to_owned(),
                    Value {
                        kind: Some(Kind::NumberValue(free_internal as f64)),
                    },
                ),
                (
                    "spiram".to_owned(),
                    Value {
                        kind: Some(Kind::NumberValue(free_spiram as f64)),
                    },
                ),
                (
                    "stack".to_owned(),
                    Value {
                        kind: Some(Kind::NumberValue(stack_wmark as f64)),
                    },
                ),
            ]);
            //log::info!("FREE Internal: {} External: {}", free_internal, free_spiram);
            Ok(res)
        }
        #[cfg(not(target_os = "espidf"))]
        {
            let time = std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let sys = sysinfo::System::new();
            let res = GenericReadingsResult::from([
                (
                    "timestamp".to_owned(),
                    Value {
                        kind: Some(Kind::NumberValue(time as f64)),
                    },
                ),
                (
                    "internal".to_owned(),
                    Value {
                        kind: Some(Kind::NumberValue(sys.free_memory() as f64)),
                    },
                ),
                (
                    "spiram".to_owned(),
                    Value {
                        kind: Some(Kind::NumberValue(sys.free_swap() as f64)),
                    },
                ),
                (
                    "stack".to_owned(),
                    Value {
                        kind: Some(Kind::NullValue(0)),
                    },
                ),
            ]);
            Ok(res)
        }
    }
}

impl Status for TimestampSensor {
    fn get_status(&self) -> Result<Option<micro_rdk::google::protobuf::Struct>, StatusError> {
        Ok(Some(micro_rdk::google::protobuf::Struct {
            fields: HashMap::new(),
        }))
    }
}

impl Status for RandoSensors {
    fn get_status(&self) -> Result<Option<micro_rdk::google::protobuf::Struct>, StatusError> {
        Ok(Some(micro_rdk::google::protobuf::Struct {
            fields: HashMap::new(),
        }))
    }
}

impl Status for InternetSensor {
    fn get_status(&self) -> Result<Option<micro_rdk::google::protobuf::Struct>, StatusError> {
        Ok(Some(micro_rdk::google::protobuf::Struct {
            fields: HashMap::new(),
        }))
    }
}

impl Status for FatMessages {
    fn get_status(&self) -> Result<Option<micro_rdk::google::protobuf::Struct>, StatusError> {
        Ok(Some(micro_rdk::google::protobuf::Struct {
            fields: HashMap::new(),
        }))
    }
}
