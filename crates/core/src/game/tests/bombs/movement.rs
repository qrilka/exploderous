use super::super::support::{action, cell, point};
use super::game;
use crate::game::{Direction, GameRules};

#[test]
fn placement_uses_the_post_movement_cell_without_snapping() {
    let mut game = game("...", point(248, 128), GameRules::default(), 0);
    game.step(&[action(Some(Direction::Right), true)]).unwrap();
    assert_eq!(game.character().position, point(264, 128));
    assert_eq!(game.bomb().unwrap().cell(), cell(1, 0));
    let bomb = game.bomb().unwrap().clone();
    game.step(&[action(Some(Direction::Left), true)]).unwrap();
    assert_eq!(game.character().position, point(248, 128));
    assert_eq!(game.bomb(), Some(&bomb));
}

#[test]
fn off_centre_placement_does_not_block_an_open_dead_end_retreat() {
    for assisted in [false, true] {
        let mut game = game(
            ".\n.",
            point(160, 416),
            GameRules::default().with_turn_assistance(assisted),
            0,
        );
        game.step(&[action(None, true)]).unwrap();
        assert_eq!(game.bomb().unwrap().cell(), cell(0, 1));
        assert_eq!(game.character().position, point(160, 416));
        for _ in 0..16 {
            game.step(&[action(Some(Direction::Up), false)]).unwrap();
        }
        assert_eq!(game.character().position, point(160, 160));
        assert_eq!(game.tick().value(), 17);
        // This tiny Arena proves retreat, not Blast survival: both Cells will burn.
        assert_eq!(game.outcome(), None);
    }
}

#[test]
fn overlapping_escape_can_cross_the_bomb_centre_but_cannot_reenter_after_clearance() {
    for assisted in [false, true] {
        // One step crosses the Bomb centre (384,128) and fully clears its
        // left edge at x=256. Returning must then stop at Footprint contact.
        let rules = GameRules::new(192, 256)
            .unwrap()
            .with_turn_assistance(assisted);
        let mut game = game("....", point(400, 128), rules, 0);
        game.step(&[action(None, true)]).unwrap();
        game.step(&[action(Some(Direction::Left), false)]).unwrap();
        assert_eq!(game.character().position, point(144, 128));
        game.step(&[action(Some(Direction::Right), false)]).unwrap();
        assert_eq!(game.character().position, point(160, 128));
        assert_eq!(game.bomb().unwrap().cell(), cell(1, 0));
        assert_eq!(game.bomb().unwrap().detonate_at().value(), 121);
        assert_eq!(game.tick().value(), 3); // shortened movement is still an ordinary tick
        assert_eq!(game.outcome(), None);
    }
}

#[test]
fn overlapping_escape_allows_approach_perpendicular_travel_and_reversal() {
    // Each start is 16 units beside (640,640). Approach the centre, travel
    // perpendicular to the approach, then reverse without fully clearing.
    for (start, toward, perpendicular, returning, expected) in [
        (
            point(656, 640),
            Direction::Left,
            Direction::Up,
            Direction::Down,
            point(640, 624),
        ),
        (
            point(640, 656),
            Direction::Up,
            Direction::Right,
            Direction::Left,
            point(656, 640),
        ),
        (
            point(624, 640),
            Direction::Right,
            Direction::Down,
            Direction::Up,
            point(640, 656),
        ),
        (
            point(640, 624),
            Direction::Down,
            Direction::Left,
            Direction::Right,
            point(624, 640),
        ),
    ] {
        for assisted in [false, true] {
            let mut game = game(
                ".....\n.....\n.....\n.....\n.....",
                start,
                GameRules::default().with_turn_assistance(assisted),
                0,
            );
            game.step(&[action(None, true)]).unwrap();
            let mut sibling = game.clone(); // clone while permission still applies
            for (direction, position) in [
                (toward, point(640, 640)),
                (perpendicular, expected),
                (returning, point(640, 640)),
            ] {
                let frame = [action(Some(direction), false)];
                game.step(&frame).unwrap();
                sibling.step(&frame).unwrap();
                assert_eq!(game, sibling);
                assert_eq!(game.character().position, position);
                assert_eq!(game.bomb().unwrap().cell(), cell(2, 2));
            }
        }
    }
}

#[test]
fn assistance_can_approach_during_correction_and_forward_travel() {
    let mut game = game(
        "....\n.#..\n....\n....",
        point(608, 528),
        GameRules::new(192, 256).unwrap(),
        0,
    );
    game.step(&[action(None, true)]).unwrap(); // centre (640,640)
    // Left is blocked by the Wall. Assistance goes DOWN 80 then LEFT 176
    // to (432,608), initially approaching the Bomb on both segments.
    // The body's top stays at y=512: exact contact with the Wall is legal.
    game.step(&[action(Some(Direction::Left), false)]).unwrap();
    assert_eq!(game.character().position, point(432, 608));
    assert_eq!(game.tick().value(), 2);
}

