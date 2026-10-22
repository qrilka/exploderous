use super::*;

#[test]
fn cardinal_commands_apply_only_to_their_tick() {
    let base = Game::new(
        scenario(3, 3, vec![Terrain::Floor; 9], 384, 384)
            .parse(GameRules::default())
            .unwrap(),
        0,
    );
    for (direction, expected) in [
        (Direction::Up, point(384, 368)),
        (Direction::Right, point(400, 384)),
        (Direction::Down, point(384, 400)),
        (Direction::Left, point(368, 384)),
    ] {
        let mut game = base.clone();
        assert_step(&mut game, &[command(direction)], expected);
    }
    let mut game = base;
    assert_step(&mut game, &[], point(384, 384));
    assert_step(&mut game, &[command(Direction::Right)], point(400, 384));
    assert_step(&mut game, &[command(Direction::Right)], point(416, 384));
    assert_step(&mut game, &[], point(416, 384));
    assert_step(&mut game, &[command(Direction::Left)], point(400, 384));
}

#[test]
fn swept_footprints_stop_at_the_first_obstruction_without_a_gap_or_sideways_motion() {
    use Direction::*;
    let fast = GameRules::new(192, 512).unwrap();
    let cases = [
        (rows(".#..", 128, 128), fast, Right, point(160, 128)),
        (rows(".+..", 128, 128), fast, Right, point(160, 128)),
        (rows("..#.", 896, 128), fast, Left, point(864, 128)),
        (rows("..+.", 896, 128), fast, Left, point(864, 128)),
        (
            rows(
                r#"
.
#
.
.
"#,
                128,
                128,
            ),
            fast,
            Down,
            point(128, 160),
        ),
        (
            rows(
                r#"
.
+
.
.
"#,
                128,
                128,
            ),
            fast,
            Down,
            point(128, 160),
        ),
        (
            rows(
                r#"
.
.
#
.
"#,
                128,
                896,
            ),
            fast,
            Up,
            point(128, 864),
        ),
        (
            rows(
                r#"
.
.
+
.
"#,
                128,
                896,
            ),
            fast,
            Up,
            point(128, 864),
        ),
        (
            rows("..#..+..", 128, 128),
            GameRules::new(192, 1024).unwrap(),
            Right,
            point(416, 128),
        ),
        (
            rows("..#.+...", 1920, 128),
            GameRules::new(192, 1024).unwrap(),
            Left,
            point(1376, 128),
        ),
    ];
    for (authored, rules, direction, expected) in cases {
        let mut game = Game::new(authored.clone().parse(rules).unwrap(), 0);
        assert_step(&mut game, &[command(direction)], expected);
        assert_step(&mut game, &[command(direction)], expected);
        assert_step(&mut game, &[], expected);
    }
    let mut approaching = Game::new(
        rows(".#..", 128, 128).parse(GameRules::default()).unwrap(),
        0,
    );
    for x in [144, 160, 160] {
        assert_step(&mut approaching, &[command(Right)], point(x, 128));
    }
}

#[test]
fn floor_edges_are_solid_and_exact_contact_is_safe() {
    for (direction, expected) in [
        (Direction::Up, point(128, 96)),
        (Direction::Right, point(160, 128)),
        (Direction::Down, point(128, 160)),
        (Direction::Left, point(96, 128)),
    ] {
        let mut game = Game::new(
            rows(".", 128, 128)
                .parse(GameRules::new(192, 1024).unwrap())
                .unwrap(),
            0,
        );
        assert_step(&mut game, &[command(direction)], expected);
        assert_step(&mut game, &[command(direction)], expected);
    }
    for side in [2, 254] {
        let radius = (side / 2) as i32;
        let mut game = Game::new(
            scenario(
                256,
                256,
                vec![Terrain::Floor; 65536],
                65536 - radius - 1,
                radius + 1,
            )
            .parse(GameRules::new(side, 1024).unwrap())
            .unwrap(),
            0,
        );
        assert_step(
            &mut game,
            &[command(Direction::Right)],
            point(65536 - radius, radius + 1),
        );
        assert_step(
            &mut game,
            &[command(Direction::Up)],
            point(65536 - radius, radius),
        );
        assert_step(
            &mut game,
            &[command(Direction::Left)],
            point(64512 - radius, radius),
        );
        assert_step(
            &mut game,
            &[command(Direction::Down)],
            point(64512 - radius, radius + 1024),
        );
    }
}

