#![no_std]
#![no_main]

mod usb;
mod hid;
mod util;

use core::default::Default;
use defmt::info;
use embassy_executor::Spawner;
use embassy_futures::join::{join3};
use embassy_stm32::adc::Adc;
use embassy_stm32::Config;
use embassy_stm32::gpio::{Level, Output, Pin, Speed};
use embassy_stm32::time::Hertz;
use usbd_hid::descriptor::KeyboardReport;
use embassy_futures::join::join4;
use embassy_stm32::exti::Channel as AnyChannel;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Delay, Timer};
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

    let p = embassy_stm32::init(config);

    let channel: Channel<NoopRawMutex, KeyboardReport, 10> = Channel::new();
    let hid = run_hid(p.PA0.degrade(), p.EXTI0.degrade(), channel.sender());
    let usb = setup_usb(p.USB_OTG_FS, channel.receiver(), p.PA12, p.PA11);

    let mut delay = Delay;
    let mut adc = Adc::new(p.ADC1, &mut delay);
    let mut pin = p.PA4;
    let ana = async {
        let mut max = u16::MIN;
        let mut min = u16::MAX;
        loop {
            let curr = adc.read(&mut pin);

            if curr > max {
                max = curr;
            }

            if curr < min {
                min = curr;
            }

            let percentage = if max != min {
                ((curr - min) as f32 / (max - min) as f32) * 100.0
            } else {
                0.0
            };

            // info!("{}", Repeater(percentage as u8));
            info!("{} {}", min, max);
            Timer::after_millis(100).await;
        }
    };

    let mut led = Output::new(p.PC13, Level::Low, Speed::Medium);
    let blinky = async {
        loop {
            led.toggle();
            Timer::after_secs(1).await;
        }
    };

    join4(hid, usb, blinky, ana).await;
}

pub struct Repeater(u8);

impl defmt::Format for Repeater {
    fn format(&self, f: defmt::Formatter) {
        for _ in 0..self.0 {
            defmt::write!(
                f,
                "#",
            )
        }
    }
}