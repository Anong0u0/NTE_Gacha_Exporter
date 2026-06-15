use crate::model::{GuiError, PoolKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GachaRule {
    pub hard_pity: u64,
    pub pickup_win_rate_percent: u8,
    pub has_guarantee: bool,
}

pub fn classify_pool_id(pool_id: &str) -> Result<PoolKind, GuiError> {
    match pool_id {
        "CardPool_Character" => Ok(PoolKind::MonopolyLimited),
        "CardPool_NewRole" => Ok(PoolKind::MonopolyStandard),
        value if value.starts_with("ForkLottery_") => Ok(PoolKind::ForkLottery),
        value => Err(GuiError::UnknownPoolId(value.to_string())),
    }
}

pub fn rule_for(kind: PoolKind) -> GachaRule {
    match kind {
        PoolKind::MonopolyLimited | PoolKind::MonopolyStandard => GachaRule {
            hard_pity: 90,
            pickup_win_rate_percent: 100,
            has_guarantee: false,
        },
        PoolKind::ForkLottery => GachaRule {
            hard_pity: 80,
            pickup_win_rate_percent: 25,
            has_guarantee: true,
        },
    }
}
