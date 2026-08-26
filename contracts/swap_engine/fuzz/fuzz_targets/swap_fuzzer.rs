#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Basic fuzzer shell. In a real scenario we would deserialize `data` into 
    // a sequence of `propose_swap`, `deposit_collateral`, `confirm_receipt` and execute them.
});
