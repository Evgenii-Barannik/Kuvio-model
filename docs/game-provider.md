# Game provider

A game provider is the entity that adds games to runtime's game pool.

This is an abstraction to isolate origins of games.

A number of games is proposed for each tick; they may fail to be executed.

The games should be considered to be played "simultaneously" within a tick.

Game providers are specified through 2 layers

### Hardcoded logic

Hardcoded game providers MUST be stored in separate module, all well-documented and independently versioned.

### Game provider spec

Game providers are generated statically at the beginning of simulation and do not hold internal state. Spec file should be a TOML in dedicated subdiractory of the model.

## Parameters to specify

- Game types (complex logic with whole game spec stack involved)
- For each game, frequency parameters (including noise generation) - potentially could have complex logic
- Connected player pools

