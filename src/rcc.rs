use cortex_m_rt::pre_init;
use time::U32Ext;
use core::ptr;
use crate::pac::RCC;
use crate::time::{Hertz, self};

/// Extension trait that sets up the `RCC` peripheral
pub trait RccExt {
    /// Configure the clocks of the RCC peripheral
    fn configure(self) -> CFGR;
}

impl RccExt for RCC {
    fn configure(self) -> CFGR {
        CFGR {
            clock_src: SysClkSource::HSI,
            rcc: self,
        }
    }
}

#[pre_init]
unsafe fn system_init() {
    let p_rcc = RCC::ptr();

    /* Set HSION bit */
    (*p_rcc).cr().modify(|_, w| w.hsion().on());

    /* Reset SW[1:0], HPRE[3:0], PPRE[2:0] and MCOSEL[2:0] bits */
    (*p_rcc).cfgr().modify(|r, w| unsafe { w.bits(r.bits() & 0xF8FFB81C) });

    /* Reset USARTSW[1:0], I2CSW bits */
    (*p_rcc).cfgr3().modify(|r, w| unsafe { w.bits(r.bits() & 0xFFFFFFEC) });

    /* Disable all interrupts */
    (*p_rcc).cir().write(|w| unsafe { w.bits(0) });
}

/// Constrained RCC peripheral
pub struct Rcc {
    pub clocks: Clocks,
    pub(crate) regs: RCC,
}

/// RCC 
const HSI: u32 = 48_000_000; // Hz
const LSI: u32 = 60_000; // Hz

#[allow(clippy::upper_case_acronyms)]
enum SysClkSource {
    HSI,
    LSI,
    /// High-speed external clock(freq,bypassed)
    HSE(u32),
}

fn get_freq(c_src: &SysClkSource) -> u32 {
    // Select clock source based on user input and capability
    // Highest selected frequency source available takes precedent.
    match c_src {
        SysClkSource::HSE(freq) => *freq,
        SysClkSource::LSI => LSI,
        _ => HSI,
    }
}

// DO NOT CHANGE THIS FUNCTION
fn hsi_trimming_value_load(rcc: &mut RCC) {
    /* load HSI trimming value*/
    let hsivalue = unsafe { ptr::read_volatile(0x1ffff10c as *const u32 ) };
    let mut hsical;// = 0x26;
    let hsitrim;// = 0x20;
    let mut temp = rcc.cr().read().bits() & 0xffffc003;
    if (hsivalue & 0xFFFF) == (0xFFFF - ((hsivalue>>16) & 0xFFFF)) {
        hsical = (hsivalue) & 0xFF;
        hsical = hsical >> 2;
        hsitrim = (hsivalue >> 8) & 0xFF;
        temp |= hsitrim << 8; 
        temp |= hsical << 2;
        rcc.cr().write(|w| unsafe { w.bits(temp) });
    } 
}

// DO NOT CHANGE THIS FUNCTION
fn pmu_trimming_value_load(rcc: &mut RCC) {
    /* load PMU trimming value*/
    let bgpvalue = unsafe { ptr::read_volatile(0x1ffff114 as *const u32 ) };
    let ldovalue = unsafe { ptr::read_volatile(0x1ffff118 as *const u32 ) };
    let lpldovalue = unsafe { ptr::read_volatile(0x1ffff11c as *const u32 ) };
    //let mut bgptemp = unsafe { ptr::read_volatile(0x40007070 as *const u32 ) & 0xffffe0e0 };
    
    let lbgp = (bgpvalue>>8)&0x1F;
    let mbgp = (bgpvalue)&0x1F;
    let ldoruntemp = (ldovalue) & 0xff;
    let ldolprtemp = (ldovalue >>8) & 0xff; 
    let lpldolprtemp = (lpldovalue>>8) & 0xff;
    
    rcc.apbenr1().modify(|_, w| w.pwren().set_bit());
    
    if (bgpvalue&0xFFFF) == (0xFFFF - ((bgpvalue>>16)&0xFFFF)) {
        // BGP 
         let bgptemp = (lbgp<<8) | mbgp;
         unsafe { ptr::write_volatile(0x4000704c as *mut u32, 0x00001985) };
         unsafe { ptr::write_volatile(0x4000704c as *mut u32, 0x00000429) };
         unsafe { ptr::write_volatile(0x40007070 as *mut u32, bgptemp) };
         unsafe { ptr::write_volatile(0x4000704c as *mut u32, 0x0000FFFF) };
         unsafe { ptr::write_volatile(0x4000704c as *mut u32, 0x0000FFFF) };
    }
    if (ldovalue&0xFFFF) == (0xFFFF - ((ldovalue>>16)&0xFFFF)) {
        // LDO_RUN 
         unsafe { ptr::write_volatile(0x4000704c as *mut u32, 0x00001985) };
         unsafe { ptr::write_volatile(0x4000704c as *mut u32, 0x00000429) };
         unsafe { ptr::write_volatile(0x40007060 as *mut u32, ldoruntemp) };
         unsafe { ptr::write_volatile(0x40007064 as *mut u32, ldolprtemp) };
         unsafe { ptr::write_volatile(0x4000704c as *mut u32, 0x0000FFFF) };
         unsafe { ptr::write_volatile(0x4000704c as *mut u32, 0x0000FFFF) };
    }  
    if ((lpldovalue>>8)&0xFF) == (0xFF - ((lpldovalue>>24)&0xFF)) {
        // LPLDO_LPR 
         unsafe { ptr::write_volatile(0x4000704c as *mut u32, 0x00001985) };
         unsafe { ptr::write_volatile(0x4000704c as *mut u32, 0x00000429) };
         unsafe { ptr::write_volatile(0x4000706C as *mut u32, lpldolprtemp) };
         unsafe { ptr::write_volatile(0x4000704c as *mut u32, 0x0000FFFF) };
         unsafe { ptr::write_volatile(0x4000704c as *mut u32, 0x0000FFFF) };
    }
}

