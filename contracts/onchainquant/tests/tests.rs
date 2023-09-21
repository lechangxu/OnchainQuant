use gstd::Encode;
use gtest::{Program, System};
use onchainquant_io::*;

const USERS: &[u64] = &[3, 4, 5];
const RATION: u64 = 100_000; // 10%
fn init(sys: &System) {
    sys.init_logger();

    let quant = Program::current(sys);

    let res = quant.send(
        USERS[0],
        InitConfig {
            r_invest_ration: RATION,
            block_step: 2,
        },
    );

    assert!(!res.main_failed());
    let res = quant.read_state::<IOOnchainQuant>().expect("state");
    assert_eq!(res.r_invest_ration, RATION);
}
#[test]
fn start() {
    let sys = System::new();
    init(&sys);
    let quant = sys.get_program(1);
    let res = quant.send(USERS[0], OcqAction::GasReserveDefault);
    assert!(res.contains(&(
        USERS[0],
        OcqEvent::GasReserve {
            amount: 100_000,
            time: 1296000
        }
        .encode()
    )));

    let res = quant.send(USERS[0], OcqAction::Start);
    assert!(res.contains(&(USERS[0], OcqEvent::Success.encode())));
    let _ = sys.spend_blocks(2);
    let res = quant.read_state::<IOOnchainQuant>().expect("state");
    assert_eq!(res.block_next, 4);
    assert_eq!(res.action_id, 2);
}

#[test]
fn stop() {
    let sys = System::new();
    init(&sys);
    let quant = sys.get_program(1);
    let res = quant.send(USERS[0], OcqAction::GasReserveDefault);
    assert!(res.contains(&(
        USERS[0],
        OcqEvent::GasReserve {
            amount: 100_000,
            time: 1296000
        }
        .encode()
    )));
    // start
    let block_height = sys.block_height();
    let res = quant.send(USERS[0], OcqAction::Start);
    assert!(res.contains(&(USERS[0], OcqEvent::Success.encode())));
    let _ = sys.spend_blocks(2);
    let status0 = quant.read_state::<IOOnchainQuant>().expect("state");
    assert_eq!(status0.block_next, block_height + 2 * 2);
    assert_eq!(status0.action_id, 2);
    println!("res {:?}", res);
    // stop
    let res = quant.send(USERS[0], OcqAction::Stop);
    assert!(res.contains(&(USERS[0], OcqEvent::Success.encode())));
    let _ = sys.spend_blocks(11);
    let status1 = quant.read_state::<IOOnchainQuant>().expect("state");
    assert_eq!(status1.block_next, 0);
    assert_eq!(status1.action_id, status0.action_id);
    // start again
    let block_height = sys.block_height();
    let res = quant.send(USERS[0], OcqAction::Start);
    assert!(res.contains(&(USERS[0], OcqEvent::Success.encode())));
    let _ = sys.spend_blocks(2);
    let status2 = quant.read_state::<IOOnchainQuant>().expect("state");
    assert_eq!(status2.block_next, block_height + 2 * 2);
    assert_eq!(status2.action_id, status1.action_id + 2);
    println!("res {:?}", res);
    // stop
    let res = quant.send(USERS[0], OcqAction::Stop);
    assert!(res.contains(&(USERS[0], OcqEvent::Success.encode())));
    let _ = sys.spend_blocks(15);
    let status3 = quant.read_state::<IOOnchainQuant>().expect("state");
    assert_eq!(status3.block_next, 0);
    assert_eq!(status3.action_id, status2.action_id);
}

#[test]
fn reserve_again() {
    let sys = System::new();
    init(&sys);
    let quant = sys.get_program(1);
    let res = quant.send(USERS[0], OcqAction::GasReserveDefault);
    let r = OcqEvent::GasReserve {
        amount: 100_000,
        time: 1296000,
    }
    .encode();
    assert!(res.contains(&(USERS[0], r.clone())));
    let res = quant.send(USERS[0], OcqAction::GasReserveDefault);
    let _ = sys.spend_blocks(10);
    assert!(res.contains(&(USERS[0], r)));
}

#[test]
fn reserve_alert() {
    let sys = System::new();
    init(&sys);
    let quant = sys.get_program(1);
    let res = quant.send(
        USERS[0],
        OcqAction::GasReserve {
            amount: 4999,
            blocks: 1,
        },
    );
    let r = OcqEvent::GasReserve {
        amount: 4999,
        time: 1,
    }
    .encode();
    assert!(res.contains(&(USERS[0], r.clone())));
    let _res = quant.send(USERS[0], OcqAction::Start);
    let _ = sys.spend_blocks(10);
    let mailbox = sys.get_mailbox(USERS[0]);
    let _ = mailbox.contains(&(
        USERS[0],
        GasAlertMsg {
            remain_gas: 4999,
            remain_block: 0,
            msg: "reamin 4999 gas, remain 0 blocks, please update gas reservation".to_string(),
        }
        .encode(),
    ));
}
