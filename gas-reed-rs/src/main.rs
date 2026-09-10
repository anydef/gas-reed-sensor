
#![no_std]
#![no_main]

use cyw43_pio::{DEFAULT_CLOCK_DIVIDER, PioSpi};
use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::{DMA_CH0, DMA_CH1, PIO0};
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_rp::{bind_interrupts, dma};
use embassy_time::{Duration, Timer};
use panic_probe as _;
use static_cell::StaticCell;
use cyw43_setup::{CLM, FW, NVRAM};
bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
    DMA_IRQ_0 => dma::InterruptHandler<DMA_CH0>, dma::InterruptHandler<DMA_CH1>;
});

#[embassy_executor::task]
async fn cyw43_task(
    runner: cyw43::Runner<'static, cyw43::SpiBus<Output<'static>, PioSpi<'static, PIO0, 0>>>,
) -> ! {
    runner.run().await
}


#[embassy_executor::main(executor = "embassy_rp::executor::Executor", entry = "cortex_m_rt::entry")]
async fn main(spawner: Spawner) {

    let fw = &FW;
    let clm = &CLM;
    let nvram = &NVRAM;


    let peripherals = embassy_rp::init(Default::default());

    let pwr = Output::new(peripherals.PIN_23, Level::Low);
    let cs = Output::new(peripherals.PIN_25, Level::High);
    let mut pio = Pio::new(peripherals.PIO0, Irqs);
    // let mut led = Output::new(peripherals.PIN_25, Level::Low);
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        DEFAULT_CLOCK_DIVIDER,
        pio.irq0,
        cs,
        peripherals.PIN_24,
        peripherals.PIN_29,
        dma::Channel::new(peripherals.DMA_CH0, Irqs),
        // dma::Channel::new(peripherals.DMA_CH1, Irqs),
    );

    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = STATE.init(cyw43::State::new());
    let (_net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw, nvram).await;
    spawner.spawn(unwrap!(cyw43_task(runner)));

    control.init(clm).await;
    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave)
        .await;


    loop {
        info!("led on");
        control.gpio_set(0, true).await;
        // led.set_high();
        Timer::after(Duration::from_millis(1000)).await;
        info!("led off");

        control.gpio_set(0, false).await;

        // led.set_low();
        Timer::after(Duration::from_millis(500)).await;
    }
}