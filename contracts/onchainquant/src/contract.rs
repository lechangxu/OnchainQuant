use gstd::{
    debug, errors::Result as GstdResult, exec, msg, prelude::*, ActorId, MessageId, ReservationId,
};

use onchainquant_io::*;

use crate::price;

#[derive(Debug, Clone, Default)]
pub struct TokenInfo {
    pub name: String,
    // generated from decimals, if token decimal is 6, the multiples is 1_000_000
    pub multiples: u64,
    // pub program_id: ActorId,
}

#[derive(Debug, Clone, Default)]
pub struct TokenDeposit {
    // weight for Asset Allocation ratio,
    pub weight: u32,
    pub amount: u128,
}

#[derive(Debug, Clone, Default)]
pub struct OnchainQuant {
    // Regular Investment Ratio, in 0.000001
    pub r_invest_ration: u64,
    pub reservation_ids: HashMap<ActorId, ReservationId>,
    pub token_info: HashMap<String, TokenInfo>,
    pub token_deposit: HashMap<String, TokenDeposit>,
    pub block_step: u32,
    pub block_next: u32,
    pub action_id: u64,
    pub owner: ActorId,
}
const RATION_MULTIPLES: u128 = 1_000_000;
static mut ONCHAIN_QUANT: Option<OnchainQuant> = None;

static RESERVATION_AMOUNT: u64 = 50_000_000;
// 30 days
static RESERVATION_TIME: u32 = 30 * 24 * 60 * 60 / 2;

pub(crate) const BTC_NAME: &str = "ocqBTC";
pub(crate) const DOT_NAME: &str = "ocqDOT";
pub(crate) const USDT_NAME: &str = "ocqUSDT";

impl OnchainQuant {
    fn start(&mut self) {
        let source = msg::source();
        if self.owner != source {
            debug!("only owner can start, {:?} is not owner", source);
            return;
        }
        let block = exec::block_height();
        if self.block_next >= block {
            debug!(
                "already start, schedule in {}, should stop before start",
                self.block_next
            );
        }
        // not start, this will triger a start
        self.block_next = exec::block_height();
        self.action();
    }

    fn stop(&mut self) {
        let source = msg::source();
        if self.owner != source {
            debug!("only owner can stop, {:?} is not owner", source);
            return;
        }
        self.block_next = 0;
    }

    fn quant(&mut self) {
        let weight_sum: u32 = self
            .token_deposit
            .iter()
            .filter(|(k, _v)| k.as_str() != USDT_NAME)
            .map(|(_k, v)| v.weight)
            .sum();
        let prices = price::get_price();
        let usdt = self.token_deposit.entry_ref(USDT_NAME).or_default();
        let budget = usdt.amount * self.r_invest_ration as u128 / RATION_MULTIPLES;
        usdt.amount -= budget;

        for (k, token) in self
            .token_deposit
            .iter_mut()
            .filter(|(k, _v)| k.as_str() != USDT_NAME)
        {
            let price = prices.get(k).unwrap();
            // budget * (weight / weight_sum) / price * btc_multiples
            let budget = budget * token.weight as u128 / weight_sum as u128;
            let info = self.token_info.get(k).unwrap();
            let buy = budget * info.multiples as u128 / *price as u128;
            token.amount += buy;
            debug!("Spend {} USDT, buy {} {}", budget, buy, k);
        }
        let mut total_asset = 0u128;
        for (k, token) in &self.token_deposit {
            debug!("{} {}", k, token.amount);
            if k == USDT_NAME {
                total_asset += token.amount;
            } else {
                let info = self.token_info.get(k).unwrap();
                let price = prices.get(k).unwrap();
                total_asset += *price as u128 * token.amount / info.multiples as u128;
            }
        }
        debug!("total asset {}", total_asset);
    }

