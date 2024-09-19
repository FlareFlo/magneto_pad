#![no_std]
#![no_main]

use core::default::Default;

use defmt::info;
use embassy_executor::Spawner;
use embassy_futures::join::{join, join3};
use embassy_stm32::{bind_interrupts, Config, Peripheral};
use embassy_stm32::adc::{Adc, AdcChannel, AnyAdcChannel};
use embassy_stm32::exti::Channel as AnyChannel;
use embassy_stm32::flash::{Flash, InterruptHandler as FInterruptHandler};
use embassy_stm32::flash::Bank1Region3;
use embassy_stm32::gpio::{AnyPin, Level, Output, Pin, Speed};
use embassy_stm32::peripherals::ADC1;
use embassy_stm32::time::Hertz;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::Timer;
use keyberon::action::Action;
use keyberon::key_code::{KbHidReport, KeyCode};
use keyberon::layout::{Event, Layout};
use usbd_hid::descriptor::KeyboardReport;
use crate::hid::{HidChannel, run_hid};
use crate::usb::setup_usb;

mod usb;
// mod hid;
mod util;
mod constants;
mod hid;

bind_interrupts!(struct Irqs {
    FLASH => FInterruptHandler;
});

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

    let channel: Channel<NoopRawMutex, KbHidReport, 10> = Channel::new();
    // let channel: HidChannel = Channel::new();
    let sender = channel.sender();
    // let hid = run_hid(p.PA0.degrade(), p.EXTI0.degrade(), channel.sender());
    let usb = setup_usb(p.USB_OTG_FS, channel.receiver(), p.PA12, p.PA11);

    let flash = Flash::new(p.FLASH, Irqs);
    let regions = flash.into_regions();
    let mut user_flash: Bank1Region3 = regions.bank1_region3;

    let mut reader = AnalogueReader::new(
        p.PA5.degrade(),
        p.PA4.degrade(),
        p.PA3.degrade(),
        p.PA2.degrade(),
        p.PA1.degrade(),
        [p.PA6.degrade_adc()],
        Adc::new(p.ADC1),
    );

    let scanner = async {
        let mut layout = Layout::new(&[
            &[
                &[
                    Action::KeyCode(KeyCode::A),
                    Action::KeyCode(KeyCode::S),
                    Action::KeyCode(KeyCode::D),
                    Action::KeyCode(KeyCode::W),
                ]
            ]
        ]);

        let mut keys = [KeyState {
            min: u16::MAX,
            max: u16::MIN,
            pressed: false,
            config: KeyConfig::Threshold(1500),
        }; 4];

        let mut a = [0; 4];
        let mut r = [false; 4];

        loop {
            for i in 0..4 {
                a[i] = reader.sample(i);
                keys[i].update(a[i]);

                let _ = layout.event(if keys[i].pressed {
                    Event::Press(0, i as u8)
                } else {
                    Event::Release(0, i as u8)
                });
            }

            let report: KbHidReport = layout.tick().collect();

            sender.send(report).await;

            // info!("k1: {:?}, k2: {:?}, k3: {:?}, k4: {:?}", keys[0].pressed, keys[1].pressed, keys[2].pressed, keys[3].pressed);

            Timer::after_micros(10).await;
            // Timer::after_millis(1).await;
        }
    };

    // usb.await;

    join(usb, scanner).await;
}

struct AnalogueReader<const AMOUNT: usize = 1> {
    channels: [AnyAdcChannel<ADC1>; AMOUNT],
    adc: Adc<'static, ADC1>,
    s0: Output<'static>,
    s1: Output<'static>,
    s2: Output<'static>,
    s3: Output<'static>,
    en: Output<'static>,
}

impl<const AMOUNT: usize> AnalogueReader<AMOUNT> {
    fn new(
        s0: AnyPin,
        s1: AnyPin,
        s2: AnyPin,
        s3: AnyPin,
        en: AnyPin,
        mut multiplexers: [AnyAdcChannel<ADC1>; AMOUNT],
        adc: Adc<'static, ADC1>
    ) -> Self {
        // low speed 8MHz
        // medium speed 50MHz
        // maximum high-speed 100MHz
        // very high-speed 180Mhz.

        Self {
            channels: multiplexers,
            adc,
            s0: Output::new(s0, Level::Low, Speed::Low),
            s1: Output::new(s1, Level::Low, Speed::Low),
            s2: Output::new(s2, Level::Low, Speed::Low),
            s3: Output::new(s3, Level::Low, Speed::Low),
            en: Output::new(en, Level::Low, Speed::Low),
        }
    }

    fn sample(&mut self, channel: usize) -> u16 {
        self.s0.set_level(Level::from(channel & 1 == 1));
        self.s1.set_level(Level::from((channel >> 1) & 1 == 1));
        self.s2.set_level(Level::from((channel >> 2) & 1 == 1));
        self.s3.set_level(Level::from((channel >> 3) & 1 == 1));

        self.adc.blocking_read(&mut self.channels[channel / 16])
    }
}

#[derive(Copy, Clone)]
enum  KeyConfig {
    // distance 0 - 400 (think about bigger range)
    Threshold(u16),
    RappidTrigger(u16),
}

// TODO convert to distance
#[derive(Copy, Clone)]
struct KeyState {
    pub min: u16,
    pub max: u16,
    pressed: bool,
    config: KeyConfig,
}

impl KeyState {
    fn update(&mut self, value: u16) {
        if value > self.max {
            self.max = value;
        }

        if value < self.min {
            self.min = value;
        }

        match self.config {
            KeyConfig::Threshold(v) => self.pressed = value < v,
            KeyConfig::RappidTrigger(_) => todo!(),
        }
    }

    // TODO adjust this (does not work correctly)
    // fn pressed_percent(&self, value: u16) -> f64 {
    //     let a = 1.0 / libm::cbrt(value as f64);
    //     let b = 1.0 / libm::cbrt(self.max as f64);
    //     let c = 1.0 / libm::cbrt(self.min as f64);
    //
    //     (a - b) / (c - b)
    // }
}

// #[embassy_executor::task]
// async fn switch_scan(mut reader: AnalogueReader) {
//     let mut keys = [KeyState {
//         min: 0,
//         max: 0,
//         pressed: false,
//         config: KeyConfig::Threshold(1500),
//     }; 4];
//
//     loop {
//         for i in 0..4 {
//             keys[i].update(reader.sample(i));
//         }
//
//
//     }
// }

// pub struct Repeater(u8);
//
// impl defmt::Format for Repeater {
//     fn format(&self, f: defmt::Formatter) {
//         for _ in 0..self.0 {
//             defmt::write!(
//                 f,
//                 "#",
//             )
//         }
//     }
// }