#[test]
fn a_blocked_turn_spends_its_tick_budget_on_the_nearest_legal_entry() {
    let mut game = Game::new(
        rows("...\n.#.\n...", 192, 160)
            .parse(GameRules::default())
            .unwrap(),
        0,
    );
    assert_step(&mut game, &[command(Direction::Down)], point(176, 160));
    let mut changed_direction = game.clone();
    assert_step(
        &mut changed_direction,
        &[command(Direction::Right)],
        point(192, 160),
    );
    assert_step(
        &mut changed_direction,
        &[command(Direction::Down)],
        point(176, 160),
    );
    assert_rejected(
        &mut game,
        &[command(Direction::Down), command(Direction::Up)],
        StepError::DuplicateCommand,
    );
    assert_step(&mut game, &[], point(176, 160));
    assert_step(&mut game, &[command(Direction::Down)], point(160, 160));
    assert_step(&mut game, &[command(Direction::Down)], point(160, 176));
}

#[test]
fn a_safe_entry_can_be_away_from_the_current_cell_centreline() {
    let mut game = Game::new(
        rows("...\n#..\n...", 352, 272)
            .parse(GameRules::default())
            .unwrap(),
        0,
    );
    // The current Cell centreline is y=384 (Down), but the nearest safe
    // Left entry is y=160 (Up): 112 units of real correction.
    for y in [256, 240, 224, 208, 192, 176, 160] {
        assert_step(&mut game, &[command(Direction::Left)], point(352, y));
    }
    assert_step(&mut game, &[command(Direction::Left)], point(336, 160));
}

#[test]
fn assistance_is_symmetric_and_ignores_current_cell_offsets() {
    use Direction::*;
    // A central pillar with Floor on every side. All coordinates are worked
    // examples at Footprint 192 / speed 16, independent of collision helpers.
    for (x, y, direction, expected) in [
        (208, 160, Down, point(192, 160)),
        (209, 160, Down, point(193, 160)), // formerly rejected at offset +81
        (560, 160, Down, point(576, 160)),
        (559, 160, Down, point(575, 160)), // formerly rejected at offset -81
        (208, 608, Up, point(192, 608)),
        (560, 608, Up, point(576, 608)),
        (160, 208, Right, point(160, 192)),
        (160, 209, Right, point(160, 193)),
        (160, 560, Right, point(160, 576)),
        (160, 559, Right, point(160, 575)),
        (608, 208, Left, point(608, 192)),
        (608, 560, Left, point(608, 576)),
        (161, 160, Down, point(160, 175)), // one sideways + fifteen forward
        (160, 160, Down, point(160, 176)), // legal edge: no correction
        (159, 160, Down, point(159, 176)), // legal off-centre travel unchanged
        (128, 150, Down, point(128, 166)), // centred and clear
        (160, 384, Up, point(160, 368)),   // parallel travel unchanged
        (160, 384, Down, point(160, 400)),
        (192, 150, Down, point(176, 150)), // correction first, not partial forward
    ] {
        let mut game = Game::new(
            rows("...\n.#.\n...", x, y)
                .parse(GameRules::default())
                .unwrap(),
            0,
        );
        assert_step(&mut game, &[command(direction)], expected);
    }
}

#[test]
fn the_actual_correction_limit_is_inclusive_in_both_axes_and_directions() {
    use Direction::*;
    // Nearest legal edges are 160 and 608. Required corrections are
    // 127, 128, or 129 units; a >128 correction must retain the strict stop.
    for (coordinate, corrected) in [
        (287, 271),
        (288, 272),
        (289, 289),
        (479, 479),
        (480, 496),
        (481, 497),
    ] {
        for (direction, start, expected) in [
            (Down, point(coordinate, 160), point(corrected, 160)),
            (Up, point(coordinate, 608), point(corrected, 608)),
            (Right, point(160, coordinate), point(160, corrected)),
            (Left, point(608, coordinate), point(608, corrected)),
        ] {
            let authored = rows("...\n.#.\n...", start.x().value(), start.y().value());
            let mut assisted = Game::new(authored.clone().parse(GameRules::default()).unwrap(), 0);
            let mut strict = Game::new(
                authored
                    .parse(GameRules::default().with_turn_assistance(false))
                    .unwrap(),
                0,
            );
            assert_step(&mut assisted, &[command(direction)], expected);
            assert_step(&mut strict, &[command(direction)], start);
        }
    }
}

#[test]
fn reported_playtest_stalls_reach_safe_entries_but_real_walls_stay_blocked() {
    let layout = "#########\n#.......#\n#.#.#.#.#\n#....+..#\n#.#.#.#.#\n#.......#\n#.#.#.#.#\n#....+..#\n#########";
    for (start, direction, ticks, entry, after_forward) in [
        (
            point(864, 768),
            Direction::Left,
            6,
            point(864, 864),
            point(848, 864),
        ),
        (
            point(864, 1008),
            Direction::Left,
            5,
            point(864, 928),
            point(848, 928),
        ),
        (
            point(1008, 928),
            Direction::Down,
            5,
            point(928, 928),
            point(928, 944),
        ),
        (
            point(864, 1040),
            Direction::Left,
            7,
            point(864, 928),
            point(848, 928),
        ),
        (
            point(864, 784),
            Direction::Left,
            5,
            point(864, 864),
            point(848, 864),
        ),
        (
            point(352, 528),
            Direction::Left,
            8,
            point(352, 528),
            point(352, 528),
        ),
    ] {
        let mut game = Game::new(
            rows(layout, start.x().value(), start.y().value())
                .parse(GameRules::default())
                .unwrap(),
            0,
        );
        for _ in 0..ticks {
            game.step(&[command(direction)]).unwrap();
        }
        assert_eq!(game.character().position, entry);
        assert_eq!(game.tick(), ticks);
        assert_step(&mut game, &[command(direction)], after_forward);
    }
}