    fn action(&mut self) {
        let block = exec::block_height();
        if self.block_next != block {
            debug!("scheduled in {0} instead of {block}", self.block_next);
            return;
        }
        debug!("run action {} in block {}", self.action_id, block);
        self.quant();
        let reservation_id = self
            .reservation_ids
            .get(&self.owner)
            .expect("can't find reservation");
        let _msg_id = msg::send_delayed_from_reservation(
            *reservation_id,
            exec::program_id(),
            OcqAction::Act,
            0,
            self.block_step,
        )
        .expect("msg_send");
        self.action_id += 1;
        self.block_next = block + self.block_step;
    }

    fn reserve(&mut self) -> OcqEvent {
        let reservation_id = ReservationId::reserve(RESERVATION_AMOUNT, RESERVATION_TIME)
            .expect("reservation across executions");
        self.reservation_ids.insert(msg::source(), reservation_id);
        debug!("reserve {RESERVATION_AMOUNT} gas for {RESERVATION_TIME} blocks");
        OcqEvent::GasReserve {
            amount: RESERVATION_AMOUNT,
            time: RESERVATION_TIME,
        }
    }
}

#[no_mangle]
extern "C" fn handle() {
    let action: OcqAction = msg::load().expect("can not decode a handle action!");
    let quant: &mut OnchainQuant = unsafe { ONCHAIN_QUANT.get_or_insert(Default::default()) };
    let rply = match action {
        OcqAction::Start => {
            quant.start();
            OcqEvent::Start
        }
        OcqAction::Stop => {
            quant.stop();
            OcqEvent::Stop
        }
        OcqAction::Act => {
            quant.action();
            OcqEvent::Act
        }
        OcqAction::GasReserve => quant.reserve(),
        OcqAction::Terminate => {
            exec::exit(quant.owner);
        }
    };
    msg::reply(rply, 0).expect("error in sending reply");
}

#[no_mangle]
extern "C" fn init() {
    let config: InitConfig = msg::load().expect("Unable to decode InitConfig");

    let mut token_info = HashMap::new();
    token_info.insert(
        BTC_NAME.to_string(),
        TokenInfo {
            name: BTC_NAME.to_string(),
            multiples: 1_0000_0000,
        },
    );
    token_info.insert(
        DOT_NAME.to_string(),
        TokenInfo {
            name: DOT_NAME.to_string(),
            multiples: 10_000_000_000u64,
        },
    );
    token_info.insert(
        USDT_NAME.to_string(),
        TokenInfo {
            name: USDT_NAME.to_string(),
            multiples: 1_000_000,
        },
    );
    let mut token_deposit = HashMap::new();
    token_deposit.insert(
        BTC_NAME.to_string(),
        TokenDeposit {
            weight: 300,
            amount: 0,
        },
    );
    token_deposit.insert(
        DOT_NAME.to_string(),
        TokenDeposit {
            weight: 200,
            amount: 0,
        },
    );
    token_deposit.insert(
        USDT_NAME.to_string(),
        TokenDeposit {
            weight: 500,
            amount: 100_000 * 1_000_000u128,
        },
    );
    let quant = OnchainQuant {
        r_invest_ration: config.r_invest_ration,
        reservation_ids: HashMap::new(),
        block_step: config.block_step,
        block_next: 0,
        action_id: 0,
        owner: msg::source(),
        token_info,
        token_deposit,
    };
    unsafe { ONCHAIN_QUANT = Some(quant) };
    price::init();
}

#[no_mangle]
extern "C" fn state() {
    reply(common_state())
        .expect("Failed to encode or reply with `<AppMetadata as Metadata>::State` from `state()`");
}

fn reply(payload: impl Encode) -> GstdResult<MessageId> {
    msg::reply(payload, 0)
}

fn common_state() -> IOOnchainQuant {
    let state = static_mut_state();
    let r_invest_ration = state.r_invest_ration;
    IOOnchainQuant {
        r_invest_ration,
        block_step: state.block_step,
        block_next: state.block_next,
        action_id: state.action_id,
    }
}

fn static_mut_state() -> &'static mut OnchainQuant {
    unsafe { ONCHAIN_QUANT.get_or_insert(Default::default()) }
}
