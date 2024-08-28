pub mod radio;

use radio::*;

use crate::{metrics, settings, Credentials, DownlinkSender, Result};

use log::{debug, error, info, warn};
use lorawan_device::async_device::{
    radio::{PhyRxTx, Timer, RxConfig},
    Device, JoinResponse, NetworkCredentials, SendResponse, Timings,
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

pub struct VirtualDevice {
    label: String,
    metrics_sender: metrics::Sender,
    rejoin_frames: u32,
    secs_between_transmits: u64,
    secs_between_join_transmits: u64,
    credentials: NetworkCredentials,
    device: Device<VirtualRadio, DefaultFactory, VirtualTimer, rand_core::OsRng, 512, 4>,
    state: State,
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
        let mut device = Device::new(
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
                state: State::NotJoined,
            },
        ))
    }

    pub async fn run(mut self) -> Result {
        // stagger the starts slightly
        let random = rand::random::<u64>() % 1000;
        sleep(Duration::from_millis(random)).await;

        loop {
            match self.state {
                State::NotJoined => self.do_join().await?,
                State::Joined => self.do_send().await?,
            }
        }
    }

    async fn do_join(&mut self) -> Result {
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
                self.state = State::Joined;
                info!("{} joined successfully", self.label);
            }
            JoinResponse::NoJoinAccept => {
                sleep(Duration::from_secs(self.secs_between_join_transmits)).await;
                error!("{} failed to join", self.label);
            }
        }
        Ok(())
    }

    async fn do_send(&mut self) -> Result {
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
            SendResponse::DownlinkReceived(fcnt_down) => {
                while let Some(_downlink) = self.device.take_downlink() {}
            }
            SendResponse::RxComplete => {}
        }
        sleep(Duration::from_secs(self.secs_between_transmits)).await;
        Ok(())
    }
}
