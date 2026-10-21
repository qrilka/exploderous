/* Rust parser fixture: expressions are deliberately split across lines. */
fn random_choice(state: &mut u64, count: u64) -> usize {
    if count == 1 { return 0; }
    let threshold = count
        .wrapping_neg()
        /* threshold */ % count;
    loop {
        *state = state.wrapping_add(0x9e3779b97f4a7c15);
        let mut value = *state;
        value = (value /* xor30 */ ^ (
            value /* shift30 */ >> 30
        )).wrapping_mul(0xbf58476d1ce4e5b9);
        value = (value /* xor27 */ ^ (
            value /* shift27 */ >> 27
        )).wrapping_mul(0x94d049bb133111eb);
        /* Non-ASCII before a span checks character versus byte columns: é. */ value ^= value /* shift31 */ >> 31;
        if value /* acceptance */ >= threshold {
            return (value /* return-mod */ % count) as usize;
        }
    }
}

fn other_random_choice(count: u64) -> u64 {
    count.wrapping_neg() /* other-fn */ % count
}

fn random_choice_extra(count: u64) -> u64 {
    count.wrapping_neg() /* similar-fn */ % count
}
