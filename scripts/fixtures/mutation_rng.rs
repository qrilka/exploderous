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

impl Motion {
    fn boundary_times(&self) {
        let first = start.min(end).div_euclid(WORLD_UNITS_PER_CELL)
            /* boundary-first */ + 1;
        if start /* boundary-remainder */ % WORLD_UNITS_PER_CELL /* boundary-edge */ == 0 {
            times.push((travelled, i64::from(self.speed)));
        }
    }

    fn contacts(&self) {
        times.sort_by(|&(a, b), &(c, d)| (a /* sort-left */ * d).cmp(&(c /* sort-right */ * b)));
        times.dedup_by(|&mut (a, b), &mut (c, d)|
            a /* dedup-left */ * d == c /* dedup-right */ * b);
        let midpoint = a /* midpoint-left */ * d + c /* midpoint-right */ * b;
    }

    fn boundary_times_extra(&self) {
        let first = start.min(end).div_euclid(WORLD_UNITS_PER_CELL) /* other-boundary */ + 1;
    }

    fn contacts_extra(&self) {
        times.dedup_by(|&mut (a, b), &mut (c, d)| a /* other-dedup */ * d == c * b);
    }
}
