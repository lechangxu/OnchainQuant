use gstd::{debug, exec, HashMap, String, ToString};

use rand::{Rng, SeedableRng};

use rand_xoshiro::Xoshiro128PlusPlus;

use crate::contract::{BTC_NAME, DOT_NAME};

static mut FUNGIBLE_TOKENS: Option<HashMap<String, u64>> = None;

pub(crate) fn init() {
    let mut tokens = HashMap::new();
    tokens.insert(BTC_NAME.to_string(), 26_500u64 * 1_000_000u64);
    tokens.insert(DOT_NAME.to_string(), 4_120_000u64);
    unsafe {
        FUNGIBLE_TOKENS = Some(tokens);
    }
}

pub fn get_price() -> HashMap<String, u64> {
    let base_price = unsafe { FUNGIBLE_TOKENS.get_or_insert(Default::default()) };

    let exec_program = exec::program_id();
    let block_timestamp = exec::block_timestamp();
    let seed = sp_core_hashing::blake2_128(
        &[
            exec_program.as_ref(),
            &block_timestamp.to_le_bytes(),
            // token.as_bytes(),
        ]
        .concat(),
    );

    let mut generator = Xoshiro128PlusPlus::from_seed(seed);
    let range = -99..100;

    let mut dest = HashMap::new();
    for (k, v) in base_price.iter() {
        let ratio: i32 = generator.gen_range(range.clone());
        let dest_price = v * (1000 + ratio) as u64 / 1000;
        debug!("get {k} price ratio {ratio}, final price {dest_price}");
        dest.insert(k.to_string(), dest_price);
    }
    dest
}
