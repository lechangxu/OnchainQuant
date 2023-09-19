use gstd::{debug, exec, HashMap, String, ToString};

use rand::{Rng, SeedableRng};

use rand_xoshiro::Xoshiro128PlusPlus;

static mut FUNGIBLE_TOKENS: Option<HashMap<String, u64>> = None;

pub(crate) fn init() {
    let mut tokens = HashMap::new();
    tokens.insert("ocqBTC".to_string(), 26500_000_000u64);
    unsafe {
        FUNGIBLE_TOKENS = Some(tokens);
    }
}

#[derive(Debug)]
pub enum GetPriceError {
    NoneExist,
}

pub fn get_price(token: &str) -> Result<u64, GetPriceError> {
    let base_price = unsafe { FUNGIBLE_TOKENS.get_or_insert(Default::default()) }
        .get(token)
        .ok_or(GetPriceError::NoneExist)?;

    let exec_program = exec::program_id();
    let block_timestamp = exec::block_timestamp();
    let seed = sp_core_hashing::blake2_128(
        &[
            exec_program.as_ref(),
            &block_timestamp.to_le_bytes(),
            token.as_bytes(),
        ]
        .concat(),
    );

    let mut generator = Xoshiro128PlusPlus::from_seed(seed);
    let range = -99..100;
    let ratio: i32 = generator.gen_range(range);
    let dest_price = base_price * (1000 + ratio) as u64 / 1000;
    debug!("get price ratio {ratio}, final price {dest_price}");
    return Ok(dest_price);
}
