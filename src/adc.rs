use crate::{ gpio::*,pac::ADC };

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ALIGN {
    RIGHT = 0,
    LEFT = 1,
}
impl From<ALIGN> for bool {
    #[inline(always)]
    fn from(variant: ALIGN) -> Self {
        variant as u8 != 0
    }
}

pub trait Channel<ADC> {
    /// Channel ID type
    ///
    /// A type used to identify this ADC channel. For example, if the ADC has eight channels, this
    /// might be a `u8`. If the ADC has multiple banks of channels, it could be a tuple, like
    /// `(u8: bank_id, u8: channel_id)`.
    type ID;

    /// Get the specific ID that identifies this channel, for example `0_u8` for the first ADC
    /// channel, if Self::ID is u8.
    fn channel() -> Self::ID;

    // `channel` is a function due to [this reported
    // issue](https://github.com/rust-lang/rust/issues/54973). Something about blanket impls
    // combined with `type ID; const CHANNEL: Self::ID;` causes problems.
    //const CHANNEL: Self::ID;
}

pub trait OneShot<ADC, Word, Pin: Channel<ADC>> {
    /// Error type returned by ADC methods
    type Error;

    /// Request that the ADC begin a conversion on the specified pin
    ///
    /// This method takes a `Pin` reference, as it is expected that the ADC will be able to sample
    /// whatever channel underlies the pin.
    fn read(&mut self, pin: &mut Pin) -> nb::Result<Word, Self::Error>;
}

/// Analog to Digital converter interface
pub struct Adc {
    rb: ADC,
    sample_time: AdcSampleTime,
    align: AdcAlign
}

#[derive(Clone, Copy, Debug, PartialEq)]
/// ADC Sampling time
///
/// Options for the sampling time, each is T + 0.5 ADC clock cycles.
pub enum AdcSampleTime {
    /// 1.5 cycles sampling time
    T1,
    /// 7.5 cycles sampling time
    T7,
    /// 13.5 cycles sampling time
    T13,
    /// 28.5 cycles sampling time
    T28,
    /// 41.5 cycles sampling time
    T41,
    /// 55.5 cycles sampling time
    T55,
    /// 71.5 cycles sampling time
    T71,
    /// 239.5 cycles sampling time
    T239,
}

impl AdcSampleTime {
    /// Get the default sample time (currently 239.5 cycles)
    pub fn default() -> Self {
        AdcSampleTime::T239
    }
}

