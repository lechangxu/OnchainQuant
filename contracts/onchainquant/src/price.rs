use gstd::exec;

use rand::{Rng, SeedableRng};

use rand_xoshiro::Xoshiro128PlusPlus;

pub fn get_price(token: &str) -> u64 {
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
    let ratio = generator.gen_range(0..100);

    return ratio;
}
