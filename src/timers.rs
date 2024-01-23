use crate::pac::{RCC, TIM1, TIM2, TIM6};
use nb;
use cast::{u16, u32};
use crate::rcc::Clocks;
use core::convert::Infallible;
use crate::time::Hertz;

pub trait CountDown {
    type Time;

    /// Starts a new count down
    fn start<T>(&mut self, count: T)
    where
        T: Into<Self::Time>;

    fn wait(&mut self) -> nb::Result<(), Infallible>;
}

pub trait Periodic {}

pub trait Cancel: CountDown {
    /// Error returned when a countdown can't be canceled.
    type Error;

    fn cancel(&mut self) -> Result<(), Self::Error>;
}

/// Hardware timers
pub struct Timer<TIM> {
    clocks: Clocks,
    tim: TIM,
    timeout: Hertz,
}

/// Interrupt events
pub enum Event {
    /// Timer timed out / count down ended
    TimeOut,
}

macro_rules! timers {
    ($($TIM:ident: ($tim:ident, $timXen:ident, $timXrst:ident, $apbenr:ident, $apbrstr:ident),)+) => {
        $(
            impl Periodic for Timer<$TIM> {}

            impl CountDown for Timer<$TIM> {
                type Time = Hertz;

                // NOTE(allow) `w.psc().bits()` is safe for TIM{6,7} but not for TIM{2,3,4} due to
                // some SVD omission
                #[allow(unused_unsafe)]
                fn start<T>(&mut self, timeout: T)
                where
                    T: Into<Hertz>,
                {
                    // pause
                    self.tim.cr1().modify(|_, w| w.cen().clear_bit());
                    // restart counter
                    self.tim.cnt().reset();

                    self.timeout = timeout.into();

                    let frequency = self.timeout.0;
                    let ticks = self.clocks.pclk().0 / frequency;

                    let psc = u16((ticks - 1) / (1 << 16)).unwrap();
                    self.tim.psc().write(|w| unsafe { w.psc().bits(psc) });

                    let arr = u16(ticks / u32(psc + 1)).unwrap();
                    self.tim.arr().write(|w| unsafe { w.bits(u32(arr)) });

                    // start counter
                    self.tim.cr1().modify(|_, w| w.cen().set_bit());
                }

                fn wait(&mut self) -> nb::Result<(), Infallible> {
                    if self.tim.sr().read().uif().bit_is_clear() {
                        Err(nb::Error::WouldBlock)
                    } else {
                        self.tim.sr().modify(|_, w| w.uif().clear_bit());
                        Ok(())
                    }
                }
            }

            impl Timer<$TIM> {
                // XXX(why not name this `new`?) bummer: constructors need to have different names
                // even if the `$TIM` are non overlapping (compare to the `free` function below
                // which just works)
                /// Configures a TIM peripheral as a periodic count down timer
                pub fn $tim<T>(tim: $TIM, timeout: T, clocks: Clocks) -> Self
                where
                    T: Into<Hertz>,
                {
                    // NOTE(unsafe) This executes only during initialisation
                    let rcc = unsafe { &(*RCC::ptr()) };
                    // enable and reset peripheral to a clean slate state
                    rcc.$apbenr().modify(|_, w| w.$timXen().set_bit());
                    rcc.$apbrstr().modify(|_, w| w.$timXrst().set_bit());
                    rcc.$apbrstr().modify(|_, w| w.$timXrst().clear_bit());

                    let mut timer = Timer {
                        clocks,
                        tim,
                        timeout: Hertz(0),
                    };
                    timer.start(timeout);

                    timer
                }

                /// Starts listening for an `event`
                pub fn listen(&mut self, event: Event) {
                    match event {
                        Event::TimeOut => {
                            // Enable update event interrupt
                            self.tim.dier().write(|w| w.uie().set_bit());
                        }
                    }
                }

                /// Stops listening for an `event`
                pub fn unlisten(&mut self, event: Event) {
                    match event {
                        Event::TimeOut => {
                            // Enable update event interrupt
                            self.tim.dier().write(|w| w.uie().clear_bit());
                        }
                    }
                }

                /// Releases the TIM peripheral
                pub fn free(self) -> $TIM {
                    // pause counter
                    self.tim.cr1().modify(|_, w| w.cen().clear_bit());
                    self.tim
                }
            }
        )+
    }
}

timers! {
    TIM1: (tim1, tim1en, tim1rst, apbenr2, apbrstr2),
}

timers! {
    TIM2: (tim2, tim2en, tim2rst, apbenr1, apbrstr1),
    TIM6: (tim6, tim6en, tim6rst, apbenr1, apbrstr1),
}