impl From<AdcSampleTime> for u8 {
    fn from(val: AdcSampleTime) -> Self {
        match val {
            AdcSampleTime::T1 => 0, //CYCLES1_5,
            AdcSampleTime::T7 => 1, //CYCLES7_5,
            AdcSampleTime::T13 => 2, //CYCLES13_5,
            AdcSampleTime::T28 => 3, //CYCLES28_5,
            AdcSampleTime::T41 => 4, //CYCLES41_5,
            AdcSampleTime::T55 => 5, //CYCLES55_5,
            AdcSampleTime::T71 => 6, //CYCLES71_5,
            AdcSampleTime::T239 => 7, //CYCLES239_5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
/// ADC Result Alignment
pub enum AdcAlign {
    /// Left aligned results (most significant bits)
    ///
    /// Results in all precisions returning a value in the range 0-65535.
    /// Depending on the precision the result will step by larger or smaller
    /// amounts.
    Left,
    /// Right aligned results (least significant bits)
    ///
    /// Results in all precisions returning values from 0-(2^bits-1) in
    /// steps of 1.
    Right,
}

impl AdcAlign {
    /// Get the default alignment (currently right aligned)
    pub fn default() -> Self {
        AdcAlign::Right
    }
}

impl From<AdcAlign> for bool {
    fn from(val: AdcAlign) -> Self {
        match val {
            AdcAlign::Left => true,
            AdcAlign::Right => false,
        }
    }
}

macro_rules! adc_pins {
    ($($pin:ty => $chan:expr),+ $(,)*) => {
        $(
            impl Channel<Adc> for $pin {
                type ID = u8;

                fn channel() -> u8 { $chan }
            }
        )+
    };
}

adc_pins!(
    gpiod::PD5<Analog> => 0_u8,
    gpiod::PD6<Analog> => 1_u8,
    gpioc::PC4<Analog> => 2_u8,
    gpiod::PD3<Analog> => 3_u8,
    gpiod::PD2<Analog> => 4_u8,
    gpiod::PD1<Analog> => 5_u8,
    gpioc::PC6<Analog> => 6_u8,
);



#[derive(Debug, Default)]
/// Internal temperature sensor (ADC Channel 16)
pub struct Vpmu;

#[derive(Debug, Default)]
/// Internal voltage reference (ADC Channel 17)
pub struct VRef;

adc_pins!(
    Vpmu => 7_u8,
    VRef => 8_u8,
);

impl Vpmu {
    /// Init a new VTemp
    pub fn new() -> Self {
        Vpmu::default()
    }

}

impl VRef {
    /// Init a new VRef
    pub fn new() -> Self {
        VRef::default()
    }

    /// Enable the internal voltage reference, remember to disable when not in use.
    pub fn enable(&mut self, adc: &mut Adc) {
        adc.rb.ccr().modify(|_, w| w.vrefen().set_bit());
    }

    /// Disable the internal reference voltage.
    pub fn disable(&mut self, adc: &mut Adc) {
        adc.rb.ccr().modify(|_, w| w.vrefen().clear_bit());
    }

    /// Returns if the internal voltage reference is enabled.
    pub fn is_enabled(&self, adc: &Adc) -> bool {
        adc.rb.ccr().read().vrefen().bit_is_set()
    }

    /// Reads the value of VDDA in milli-volts
    pub fn read_vdda(adc: &mut Adc) -> u16 {
        //let vrefint_cal = u32::from(unsafe { ptr::read(VREFCAL) });
        let mut vref = Self::new();

        let prev_cfg = adc.default_cfg();

        let vref_val: u32 = if vref.is_enabled(adc) {
            adc.read(&mut vref).unwrap()
        } else {
            vref.enable(adc);

            let ret = adc.read(&mut vref).unwrap();

            vref.disable(adc);
            ret
        };

        adc.restore_cfg(prev_cfg);

        (4095 * 1200 / vref_val) as u16
    }
}

/// A stored ADC config, can be restored by using the `Adc::restore_cfg` method
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct StoredConfig(AdcSampleTime, AdcAlign);

impl Adc {
    /// Init a new Adc
    ///
    /// Sets all configurable parameters to defaults, enables the HSI14 clock
    /// for the ADC if it is not already enabled and performs a boot time
    /// calibration. As such this method may take an appreciable time to run.
    pub fn new(adc: ADC) -> Self {
        let mut s = Self {
            rb: adc,
            sample_time: AdcSampleTime::default(),
            align: AdcAlign::default()
        };
        s.select_clock();
        s
    }

    /// Saves a copy of the current ADC config
    pub fn save_cfg(&mut self) -> StoredConfig {
        StoredConfig(self.sample_time, self.align)
    }

    /// Restores a stored config
    pub fn restore_cfg(&mut self, cfg: StoredConfig) {
        self.sample_time = cfg.0;
        self.align = cfg.1;
    }

    /// Resets the ADC config to default, returning the existing config as
    /// a stored config.
    pub fn default_cfg(&mut self) -> StoredConfig {
        let cfg = self.save_cfg();
        self.sample_time = AdcSampleTime::default();
        self.align = AdcAlign::default();
        cfg
    }

    /// Set the Adc sampling time
    ///
    /// Options can be found in [AdcSampleTime](crate::adc::AdcSampleTime).
    pub fn set_sample_time(&mut self, t_samp: AdcSampleTime) {
        self.sample_time = t_samp;
    }

    /// Set the Adc result alignment
    ///
    /// Options can be found in [AdcAlign](crate::adc::AdcAlign).
    pub fn set_align(&mut self, align: AdcAlign) {
        self.align = align;
    }

    /// Returns the largest possible sample value for the current settings
    pub fn max_sample(&self) -> u16 {
        match self.align {
            AdcAlign::Left => u16::max_value(),
            AdcAlign::Right => (1 << 12) - 1,
        }
    }

    /// Read the value of a channel and converts the result to milli-volts
    pub fn read_abs_mv<PIN: Channel<Adc, ID = u8>>(&mut self, pin: &mut PIN) -> u16 {
        let vdda = u32::from(VRef::read_vdda(self));
        let v: u32 = self.read(pin).unwrap();
        let max_samp = u32::from(self.max_sample());

        (v * vdda / max_samp) as u16
    }

    fn select_clock(&mut self) {
        self.rb.cfgr2().write(|w| unsafe { w.bits(2) }); //SynClkDiv4
    }

    fn power_up(&mut self) {
        if self.rb.isr().read().adrdy().bit_is_set() {
            self.rb.isr().modify(|_, w| w.adrdy().clear_bit());
        }
        self.rb.cr().modify(|_, w| w.aden().set_bit());
        while self.rb.isr().read().adrdy().bit_is_clear() {}
    }

    fn power_down(&mut self) {
        self.rb.cr().modify(|_, w| w.adstp().set_bit());
        while self.rb.cr().read().adstp().bit_is_set() {}
        self.rb.cr().modify(|_, w| w.addis().set_bit());
        while self.rb.cr().read().aden().bit_is_set() {}
    }

    fn convert(&mut self, chan: u8) -> u16 {
        self.rb.chselr().write(|w| unsafe { w.bits(1_u32 << chan) });

        self.rb.smpr()
            .modify(|_, w| unsafe { w.smp().bits(self.sample_time.into()) });
        self.rb.cfgr1().modify(|_, w| w.align().bit(self.align.into()) );

        self.rb.cr().modify(|_, w| w.adstart().set_bit());
        while self.rb.isr().read().eoc().bit_is_clear() {}

        let res = self.rb.dr().read().bits() as u16;
        if self.align == AdcAlign::Left {
            res << 8
        } else {
            res
        }
    }
}

impl<WORD, PIN> OneShot<Adc, WORD, PIN> for Adc
where
    WORD: From<u16>,
    PIN: Channel<Adc, ID = u8>,
{
    type Error = ();

    fn read(&mut self, _pin: &mut PIN) -> nb::Result<WORD, Self::Error> {
        self.power_up();
        let res = self.convert(PIN::channel());
        self.power_down();
        Ok(res.into())
    }
}
