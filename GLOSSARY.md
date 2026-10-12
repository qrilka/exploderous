# Exploderous

Exploderous is a Bomberman-inspired game about navigating grid arenas, placing bombs, and surviving their blasts.

## Language

**Normal**:
The single-player mode built around completing objective-based levels.
_Avoid_: Campaign mode

**Battle**:
The competitive mode with configurable arenas and human or computer-controlled participants.

**Versus**:
The competitive preset for exactly two human participants in a fixed arena. Local or network play describes how participants connect, not a different gameplay mode.

**Character**:
A mobile being in an arena, including a human-controlled bomber, an AI-controlled competitor, or an enemy. Its identity is independent of its position and who controls it.
_Avoid_: Actor

**NPC**:
A computer-controlled character, including both AI-controlled competitors and enemies.

**Arena**:
The grid-shaped playing area containing terrain, characters, bombs, and other gameplay objects.
_Avoid_: Map, board

**Scenario**:
An authored starting arena, its initial objects and positions, and the rules of play. It is a starting definition, not ongoing state or elapsed game time.

**Game**:
One simulation instance of a scenario, with its own evolving arena, objects, and game time.

**Cell**:
One square area of an arena's terrain grid. A character can occupy parts of several cells at once.
_Avoid_: Tile

**Footprint**:
The area occupied by a character's body for movement collision and blast contact.
_Avoid_: Collision body

**Level**:
An objective-based challenge in Normal mode, comprising an arena and its completion conditions.
_Avoid_: Stage

**Wall**:
Impassable terrain that cannot be destroyed by a blast.
_Avoid_: Hard block, indestructible block

**Block**:
Impassable terrain that can be destroyed by a blast.
_Avoid_: Soft block, boulder

**Bomb**:
An explosive placed in an arena that produces a blast when detonated.

**Bomb capacity**:
The maximum number of undetonated bombs a character may have in the arena at once.

**Blast**:
The harmful area produced by a bomb's detonation, including its origin and its reach along the four cardinal directions.
_Avoid_: Flame, explosion radius

**Chain reaction**:
A sequence of detonations in which one bomb's blast triggers another bomb.

**Item**:
A collectible object in an arena that grants its specified power-up when consumed. It can be concealed beneath a block.
_Avoid_: Pickup

**Exit**:
The destination a player must reach to complete a Normal level once its completion conditions are satisfied.
