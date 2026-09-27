#![no_std]
#![no_main]
// extern crate alloc;

pub mod meter_view;
pub mod metrics;

// use alloc::string::ToString;
use core::fmt::{Debug, Write};
use core::num::ParseIntError;
use cyw43::{JoinOptions, new};
use cyw43_pio::{DEFAULT_CLOCK_DIVIDER, PioSpi};
use cyw43_setup::{CLM, FW, NVRAM};
use defmt::*;
use defmt_rtt as _;
use embassy_executor::_export::task_pool_align;
use embassy_executor::Spawner;
use embassy_net::dns::DnsSocket;
use embassy_net::tcp::client::{TcpClient, TcpClientState};
use embassy_net::{Config, Stack, StackResources};
use embassy_rp::clocks::RoscRng;
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::multicore::spawn_core1;
use embassy_rp::peripherals::{DMA_CH0, DMA_CH1, PIO0};
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_rp::{bind_interrupts, dma};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::{Channel, Receiver, Sender};
use embassy_time::{Duration, Timer};
use heapless::String;
use panic_probe as _;
use picoserve::routing::{get, post};
use picoserve::{AppBuilder, AppRouter, Router, make_static};
use picoserve::response::{IntoResponse, StatusCode};
use reqwless::client::{HttpClient, TlsConfig};
use static_cell::StaticCell;

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
    DMA_IRQ_0 => dma::InterruptHandler<DMA_CH0>, dma::InterruptHandler<DMA_CH1>;
});
enum ReedState {
    Contact,
}
static REED_CHANNEL: Channel<ThreadModeRawMutex, ReedState, 64> = Channel::new();

#[embassy_executor::task]
async fn cyw43_task(
    runner: cyw43::Runner<'static, cyw43::SpiBus<Output<'static>, PioSpi<'static, PIO0, 0>>>,
) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, cyw43::NetDriver<'static>>) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn reed_task(
    mut pin: Input<'static>,
    control: Sender<'static, ThreadModeRawMutex, ReedState, 64>,
) -> ! {
    loop {
        pin.wait_for_rising_edge().await;
        control.send(ReedState::Contact).await;
        Timer::after(Duration::from_secs(3)).await;
        metrics::meter_metrics::get().meter_tick.inc();
        metrics::meter_metrics::get().meter_absolute.add(1);
        // metrics::get().meter_tick.inc();
        info!("touchdown!");
    }
}

#[embassy_executor::task]
async fn led_task(
    mut led: Output<'static>,
    receiver: Receiver<'static, ThreadModeRawMutex, ReedState, 64>,
) -> ! {
    loop {
        match receiver.receive().await {
            ReedState::Contact => {
                led.set_high();
                Timer::after(Duration::from_millis(500)).await;
                led.set_low();
            }
        }
    }
}

const WEB_TASK_POOL_SIZE: usize = 4;
const CONFIG: picoserve::Config = picoserve::Config::const_default().keep_connection_alive();


#[embassy_executor::task(pool_size= WEB_TASK_POOL_SIZE)]
async fn web_task(task_id: usize, stack: embassy_net::Stack<'static>) -> ! {
    let app = Router::new()
        .route("/", get(meter_view::hello_handler))
        .route("/meter", post(|form_param: heapless::String<16> | async move {
            info!("Form param {}", form_param);
            let (name, value) = form_param.split_once("=").expect("Failed parsing form value");
            let mut buf: String<32> = String::new();
            match value.parse::<i64>() {
                Ok(v) => {
                    metrics::meter_metrics::get().meter_absolute.set(v);
                    (StatusCode::SEE_OTHER, ("Location", "/"), "")

                }
                Err(v) => {
                    error!("failed parsing value {}", defmt::Debug2Format(&v));
                    (StatusCode::SEE_OTHER, ("Location", "/?validation=error"), "")

                }
            }
        }))
        .route(
            "/metrics",
            get(|| async {
                info!("Requested metrics");
                picoserve::response::chunked::ChunkedResponse::new(
                    metrics::meter_metrics::MetricsResponse,
                )
            }),
        );
    let port = 80;
    let mut tcp_rx_buffer = [0; 1024];
    let mut tcp_tx_buffer = [0; 1024];
    let mut http_buffer = [0; 2048];
    picoserve::Server::new(&app, &CONFIG, &mut http_buffer)
        .listen_and_serve(task_id, stack, port, &mut tcp_rx_buffer, &mut tcp_tx_buffer)
        .await
        .into_never()
}