#[test]
fn assistance_can_clear_a_bomb_at_its_bend_but_cannot_reenter_it() {
    // A two-unit Footprint makes exact clearance at the bend fit in one tick.
    let mut game = game(
        "....\n.#..\n....",
        point(513, 270),
        GameRules::new(2, 64).unwrap(),
        0,
    );
    game.step(&[action(None, true)]).unwrap(); // Bomb at (2,1)
    // The Wall blocks LEFT. UP 15 clears both obstacles at y=255; LEFT 49
    // then travels along their edge, with the body's bottom exactly at y=256.
    game.step(&[action(Some(Direction::Left), false)]).unwrap();
    assert_eq!(game.character().position, point(464, 255));
    game.step(&[action(Some(Direction::Right), false)]).unwrap();
    assert_eq!(game.character().position, point(528, 255));
    // DOWN would re-enter the Bomb. Neither end of the Wall/Bomb pair can
    // be reached within the 128-unit correction limit, so assistance cannot help.
    game.step(&[action(Some(Direction::Down), false)]).unwrap();
    assert_eq!(game.character().position, point(528, 255));
    assert_eq!(game.outcome(), None);
}

#[test]
fn removal_discards_old_permissions_and_a_replacement_allows_its_own_passage() {
    let rules = GameRules::default()
        .with_turn_assistance(false)
        .with_bomb_parameters(4, 1, 1)
        .unwrap();
    let mut game = game("....\n....\n....\n....", point(520, 520), rules, 0);
    game.step(&[action(None, true)]).unwrap(); // Bomb at (2,2)
    game.step(&[action(Some(Direction::Up), false)]).unwrap();
    game.step(&[action(Some(Direction::Left), false)]).unwrap();
    assert_eq!(game.character().position, point(504, 504));
    // The Footprint still overlaps the Bomb, but the diagonal centre Cell is safe.
    game.step(&[]).unwrap();
    game.step(&[]).unwrap();
    assert!(game.bomb().is_none());
    assert_eq!(game.outcome(), None);
    game.step(&[action(Some(Direction::Down), true)]).unwrap();
    assert!(game.blasts().is_empty());
    assert_eq!(game.character().position, point(504, 520));
    assert_eq!(game.bomb().unwrap().cell(), cell(1, 2));
    game.step(&[action(Some(Direction::Down), false)]).unwrap();
    assert_eq!(game.character().position, point(504, 536)); // toward new centre: permitted
    game.step(&[action(Some(Direction::Right), false)]).unwrap();
    assert_eq!(game.character().position, point(520, 536)); // old Bomb Cell is no obstacle
    assert_eq!(game.outcome(), None);
}

#[test]
fn bomb_obstacles_stop_whole_paths_from_every_direction() {
    for (escape, return_direction, far, stopped) in [
        (
            Direction::Up,
            Direction::Down,
            point(640, 128),
            point(640, 416),
        ),
        (
            Direction::Right,
            Direction::Left,
            point(1152, 640),
            point(864, 640),
        ),
        (
            Direction::Down,
            Direction::Up,
            point(640, 1152),
            point(640, 864),
        ),
        (
            Direction::Left,
            Direction::Right,
            point(128, 640),
            point(416, 640),
        ),
    ] {
        let mut game = game(
            ".....\n.....\n.....\n.....\n.....",
            point(640, 640),
            GameRules::new(192, 512).unwrap(),
            0,
        );
        game.step(&[action(None, true)]).unwrap();
        game.step(&[action(Some(escape), false)]).unwrap();
        assert_eq!(game.character().position, far);
        game.step(&[action(Some(return_direction), false)]).unwrap();
        assert_eq!(
            game.character().position,
            stopped,
            "returning {return_direction:?}"
        );
    }
}

#[test]
fn turn_assistance_treats_an_escaped_bomb_like_a_full_cell_obstacle() {
    for assisted in [true, false] {
        let mut game = game(
            "...\n...\n...",
            point(384, 384),
            GameRules::default().with_turn_assistance(assisted),
            0,
        );
        game.step(&[action(None, true)]).unwrap();
        for _ in 0..14 {
            game.step(&[action(Some(Direction::Up), false)]).unwrap();
        }
        for _ in 0..12 {
            game.step(&[action(Some(Direction::Left), false)]).unwrap();
        }
        assert_eq!(game.character().position, point(192, 160));
        for expected in [point(176, 160), point(160, 160), point(160, 176)] {
            game.step(&[action(Some(Direction::Down), false)]).unwrap();
            assert_eq!(
                game.character().position,
                if assisted { expected } else { point(192, 160) }
            );
        }
    }
}

#[test]
fn permission_ends_only_when_the_whole_footprint_leaves_the_bomb_cell() {
    let mut game = game("....", point(384, 128), GameRules::default(), 0);
    game.step(&[action(None, true)]).unwrap();
    for _ in 0..13 {
        game.step(&[action(Some(Direction::Left), false)]).unwrap();
    }
    assert_eq!(game.character().position, point(176, 128)); // body still overlaps by 16
    game.step(&[action(Some(Direction::Right), false)]).unwrap();
    assert_eq!(game.character().position, point(192, 128)); // approach permitted while overlapping
    for _ in 0..2 {
        game.step(&[action(Some(Direction::Left), false)]).unwrap();
    }
    assert_eq!(game.character().position, point(160, 128)); // exact edge: fully clear
    game.step(&[action(Some(Direction::Right), false)]).unwrap();
    assert_eq!(game.character().position, point(160, 128)); // re-entry is blocked
    let bomb = game.bomb().unwrap().clone();
    game.step(&[action(Some(Direction::Left), true)]).unwrap(); // full capacity is ordinary gameplay
    assert_eq!(game.character().position, point(144, 128));
    assert_eq!(game.bomb(), Some(&bomb));
    assert_eq!(game.tick().value(), 19);
}
