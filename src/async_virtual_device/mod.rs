use crate::{metrics, settings, Credentials, DownlinkSender, Result};
use lorawan_device::async_device::radio::{RxConfig, RxQuality, RxStatus, TxConfig};
use lorawan_device::async_device::{
    radio::{PhyRxTx, Timer},
    Device, NetworkCredentials, Timings,
};
use lorawan_device::default_crypto::DefaultFactory;
use lorawan_device::region::Configuration;
use lorawan_device::{AppEui, AppKey, DevEui};
use semtech_udp::client_runtime::{ClientTx, DownlinkRequest};
use std::str::FromStr;
use std::time::Instant;
use tokio::time::{sleep, Duration};

pub struct VirtualDevice {
    label: String,
    metrics_sender: metrics::Sender,
    time: Instant,
    client_tx: ClientTx,
    rejoin_frames: u32,
    secs_between_transmits: u64,
    secs_between_join_transmits: u64,
    credentials: NetworkCredentials,
    device: Device<VirtualRadio, DefaultFactory, VirtualTimer, rand_core::OsRng, 512, 4>,
}

#[derive(Clone)]
pub struct DS;

impl DownlinkSender for DS {
    async fn send(&self, _downlink: Box<DownlinkRequest>, _delayed_for: u64) -> Result {
        todo!()
    }
}

struct VirtualRadio;
impl PhyRxTx for VirtualRadio {
    type PhyError = ();
    const ANTENNA_GAIN: i8 = 0;
    const MAX_RADIO_POWER: u8 = 0;

    async fn tx(
        &mut self,
        config: TxConfig,
        buf: &[u8],
    ) -> std::result::Result<u32, Self::PhyError> {
        todo!()
    }

    async fn setup_rx(&mut self, config: RxConfig) -> std::result::Result<(), Self::PhyError> {
        todo!()
    }

    async fn rx_continuous(
        &mut self,
        rx_buf: &mut [u8],
    ) -> std::result::Result<(usize, RxQuality), Self::PhyError> {
        todo!()
    }

    async fn rx_single(&mut self, buf: &mut [u8]) -> std::result::Result<RxStatus, Self::PhyError> {
        todo!()
    }
}

impl Timings for VirtualRadio {
    fn get_rx_window_lead_time_ms(&self) -> u32 {
        todo!()
    }
}

struct VirtualTimer(Instant);

impl Timer for VirtualTimer {
    fn reset(&mut self) {
        self.0 = Instant::now();
    }

    async fn at(&mut self, millis: u64) {
        let delay = self.0.elapsed().as_millis() as u64 - millis;
        if delay > 0 {
            sleep(Duration::from_millis(delay)).await;
        }
    }

    async fn delay_ms(&mut self, millis: u64) {
        tokio::time::sleep(Duration::from_millis(millis)).await;
    }
}

impl VirtualDevice {
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        label: String,
        time: Instant,
        client_tx: ClientTx,
        credentials: Credentials,
        metrics_sender: metrics::Sender,
        rejoin_frames: u32,
        secs_between_transmits: u64,
        secs_between_join_transmits: u64,
        region: settings::Region,
    ) -> Result<(DS, Self)> {
        Ok((
            DS {},
            Self {
                label,
                metrics_sender,
                rejoin_frames,
                time,
                client_tx,
                secs_between_transmits,
                secs_between_join_transmits,
                credentials: NetworkCredentials::new(
                    AppEui::from_str(&credentials.app_eui)?,
                    DevEui::from_str(&credentials.dev_eui)?,
                    AppKey::from_str(&credentials.app_key)?,
                ),
                device: Device::new(
                    Configuration::new(region.into()),
                    VirtualRadio {},
                    VirtualTimer(time),
                    rand_core::OsRng,
                ),
            },
        ))
    }

    pub async fn run(mut self) -> Result {
        Ok(())
    }
}
