mod data;

pub use data::*;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstructionTag {
    CreatePool        = 0x00,
    ToggleMarketState = 0x01,
    UpdateConfig      = 0x02,
    RegisterMaker     = 0x03,
    InitMakerBook     = 0x04,
    MakerDepositFunds = 0x05,
    UpdateMakerBook   = 0x07,
    MakerHeartbeat    = 0x08,
    MakerSetSequence  = 0x09,
    MakerWithdrawFunds = 0x0c,
    SyncSwap          = 0x0f,
}

impl InstructionTag {
    #[inline]
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x00 => Some(Self::CreatePool),
            0x01 => Some(Self::ToggleMarketState),
            0x02 => Some(Self::UpdateConfig),
            0x03 => Some(Self::RegisterMaker),
            0x04 => Some(Self::InitMakerBook),
            0x05 => Some(Self::MakerDepositFunds),
            0x07 => Some(Self::UpdateMakerBook),
            0x08 => Some(Self::MakerHeartbeat),
            0x09 => Some(Self::MakerSetSequence),
            0x0c => Some(Self::MakerWithdrawFunds),
            0x0f => Some(Self::SyncSwap),
            _    => None,
        }
    }
}
