#![no_std]
#![no_main]

mod usb;
mod hid;
mod util;

use core::default::Default;
use embassy_executor::Spawner;
use embassy_futures::join::{join, join3};
use embassy_stm32::Config;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::time::Hertz;
use usbd_hid::descriptor::KeyboardReport;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::Timer;
use {defmt_rtt as _, panic_probe as _};
use crate::hid::run_hid;
use crate::usb::setup_usb;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let mut config = Config::default();

    {
        use embassy_stm32::rcc::*;
        config.rcc.hse = Some(Hse {
            freq: Hertz(25_000_000),
            mode: HseMode::Oscillator,
        });
        config.rcc.pll_src = PllSource::HSE;
        config.rcc.pll = Some(Pll {
            prediv: PllPreDiv::DIV25,
            mul: PllMul::MUL336,
            divp: Some(PllPDiv::DIV4), // 25mhz / 25 * 336 / 4 = 84Mhz.
            divq: Some(PllQDiv::DIV7), // 25mhz / 25 * 336 / 7 = 48Mhz.
            divr: None,
        });
        config.rcc.ahb_pre = AHBPrescaler::DIV1;
        config.rcc.apb1_pre = APBPrescaler::DIV2;
        config.rcc.apb2_pre = APBPrescaler::DIV1;
        config.rcc.sys = Sysclk::PLL1_P;
    }

    let mut p = embassy_stm32::init(config);

    let channel: Channel<NoopRawMutex, KeyboardReport, 10> = Channel::new();
    let hid = run_hid(p.PA0, p.EXTI0, channel.sender());
    let usb = setup_usb(p.USB_OTG_FS, channel.receiver(), p.PA12, p.PA11);

    let mut led = Output::new(p.PC13, Level::Low, Speed::Medium);
    let blinky = async {
        loop {
            led.toggle();
            Timer::after_secs(1).await;
        }
    };

    join3(hid, usb, blinky).await;
}