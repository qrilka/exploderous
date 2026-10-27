use super::support::{point, scenario};
use crate::game::{
    CellCoord, CharacterCommand, Column, Direction, Game, GameRules, Position, Row, Terrain,
};

fn classic_arena(position: Position, rules: GameRules) -> Game {
    // Thirteen playable Cells per axis; only odd/odd Cells are pillars.
    let layout = ".............\n\
                  .#.#.#.#.#.#.\n\
                  .............\n\
                  .#.#.#.#.#.#.\n\
                  .............\n\
                  .#.#.#.#.#.#.\n\
                  .............\n\
                  .#.#.#.#.#.#.\n\
                  .............\n\
                  .#.#.#.#.#.#.\n\
                  .............\n\
                  .#.#.#.#.#.#.\n\
                  .............";
    Game::new(scenario(layout, position, rules), crate::game::Tick::new(0))
}

fn step(game: &mut Game, movement: Option<Direction>, place_bomb: bool) {
    game.step(&[CharacterCommand {
        character: game.character().id,
        movement,
        place_bomb,
    }])
    .unwrap();
}

#[test]
fn a_two_unit_footprint_stops_at_exact_wall_contact_when_moving_up() {
    for assisted in [false, true] {
        let rules = GameRules::new(2, 128)
            .unwrap()
            .with_turn_assistance(assisted);
        let mut game = Game::new(
            scenario("#\n.\n.", point(128, 384), rules),
            crate::game::Tick::new(0),
        );
        step(&mut game, Some(Direction::Up), false);
        // The Wall ends at y=256; the Footprint extends one unit above its centre.
        assert_eq!(
            game.character().position,
            point(128, 257),
            "assisted={assisted}"
        );
    }
}

#[test]
fn a_thirteen_cell_interior_keeps_complete_footprints_and_blasts_inside_every_corner() {
    use Direction::{Down, Left, Right, Up};

    // Both movement modes, every corner: bounds must work without terrain Walls.
    for assisted in [false, true] {
        for (start, outward, inward, contact, corner) in [
            (
                point(128, 128),
                [Left, Up],
                [Right, Down],
                point(96, 96),
                (0, 0),
            ),
            (
                point(3200, 128),
                [Right, Up],
                [Left, Down],
                point(3232, 96),
                (12, 0),
            ),
            (
                point(128, 3200),
                [Left, Down],
                [Right, Up],
                point(96, 3232),
                (0, 12),
            ),
            (
                point(3200, 3200),
                [Right, Down],
                [Left, Up],
                point(3232, 3232),
                (12, 12),
            ),
        ] {
            let rules = GameRules::new(192, 512)
                .unwrap()
                .with_turn_assistance(assisted)
                .with_bomb_parameters(4, u32::MAX, 30)
                .unwrap();
            let mut game = classic_arena(start, rules);
            assert_eq!((game.arena().width(), game.arena().height()), (13, 13));
            assert_eq!(game.arena().terrain().len(), 169);
            for direction in outward {
                step(&mut game, Some(direction), false);
            }
            assert_eq!(game.character().position, contact);
            let origin = CellCoord::new(Column::new(corner.0), Row::new(corner.1));
            assert_eq!(game.arena().cell_containing(contact), Some(origin));
            assert_eq!(game.arena().terrain_at(origin), Some(Terrain::Floor));
            assert_eq!(
                game.arena()
                    .terrain_at(CellCoord::new(Column::new(13), Row::new(0))),
                None
            );
            assert_eq!(game.arena().cell_containing(point(-1, 128)), None);
            assert_eq!(game.arena().cell_containing(point(128, 3328)), None);

            // Place at off-centre exterior contact and retreat inward.
            step(&mut game, None, true);
            assert_eq!(game.character().position, contact);
            assert_eq!(game.bomb().unwrap().cell(), origin);
            // Escape two Cells along the edge, then two Cells inward. Pillars must
            // leave the even-coordinate corridors navigable; neither Blast arm hits us.
            for direction in inward {
                step(&mut game, Some(direction), false);
            }
            game.step(&[]).unwrap();
            game.step(&[]).unwrap();
            assert_eq!(game.outcome(), None);
            assert!(game.bomb().is_none());
            let blast = &game.blasts()[0];
            assert_eq!(blast.origin(), origin);
            assert_eq!(blast.cells().len(), 25); // two full thirteen-Cell edges, one shared origin
            for (cell, _) in game.arena().cells() {
                let on_ray = cell.column().value() == corner.0 || cell.row().value() == corner.1;
                assert_eq!(blast.cells().contains(&cell), on_ray, "cell={cell:?}");
            }
        }
    }
}