#[embassy_executor::main(
    executor = "embassy_rp::executor::Executor",
    entry = "cortex_m_rt::entry"
)]
async fn main(spawner: Spawner) {
    info!("Starting...");
    let wifi_ssid: &'static str = env!("SSID");
    let wifi_password: &'static str = env!("PASSWORD");
    let fw = &FW;
    let clm = &CLM;
    let nvram = &NVRAM;

    let peripherals = embassy_rp::init(Default::default());
    let mut rng = RoscRng;

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

    // make_static!()
    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = STATE.init(cyw43::State::new());
    let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw, nvram).await;
    spawner.spawn(unwrap!(cyw43_task(runner)));

    control.init(clm).await;
    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave)
        .await;

    let config = Config::dhcpv4(Default::default());
    let seed = rng.next_u64();

    static RESOURCES: StaticCell<StackResources<8>> = StaticCell::new();
    let (network_stack, runner) = embassy_net::new(
        net_device,
        config,
        RESOURCES.init(StackResources::new()),
        seed,
    );

    spawner.spawn(unwrap!(net_task(runner)));

    while let Err(err) = control
        .join(wifi_ssid, JoinOptions::new(wifi_password.as_bytes()))
        .await
    {
        info!("join failed: {:?}", err);
    }

    info!("waiting for link...");
    network_stack.wait_link_up().await;

    info!("waiting for DHCP...");
    network_stack.wait_config_up().await;

    // And now we can use it!
    info!("Stack is up!");

    // access_website(network_stack, rng.next_u64()).await;

    let led = Output::new(peripherals.PIN_15, Level::Low);

    let reed = Input::new(peripherals.PIN_14, Pull::Down);
    spawner.spawn(unwrap!(reed_task(reed, REED_CHANNEL.sender())));
    spawner.spawn(unwrap!(led_task(led, REED_CHANNEL.receiver())));

    for task_id in 0..WEB_TASK_POOL_SIZE {
        spawner.spawn(web_task(task_id, network_stack).unwrap());
    }
    info!(
        "Assigned IP: {}",
        network_stack.config_v4().unwrap().address
    );
    loop {
        // info!("led on");
        control.gpio_set(0, true).await;
        // led.set_high();
        // led.set_high();
        Timer::after(Duration::from_millis(1000)).await;
        // info!("led off");

        control.gpio_set(0, false).await;
        // led.set_low();

        // led.set_low();
        Timer::after(Duration::from_millis(5000)).await;
    }
}

async fn access_website(stack: Stack<'_>, tls_seed: u64) {
    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];
    let dns = DnsSocket::new(stack);
    let tcp_state = TcpClientState::<1, 4096, 4096>::new();
    let tcp = TcpClient::new(stack, &tcp_state);

    let tls = TlsConfig::new(
        tls_seed,
        &mut rx_buffer,
        &mut tx_buffer,
        reqwless::client::TlsVerify::None,
    );

    let mut client = HttpClient::new_with_tls(&tcp, &dns, tls);
    let mut buffer = [0u8; 4096];
    let mut http_req = client
        .request(
            reqwless::request::Method::GET,
            "https://jsonplaceholder.typicode.com/posts/1",
        )
        .await
        .unwrap();
    let response = http_req.send(&mut buffer).await.unwrap();

    info!("Got response");
    let res = response.body().read_to_end().await.unwrap();

    let content = core::str::from_utf8(res).unwrap();
    println!("{}", content);
}