#[test]
fn assistance_cannot_cross_an_obstruction_to_reach_a_forward_opening() {
    let mut game = Game::new(
        rows(".#.\n#..\n#..", 257, 257)
            .parse(GameRules::new(2, 1).unwrap())
            .unwrap(),
        0,
    );
    // The Left opening above is only two units away, but the Up
    // correction would penetrate the Wall directly above the Character.
    assert_step(&mut game, &[command(Direction::Left)], point(257, 257));
}

#[test]
fn assistance_never_drifts_toward_a_blocked_or_unsafe_forward_path() {
    use Direction::*;
    for (layout, start, distance, direction, expected) in [
        ("...\n.+.\n...", point(192, 160), 16, Down, point(176, 160)),
        ("...\n###\n...", point(192, 150), 16, Down, point(192, 160)),
        ("...\n+++\n...", point(192, 160), 16, Down, point(192, 160)),
        (
            "...\n.#.\n#..\n...\n...",
            point(192, 160),
            1024,
            Down,
            point(192, 160),
        ),
        (
            "...\n.+.\n+..\n...\n...",
            point(192, 160),
            1024,
            Down,
            point(192, 160),
        ),
        (
            "...\n.#.\n...",
            point(192, 160),
            1024,
            Down,
            point(192, 160),
        ),
        ("...\n...\n...", point(192, 96), 16, Up, point(192, 96)),
        ("...\n...\n...", point(96, 192), 16, Left, point(96, 192)),
        ("...\n...\n...", point(672, 576), 16, Right, point(672, 576)),
        ("...\n...\n...", point(576, 672), 16, Down, point(576, 672)),
        // The nearest safe entry is 144 units away: beyond the correction limit.
        ("...\n.#.\n...", point(464, 160), 16, Down, point(464, 160)),
    ] {
        let mut game = Game::new(
            rows(layout, start.x().value(), start.y().value())
                .parse(GameRules::new(192, distance).unwrap())
                .unwrap(),
            0,
        );
        assert_step(&mut game, &[command(direction)], expected);
    }
}

#[test]
fn strict_off_centre_corridors_and_corner_contact_do_not_align_or_slide_the_character() {
    let strict = GameRules::default().with_turn_assistance(false);
    let classic = r#"
#######
#.....#
#.#.#.#
#.....#
#.#.#.#
#.....#
#######
"#;
    for x in [352, 416] {
        let mut game = Game::new(rows(classic, x, 640).parse(strict).unwrap(), 0);
        assert_step(&mut game, &[command(Direction::Up)], point(x, 624));
    }
    for x in [351, 417] {
        assert_eq!(
            rows(classic, x, 640).parse(strict),
            Err(ConstructionError::InvalidStartingPosition)
        );
    }
    let mut turn = Game::new(rows(classic, 400, 417).parse(strict).unwrap(), 0);
    assert_step(&mut turn, &[command(Direction::Right)], point(416, 417));
    assert_step(&mut turn, &[command(Direction::Right)], point(416, 417));
    assert_step(&mut turn, &[command(Direction::Up)], point(416, 401));
    assert_step(&mut turn, &[command(Direction::Right)], point(432, 401));

    let mut corner = Game::new(
        rows(
            r#"
..
.#
"#,
            160,
            160,
        )
        .parse(strict)
        .unwrap(),
        0,
    );
    assert_step(&mut corner, &[command(Direction::Right)], point(176, 160));
    let mut blocked_corner = Game::new(
        rows(
            r#"
..
.#
"#,
            160,
            161,
        )
        .parse(strict)
        .unwrap(),
        0,
    );
    assert_step(
        &mut blocked_corner,
        &[command(Direction::Right)],
        point(160, 161),
    );
    let mut spanning = Game::new(
        rows(
            r#"
..
..
"#,
            256,
            256,
        )
        .parse(strict)
        .unwrap(),
        0,
    );
    assert_step(&mut spanning, &[command(Direction::Down)], point(256, 272));
    let mut perpendicular = Game::new(
        rows(
            r#"
.#.
...
"#,
            384,
            352,
        )
        .parse(strict)
        .unwrap(),
        0,
    );
    assert_step(
        &mut perpendicular,
        &[command(Direction::Right)],
        point(400, 352),
    );
}
