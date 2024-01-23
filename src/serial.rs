use crate::pac::{RCC, UART1, UART2};
use crate::rcc::Clocks;
use crate::time::Bps;

use core::ptr;
use embedded_hal_nb;
use nb;

/// Interrupt event
pub enum Event {
    /// New data has been received
    Rxne,
    /// New data can be sent
    Txe,
}

/// Serial error
#[derive(Debug)]
pub enum Error {
    /// Framing error
    Framing,
    /// Noise error
    Noise,
    /// RX buffer overrun
    Overrun,
    /// Parity check error
    Parity,
    #[doc(hidden)]
    _Extensible,
}

pub trait Pins<UART> {}

/// Serial abstraction
pub struct Serial<UART> {
    uart: UART,
}

macro_rules! uart {
    ($($UART:ident: ($uart:ident, $uartXen:ident, $apbenr:ident),)+) => {
        $(
            /// UART
            impl Serial<$UART> {
                pub fn $uart(uart: $UART, baud_rate: Bps, clocks: Clocks) -> Self {
                    // NOTE(unsafe) This executes only during initialisation
                    let rcc = unsafe { &(*RCC::ptr()) };

                    /* Enable clock for UART */
                    rcc.$apbenr().modify(|_, w| w.$uartXen().set_bit());

                    // Calculate correct baudrate divisor on the fly
                    let brr = clocks.pclk().0 / baud_rate.0;
                    uart.brr().write(|w| unsafe { w.bits(brr) });

                    /* Reset other registers to disable advanced UART features */
                    uart.cr2().reset();
                    uart.cr3().reset();

                    /* Enable transmission and receiving */
                    uart.cr1().modify(|_, w| unsafe { w.bits(0xD) });

                    Serial { uart }
                }

                pub fn release(self) -> $UART {
                    (self.uart)
                }
            }

			impl embedded_hal_nb::serial::ErrorType for Serial<$UART> {
			    type Error = embedded_hal_nb::serial::ErrorKind;
			}

            impl embedded_hal_nb::serial::Read<u8> for Serial<$UART> {
                fn read(&mut self) -> nb::Result<u8, Self::Error> {
                    // NOTE(unsafe) atomic read with no side effects
                    let isr = unsafe { (*$UART::ptr()).isr().read() };

                    Err(if isr.pe().bit_is_set() {
                        nb::Error::Other(Self::Error::Parity)
                    } else if isr.fe().bit_is_set() {
                        nb::Error::Other(Self::Error::FrameFormat)
                    } else if isr.nf().bit_is_set() {
                        nb::Error::Other(Self::Error::Noise)
                    } else if isr.ore().bit_is_set() {
                        nb::Error::Other(Self::Error::Overrun)
                    } else if isr.rxne().bit_is_set() {
                        // NOTE(read_volatile) see `write_volatile` below
                        return Ok(unsafe { ptr::read_volatile(&(*$UART::ptr()).rdr() as *const _ as *const _) });
                    } else {
                        nb::Error::WouldBlock
                    })
                }
            }

            impl embedded_hal_nb::serial::Write<u8> for Serial<$UART> {
                fn flush(&mut self) -> nb::Result<(), Self::Error> {
                    // NOTE(unsafe) atomic read with no side effects
                    let isr = unsafe { (*$UART::ptr()).isr().read() };

                    if isr.tc().bit_is_set() {
                        Ok(())
                    } else {
                        Err(nb::Error::WouldBlock)
                    }
                }

                fn write(&mut self, byte: u8) -> nb::Result<(), Self::Error> {
                    // NOTE(unsafe) atomic read with no side effects
                    let isr = unsafe { (*$UART::ptr()).isr().read() };

                    if isr.txe().bit_is_set() {
                        // NOTE(unsafe) atomic write to stateless register
                        // NOTE(write_volatile) 8-bit write that's not possible through the svd2rust API
                        unsafe { (*$UART::ptr()).tdr().write(|w| w.tdr().bits(byte as u16)) }
                        Ok(())
                    } else {
                        Err(nb::Error::WouldBlock)
                    }
                }
            }

        )+
    }
}



uart! {
    UART1: (uart1, uart1en, apbenr2),
    UART2: (uart2, uart2en, apbenr1),
}
