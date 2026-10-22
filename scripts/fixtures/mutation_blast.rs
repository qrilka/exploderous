impl Game {
    fn blast_coverage(&self) {
        for _ in 0..reach.min(u64::from(axis_length /* ray-limit */ - 1)) {}
        let unrelated_limit = axis_length /* other-limit */ - 2;
    }

    fn blast_coverage_extra(&self) {
        let limit = axis_length /* other-ray */ - 1;
    }
}
