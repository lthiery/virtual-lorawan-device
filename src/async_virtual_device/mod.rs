pub mod radio;

use radio::*;

use crate::{metrics, settings, Credentials, DownlinkSender, Result};

use log::{debug, error, info, warn};
use lorawan_device::async_device::{
    radio::{PhyRxTx, RxConfig, Timer},
    Device, DeviceHandler, Downlink, JoinResponse, NetworkCredentials, SendResponse, Timings,
};
use lorawan_device::default_crypto::DefaultFactory;
use lorawan_device::region::Configuration;
use lorawan_device::{AppEui, AppKey, DevEui, JoinMode};

pub use semtech_udp::client_runtime::{ClientTx, DownlinkRequest};
use std::str::FromStr;
use std::time::Instant;
use tokio::{
    sync::mpsc,
    time::{sleep, Duration},
};

struct DeviceImpl {
    pub state: State,
    pub override_tx_period: Option<u16>,
}

impl Default for DeviceImpl {
    fn default() -> Self {
        Self {
            state: State::NotJoined,
            override_tx_period: None,
        }
    }
}

impl lorawan_device::async_device::DeviceHandler for DeviceImpl {}

impl lorawan_device::async_device::CertificationHandler for DeviceImpl {
    fn reset_device(&mut self) {
        // TODO: tear everything down and start again...
        // ...and we also need to reset timer to send join packet immediately
        self.state = State::NotJoined;
        info!("'Restarting' device...");
    }

    fn reset_mac(&mut self) {
        // TODO: tear everything down and start again...
        // ...and we also need to reset timer to send join packet immediately
        self.state = State::NotJoined;
        info!("Restarting mac layer...");
    }

    fn override_periodicity(&mut self, seconds: Option<u16>) {
        self.override_tx_period = seconds;
        info!("Updating tx_period to {:?}", seconds);
    }
}

pub struct VirtualDevice {
    label: String,
    metrics_sender: metrics::Sender,
    rejoin_frames: u32,
    secs_between_transmits: u64,
    secs_between_join_transmits: u64,
    credentials: NetworkCredentials,
    device:
        Device<DeviceImpl, VirtualRadio, DefaultFactory, VirtualTimer, rand_core::OsRng, 512, 4>,
}

enum State {
    Joined,
    NotJoined,
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
        let (sender, receiver) = mpsc::channel(100);
        let dev_impl = DeviceImpl::default();
        let mut device = Device::new(
            dev_impl,
            Configuration::new(region.into()),
            VirtualRadio {
                receiver,
                client_tx,
                time,
                rx_config: None,
            },
            VirtualTimer(time),
            rand_core::OsRng,
        );
        device.enable_class_c();
        Ok((
            DS(sender),
            Self {
                label,
                metrics_sender,
                rejoin_frames,
                secs_between_transmits,
                secs_between_join_transmits,
                credentials: NetworkCredentials::new(
                    AppEui::from_str(&credentials.app_eui)?,
                    DevEui::from_str(&credentials.dev_eui)?,
                    AppKey::from_str(&credentials.app_key)?,
                ),
                device,
            },
        ))
    }

    pub async fn run(mut self) -> Result {
        // stagger the starts slightly
        let random = rand::random::<u64>() % 1000;
        sleep(Duration::from_millis(random)).await;

        loop {
            let result = match self.device.device.state {
                State::NotJoined => self.do_join().await,
                State::Joined => self.do_send().await,
            };
            let mut duration = match result {
                Ok(d) => d,
                Err(e) => {
                    error!("{} error: {:?}", self.label, e);
                    Duration::from_secs(0)
                }
            };

            // Override duration
            if let Some(d) = self.device.device.override_tx_period {
                duration = Duration::from_secs(d.into());
            }

            tokio::select!(
                _ = sleep(duration) => {},
                result = self.device.rxc_listen() => {
                    match result {
                        Ok(response) => {
                            println!("response: {:?}", response);
                        }
                        Err(e) => {
                            error!("{} error: {:?}", self.label, e);
                        }
                    }
                    while let Some(downlink) = self.device.take_downlink() {
                        self.handle_downlink(downlink, true);
                    }
                }
            )
        }
    }

    async fn do_join(&mut self) -> Result<Duration> {
        let join_response = self
            .device
            .join(&JoinMode::OTAA {
                deveui: self.credentials.deveui().clone(),
                appeui: self.credentials.appeui().clone(),
                appkey: self.credentials.appkey().clone(),
            })
            .await
            .map_err(|e| crate::Error::AsyncLorawanRadio(e))?;

        match join_response {
            JoinResponse::JoinSuccess => {
                self.device.device.state = State::Joined;
                info!("{} joined successfully", self.label);
                Ok(Duration::from_secs(0))
            }
            JoinResponse::NoJoinAccept => {
                error!("{} failed to join", self.label);
                Ok(Duration::from_secs(self.secs_between_join_transmits))
            }
        }
    }

    async fn do_send(&mut self) -> Result<Duration> {
        let send_response = self
            .device
            .send(
                &vec![
                    rand::random(),
                    rand::random(),
                    rand::random(),
                    rand::random(),
                ],
                5,
                true,
            )
            .await
            .map_err(|e| crate::Error::AsyncLorawanRadio(e))?;
        match send_response {
            SendResponse::SessionExpired => {}
            SendResponse::NoAck => {}
            SendResponse::DownlinkReceived(fcnt) => {
                while let Some(downlink) = self.device.take_downlink() {
                    self.handle_downlink(downlink, false);
                }
            }
            SendResponse::RxComplete => {}
        }
        println!("{}", self.secs_between_transmits);
        Ok(Duration::from_secs(self.secs_between_transmits))
    }

    fn handle_downlink(&mut self, downlink: Downlink, is_class_c: bool) {
        let data_len = downlink.data.len();
        let fport = downlink.fport;
        info!(
            "{} downlink received: len = {data_len}, fport = {fport}, class_c = {is_class_c}",
            self.label
        );
        info!("downlink data: {:?}", downlink.data);
    }
}
