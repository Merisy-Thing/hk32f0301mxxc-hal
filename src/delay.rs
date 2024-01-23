//! Delays

use cortex_m::peripheral::syst::SystClkSource;
use cortex_m::peripheral::SYST;

use embedded_hal::delay::DelayNs;
use crate::rcc::Clocks;

/// System timer (SysTick) as a delay provider
pub struct Delay {
    clocks: Clocks,
    syst: SYST,
}

impl Delay {
    /// Configures the system timer (SysTick) as a delay provider
    pub fn new(mut syst: SYST, clocks: Clocks) -> Self {
        if !syst.is_counter_enabled() {
            syst.set_reload(clocks.sysclk().0 / 1000 - 1);//1ms
            syst.clear_current();
            syst.set_clock_source(SystClkSource::Core);
            syst.enable_counter();
        }

        Delay { syst, clocks }
    }

    pub fn free(self) -> SYST {
        self.syst
    }

    fn delay_limit_ns(&mut self, ns: u32) {// 500_000ns (500us) max
        let delay = (ns * (self.clocks.sysclk().0 / 1_000_000)) / 1000;
        let start = SYST::get_current();

        if start > delay {
            while (start - SYST::get_current()) < delay {}
        } else {
            let end_val = SYST::get_reload() - (delay - start);
            let mut curr_val;
            loop {
                curr_val = SYST::get_current();
                if (curr_val > start) && (curr_val < end_val) {
                    break;
                }
            }
        }
    }
}

impl DelayNs for Delay {
    fn delay_ns(&mut self, mut ns: u32) {
        const MAX_LIMIT_NANO: u32 = 500_000;

        while ns > MAX_LIMIT_NANO {
            ns -= MAX_LIMIT_NANO;
            self.delay_limit_ns(MAX_LIMIT_NANO);
        }
        
        self.delay_limit_ns(ns);
    }
}

