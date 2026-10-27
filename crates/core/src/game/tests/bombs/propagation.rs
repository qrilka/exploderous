use super::super::support::{action, bomber, cell, point};
use crate::game::{Game, GameRules, Scenario, Terrain};

#[test]
fn cardinal_rays_preserve_requested_reach_and_span_each_arena_axis() {
    for (width, height, position, reach, expected) in [
        (
            256,
            1,
            point(128, 128),
            u32::MAX,
            (0..256).map(|column| cell(column, 0)).collect::<Vec<_>>(),
        ),
        (
            256,
            1,
            point(65408, 128),
            u32::MAX,
            (0..256).map(|column| cell(column, 0)).collect(),
        ),
        (
            1,
            256,
            point(128, 128),
            u32::MAX,
            (0..256).map(|row| cell(0, row)).collect(),
        ),
        (
            1,
            256,
            point(128, 65408),
            u32::MAX,
            (0..256).map(|row| cell(0, row)).collect(),
        ),
        (
            7,
            3,
            point(640, 384),
            2,
            vec![
                cell(2, 1),
                cell(2, 0),
                cell(3, 1),
                cell(4, 1),
                cell(2, 2),
                cell(1, 1),
                cell(0, 1),
            ],
        ),
        (
            7,
            3,
            point(640, 384),
            u32::MAX,
            vec![
                cell(2, 1),
                cell(2, 0),
                cell(3, 1),
                cell(4, 1),
                cell(5, 1),
                cell(6, 1),
                cell(2, 2),
                cell(1, 1),
                cell(0, 1),
            ],
        ),
    ] {
        let rules = GameRules::default()
            .with_bomb_parameters(1, reach, 1)
            .unwrap();
        let initial = Scenario::new(
            width,
            height,
            vec![Terrain::Floor; (width * height) as usize],
            bomber(position),
            rules,
        )
        .unwrap();
        let mut game = Game::new(initial, crate::game::Tick::new(0));
        game.step(&[action(None, true)]).unwrap();
        game.step(&[]).unwrap();
        let coverage = game.blasts()[0].cells().collect::<Vec<_>>();
        assert_eq!(
            coverage.len(),
            expected.len(),
            "{width}x{height}, reach {reach}"
        );
        for location in expected {
            assert!(
                coverage.contains(&location),
                "{width}x{height}, reach {reach}: missing {location:?}"
            );
        }
    }
}
