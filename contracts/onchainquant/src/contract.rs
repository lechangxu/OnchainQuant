use gstd::{
    debug, errors::Result as GstdResult, exec, msg, prelude::*, ActorId, MessageId, ReservationId,
};

use onchainquant_io::*;

use crate::price;

#[derive(Debug, Clone, Default)]
pub struct OnchainQuant {
    // Regular Investment Ratio, in 0.000001
    pub r_invest_ration: u64,
    pub reservation_ids: HashMap<ActorId, ReservationId>,
    pub block_step: u32,
    pub block_next: u32,
    pub action_id: u64,
    pub owner: ActorId,
}
static mut ONCHAIN_QUANT: Option<OnchainQuant> = None;

static RESERVATION_AMOUNT: u64 = 50_000_000;
// 30 days
static RESERVATION_TIME: u32 = 30 * 24 * 60 * 60 / 2;
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

    fn action(&mut self) {
        let block = exec::block_height();
        if self.block_next != block {
            debug!("scheduled in {0} instead of {block}", self.block_next);
            return;
        }
        debug!("run action {} in block {}", self.action_id, block);

        let price = price::get_price("ocqBTC");
        debug!("get price {price}");
        let reservation_id = self
            .reservation_ids
            .get(&self.owner)
            .expect("can't find reservation");
        let _msg_id = msg::send_delayed_from_reservation(
            reservation_id.clone(),
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
    };
    msg::reply(rply, 0).expect("error in sending reply");
}

#[no_mangle]
extern "C" fn init() {
    let config: InitConfig = msg::load().expect("Unable to decode InitConfig");
    let quant = OnchainQuant {
        r_invest_ration: config.r_invest_ration,
        reservation_ids: HashMap::new(),
        block_step: config.block_step,
        block_next: 0,
        action_id: 0,
        owner: msg::source(),
    };
    unsafe { ONCHAIN_QUANT = Some(quant) };
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
