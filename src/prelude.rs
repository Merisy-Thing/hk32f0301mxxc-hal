//! Prelude

pub use embedded_hal::digital::*;
pub use embedded_hal::delay::*;
pub use crate::gpio::GpioExt as _hk32_gpio_GpioExt;
pub use crate::rcc::RccExt as _hk32_hal_rcc_RccExt;
pub use crate::time::U32Ext as _hk32_hal_time_U32Ext;