fn set_flash_wait(flash: &mut crate::pac::FLASH, freq: Hertz) {
    if freq <= 16000000.hz() {
        flash.acr().write(|w| unsafe { w.latency().bits(0) })
    } else if freq <= 32000000.hz() {
        flash.acr().write(|w| unsafe { w.latency().bits(1) })
    } else {
        flash.acr().write(|w| unsafe { w.latency().bits(2) })
    }
}

fn enable_clock(rcc: &mut RCC, flash: &mut crate::pac::FLASH, c_src: &SysClkSource) {
    let sw_to;
    // Enable the requested clock
    match c_src {
        SysClkSource::HSE(freq) => {
            rcc.cfgr4().modify(|_, w| unsafe { w.extclk_sel().bits(0) });
            rcc.cr().modify(|_, w| w.extclkon().on());
            while rcc.cr().read().extclkrdy().is_not_ready() {}

            set_flash_wait(flash, (*freq).hz());
            sw_to = 1;
        }
        SysClkSource::HSI => {
            rcc.cr().modify(|_, w| w.hsion().on());
            rcc.cfgr4().modify(|_, w| unsafe { w.flitfclk_pre().bits(7) });
            while rcc.cr().read().hsirdy().is_not_ready() {}
            
            set_flash_wait(flash, HSI.hz());
            sw_to = 0;
        }
        SysClkSource::LSI => {
            rcc.csr().modify(|_, w| w.lsion().set_bit());
            while rcc.csr().read().lsirdy().bit_is_clear() {}

            set_flash_wait(flash, LSI.hz());
            sw_to = 3;
        }
    }

    //set HCLK and PCLK prescaler
    rcc.cfgr().modify(|_, w|
        w.hpre().div1().ppre().div1()
    );
    
    //switch to target clock source
    rcc.cfgr().modify(|_, w|  
        w.sw().bits(sw_to)
    );

    while rcc.cfgr().read().sws().bits() != sw_to {}
}

pub struct CFGR {
    clock_src: SysClkSource,
    rcc: RCC,
}

impl CFGR {
    pub fn hse<F>(mut self, freq: F ) -> Self
    where
        F: Into<Hertz>,
    {
        self.clock_src = SysClkSource::HSE(freq.into().0);
        self
    }

    pub fn hsi(mut self) -> Self {
        self.clock_src = SysClkSource::HSI;
        self
    }

    pub fn lsi(mut self) -> Self {
        self.clock_src = SysClkSource::LSI;
        self
    }

    pub fn freeze(mut self, flash: &mut crate::pac::FLASH) -> Rcc {
        self::hsi_trimming_value_load(&mut self.rcc);
        self::pmu_trimming_value_load(&mut self.rcc);
        
        self::enable_clock(&mut self.rcc, flash,&self.clock_src);
        
        flash.int_vec_offset().write(|w| unsafe { w.bits(0) } );

        let freq = get_freq(&self.clock_src);

        Rcc {
            clocks: Clocks {
                hclk: Hertz(freq),
                pclk: Hertz(freq),
                sysclk: Hertz(freq),
            },
            regs: self.rcc,
        }
    }
}

/// Frozen clock frequencies
///
/// The existence of this value indicates that the clock configuration can no longer be changed
#[derive(Clone, Copy)]
pub struct Clocks {
    hclk: Hertz,
    pclk: Hertz,
    sysclk: Hertz,
}

impl Clocks {
    /// Returns the frequency of the AHB
    pub fn hclk(&self) -> Hertz {
        self.hclk
    }

    /// Returns the frequency of the APB
    pub fn pclk(&self) -> Hertz {
        self.pclk
    }

    /// Returns the system (core) frequency
    pub fn sysclk(&self) -> Hertz {
        self.sysclk
    }
}
