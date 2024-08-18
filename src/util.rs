use lorawan_device::nb_device::radio;
use semtech_udp::{Bandwidth, CodingRate, DataRate, SpreadingFactor, push_data};
use semtech_udp::push_data::RxPkV1;

pub(crate) fn tx_request_to_rxpk(settings: Settings, buffer: &[u8], tmst: u32) -> push_data::Packet {
    use push_data::{CRC, RxPk, Packet};
    let rxpk = RxPkV1 {
        chan: 0,
        data: Vec::from(buffer),
        size: buffer.len() as u64,
        codr: settings.get_codr(),
        datr: settings.get_datr(),
        freq: settings.get_freq(),
        lsnr: 5.5,
        modu: semtech_udp::Modulation::LORA,
        rfch: 0,
        rssi: -112,
        rssis: None,
        stat: CRC::OK,
        tmst,
        time: None,
    };
    Packet::from_rxpk([0, 0, 0, 0, 0, 0, 0, 0].into(), RxPk::V1(rxpk)).into()
}

#[derive(Debug)]
pub struct Settings {
    rf_config: radio::RfConfig,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            rf_config: radio::RfConfig {
                frequency: 903000000,

                bb: radio::BaseBandModulationParams::new(
                    radio::SpreadingFactor::_7,
                    radio::Bandwidth::_125KHz,
                    radio::CodingRate::_4_5,
                ),
            },
        }
    }
}

impl From<radio::RfConfig> for Settings {
    fn from(rf_config: radio::RfConfig) -> Settings {
        Settings { rf_config }
    }
}


impl Settings {
    pub fn get_datr(&self) -> DataRate {
        DataRate::new(
            match self.rf_config.bb.sf {
                radio::SpreadingFactor::_5 => SpreadingFactor::SF5,
                radio::SpreadingFactor::_6 => SpreadingFactor::SF6,
                radio::SpreadingFactor::_7 => SpreadingFactor::SF7,
                radio::SpreadingFactor::_8 => SpreadingFactor::SF8,
                radio::SpreadingFactor::_9 => SpreadingFactor::SF9,
                radio::SpreadingFactor::_10 => SpreadingFactor::SF10,
                radio::SpreadingFactor::_11 => SpreadingFactor::SF11,
                radio::SpreadingFactor::_12 => SpreadingFactor::SF12,
            },
            match self.rf_config.bb.bw {
                radio::Bandwidth::_125KHz => Bandwidth::BW125,
                radio::Bandwidth::_250KHz => Bandwidth::BW250,
                radio::Bandwidth::_500KHz => Bandwidth::BW500,
                _ => panic!("unexpected lorawan bandwidth"),
            },
        )
    }

    pub fn get_codr(&self) -> CodingRate {
        match self.rf_config.bb.cr {
            radio::CodingRate::_4_5 => CodingRate::_4_5,
            radio::CodingRate::_4_6 => CodingRate::_4_6,
            radio::CodingRate::_4_7 => CodingRate::_4_7,
            radio::CodingRate::_4_8 => CodingRate::_4_8,
        }
    }

    pub fn get_freq(&self) -> f64 {
        self.rf_config.frequency as f64 / 1_000_000.0
    }
}
