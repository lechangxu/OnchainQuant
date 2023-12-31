use gstd::{
    debug, errors::Result as GstdResult, exec, msg, prelude::*, ActorId, MessageId, Reservation,
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
    pub reservations: HashMap<ActorId, Reservation>,
    pub token_info: HashMap<String, TokenInfo>,
    // account => (Token => Deposit)
    pub user_invest: HashMap<ActorId, HashMap<String, TokenDeposit>>,
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
static ALERT_REMAIN_GAS: u64 = 5_000;
static ALERT_REMAIN_BLOCKS: u32 = 2 * 24 * 60 * 60 / 2;

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
        let prices = price::get_price();
        for (user, token_deposit) in self.user_invest.iter_mut() {
            let who = if user == &exec::program_id() {
                "********** contract ********* ".to_string()
            } else {
                hex::encode(user.as_ref())
            };
            let weight_sum: u32 = token_deposit
                .iter()
                .filter(|(k, _v)| {
                    k.as_str() != USDT_NAME && (k.as_str() == BTC_NAME || k.as_str() == DOT_NAME)
                })
                .map(|(_k, v)| v.weight)
                .sum();
            let usdt = token_deposit.entry_ref(USDT_NAME).or_default();
            let budget = usdt.amount * self.r_invest_ration as u128 / RATION_MULTIPLES;
            usdt.amount -= budget;
            for (k, token) in token_deposit
                .iter_mut()
                .filter(|(k, _v)| k.as_str() != USDT_NAME)
            {
                let price = prices.get(k).unwrap();
                // budget * (weight / weight_sum) / price * btc_multiples
                let budget = budget * token.weight as u128 / weight_sum as u128;
                let info = self.token_info.get(k).unwrap();
                let buy = budget * info.multiples as u128 / *price as u128;
                token.amount += buy;
                debug!("{} Spend {} USDT, buy {} {}", who, budget, buy, k);
                let _ = msg::send(
                    *user,
                    TradeMsg {
                        time: exec::block_timestamp(),
                        from_token: USDT_NAME.to_string(),
                        from_amount: budget,
                        to_token: k.to_string(),
                        to_amount: buy,
                    }
                    .encode(),
                    0,
                );
            }
            let mut total_asset = 0u128;
            for (k, token) in token_deposit {
                debug!("{} {} {}", who, k, token.amount);
                if k == USDT_NAME {
                    total_asset += token.amount;
                } else {
                    let info = self.token_info.get(k).unwrap();
                    let price = prices.get(k).unwrap();
                    total_asset += *price as u128 * token.amount / info.multiples as u128;
                }
            }
            debug!("{} total asset {}", who, total_asset);
        }
    }

    fn check_reserve(&self, user: &ActorId) {
        if let Some(res) = self.reservations.get(user) {
            let amount = res.amount();
            let block_height = exec::block_height();
            let valid_until = res.valid_until();

            debug!("amount {amount}, block_height {block_height} valid_until {valid_until}");

            if amount <= ALERT_REMAIN_GAS
                || block_height >= valid_until
                || (valid_until - block_height) <= ALERT_REMAIN_BLOCKS
            {
                let remain_block = if block_height < valid_until {
                    valid_until - block_height
                } else {
                    0
                };
                let mail = format!(
                    "reamin {amount} gas, remain {0} blocks, please update gas reservation",
                    remain_block
                );

                debug!("send to mailbox:{}", mail);
                match msg::send(
                    *user,
                    GasAlertMsg {
                        remain_gas: amount,
                        remain_block,
                        msg: mail,
                    }
                    .encode(),
                    0,
                ) {
                    Ok(_) => debug!("success send to mailbox"),
                    Err(e) => debug!("send to mailbox failed {e}"),
                }
            }
        } else {
            let _ = msg::send(
                *user,
                GasAlertMsg {
                    msg: "no gas reservation".to_string(),
                    remain_gas: 0,
                    remain_block: 0,
                }
                .encode(),
                0,
            );
        }
    }

    fn action(&mut self) {
        let block = exec::block_height();
        if self.block_next != block {
            debug!("scheduled in {0} instead of {block}", self.block_next);
            return;
        }
        debug!("run action {} in block {}", self.action_id, block);
        self.quant();
        self.check_reserve(&self.owner);
        let reservation = self
            .reservations
            .get(&self.owner)
            .expect("can't find reservation");
        let _msg_id = msg::send_delayed_from_reservation(
            reservation.id(),
            exec::program_id(),
            OcqAction::Act,
            0,
            self.block_step,
        )
        .expect("msg_send");
        self.action_id += 1;
        self.block_next = block + self.block_step;
    }

    fn reserve(&mut self, amount: u64, blocks: u32) -> OcqEvent {
        let reservation = match Reservation::reserve(amount, blocks) {
            Ok(res) => res,
            Err(e) => {
                debug!("reservation failed: {e}");
                return OcqEvent::GasReserve { amount: 0, time: 0 };
            }
        };

        if let Some(resv) = self.reservations.insert(msg::source(), reservation) {
            if let Ok(gas) = resv.unreserve() {
                debug!("release {gas} gas");
            }
        }
        debug!("reserve {amount} gas for {blocks} blocks");
        OcqEvent::GasReserve {
            amount,
            time: blocks,
        }
    }

    fn allocation_ration(&mut self, tokens: Vec<(String, u32)>) {
        let user_tokens = self.user_invest.entry(msg::source()).or_default();
        for (token, weight) in tokens {
            user_tokens.entry(token).or_default().weight = weight;
        }
    }

    fn invest(&mut self, token: String, amount: u128) {
        self.user_invest
            .entry(msg::source())
            .or_default()
            .entry(token)
            .or_default()
            .amount += amount;
    }

    fn asset_of(&self) -> Vec<(String, u128)> {
        self.user_invest
            .get(&msg::source())
            .map(|m| m.iter().map(|(k, v)| (k.to_string(), v.amount)).collect())
            .unwrap_or_default()
    }
}

