mod data;

pub use data::*;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstructionTag {
    CreatePool         = 0x00,
    ToggleMarketState  = 0x01,
    UpdateConfig       = 0x02,
    CollectFees        = 0x03,
    InitMakerBook      = 0x06,
    UpdateMakerBook    = 0x07,
    UpdateMidPrice     = 0x08,
    ClearBook          = 0x09,
    SetBookDelegate    = 0x0a,
    MakerDepositFunds  = 0x0b,
    MakerWithdrawFunds = 0x0c,
    AsyncSwapCreate    = 0x0d,
    AsyncSwapExecute   = 0x0e,
    AsyncSwapCancel    = 0x0f,
    SyncSwap           = 0x10,
    MakerHeartbeat     = 0x11,
    MakerSetSequence   = 0x12,
    SetBookHalt        = 0x13,
    UpdateSyncSpread   = 0x1a,
    UpdateExpiryInSlots = 0x1e,
    CloseMakerBook     = 0x1f,
}

impl InstructionTag {
    #[inline]
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x00 => Some(Self::CreatePool),
            0x01 => Some(Self::ToggleMarketState),
            0x02 => Some(Self::UpdateConfig),
            0x03 => Some(Self::CollectFees),
            0x06 => Some(Self::InitMakerBook),
            0x07 => Some(Self::UpdateMakerBook),
            0x08 => Some(Self::UpdateMidPrice),
            0x09 => Some(Self::ClearBook),
            0x0a => Some(Self::SetBookDelegate),
            0x0b => Some(Self::MakerDepositFunds),
            0x0c => Some(Self::MakerWithdrawFunds),
            0x0d => Some(Self::AsyncSwapCreate),
            0x0e => Some(Self::AsyncSwapExecute),
            0x0f => Some(Self::AsyncSwapCancel),
            0x10 => Some(Self::SyncSwap),
            0x11 => Some(Self::MakerHeartbeat),
            0x12 => Some(Self::MakerSetSequence),
            0x13 => Some(Self::SetBookHalt),
            0x1a => Some(Self::UpdateSyncSpread),
            0x1e => Some(Self::UpdateExpiryInSlots),
            0x1f => Some(Self::CloseMakerBook),
            _    => None,
        }
    }
}