#[no_mangle]
extern "C" fn handle() {
    let action: OcqAction = msg::load().expect("can not decode a handle action!");
    let quant: &mut OnchainQuant = unsafe { ONCHAIN_QUANT.get_or_insert(Default::default()) };
    let rply = match action {
        OcqAction::Start => {
            quant.start();
            OcqEvent::Success
        }
        OcqAction::Stop => {
            quant.stop();
            OcqEvent::Success
        }
        OcqAction::Act => {
            quant.action();
            OcqEvent::Success
        }
        OcqAction::GasReserve { amount, blocks } => quant.reserve(amount, blocks),
        OcqAction::GasReserveDefault => quant.reserve(RESERVATION_AMOUNT, RESERVATION_TIME),
        OcqAction::Terminate => {
            exec::exit(quant.owner);
        }
        OcqAction::AssetAllocationRatio(tokens) => {
            if !tokens.is_empty() {
                quant.allocation_ration(tokens);
            }
            OcqEvent::Success
        }
        OcqAction::Invest { token, amount } => {
            // should query ft contract for the amount of tokens
            quant.invest(token, amount);
            OcqEvent::Success
        }
        OcqAction::WithDraw {
            token: _,
            amount: _,
        } => todo!(),
        OcqAction::Asset => OcqEvent::Asset(quant.asset_of()),
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
    let user_invest = dummy_user_invest(&token_deposit);
    let quant = OnchainQuant {
        r_invest_ration: config.r_invest_ration,
        reservations: HashMap::new(),
        block_step: config.block_step,
        block_next: 0,
        action_id: 0,
        owner: msg::source(),
        token_info,
        user_invest,
    };
    unsafe { ONCHAIN_QUANT = Some(quant) };
    price::init();
}

fn actor_id_from_str(other: &str) -> ActorId {
    let id = other.strip_prefix("0x").unwrap_or(other);

    let mut bytes = [0u8; 32];

    if hex::decode_to_slice(id, &mut bytes).is_err() {
        panic!("Invalid identifier: {:?}", other)
    }

    ActorId::from(bytes)
}

fn dummy_user_invest(
    prototype: &HashMap<String, TokenDeposit>,
) -> HashMap<ActorId, HashMap<String, TokenDeposit>> {
    let actors = [
        "0x54d13cda7abe4aab7adbe1b7d2da8662934f33c628d7737d2be33e95075fc77e",
        "0x4ccf35ad0f22d5a83a6a0608bcbbce9992974293ac492858a2370a93af9ebd45",
        "0x8472f7754a62850263727957b7acf7d077961a9e94816fce2780c72d5a2a5717",
        "0x327caff531d22348427ca6d7a051cecc6d621a72c8d9db3be8dd544fa78a263c",
    ];
    let mut dest = HashMap::new();
    for actor in actors {
        let actor = actor_id_from_str(actor);
        dest.insert(actor, prototype.clone());
    }
    dest.insert(exec::program_id(), prototype.clone());
    dest
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
