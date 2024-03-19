# Simultaion runtime

## About

This document describes specifications of a main thread of a single simulation with given set of parameters.

Every simulation is done in a single-threaded mode to preserve deterministic entropy for analysis. Observational behavior included in modeling is not discussed here as the model is totally oblivious to the fact that it is being analyzed in any way (all observational operations MUST be clean for the model).

## Definitions

### Tick

**Tick** is a quantum of time in model. Multiple events may happen or nothing at all. All ticks are equal, tick patterns MAY be implemented through underlying objects.

### Game provider

Different entities may propose games. This is an abstraction to isolate origins of games.

A proposer may rely on some system parameters, but there SHOULD NOT be any proposer state that's not deterministically defined by the rest of the model.

A number of games is proposed for each tick; they may fail to be executed.

The games should be considered to be played "simultaneously" within a tick.

#### Examples

- Global provider (abstract resource source/sink; exchange market; reservoir actors representation)
- Actor (wants to play certain number of certain games)
- Tile

#### Fundamental frequency heuristic

Game proposal frequency should not match 1 game/tick exactly, nor any rational fraction of these. Tick frequency is innatural fundamental frequency in the model that should be filtered out.

This could be done by raising game frequency significantly above 1 game/tick, lowering it significantly below. Also, randomization and spread-spectrum SHOULD always be implemented.

Notable exception to this rule is modeling of low-level blockchain behaviour, where ticks could naturally be assigned to blocks.

Additionally, games should not match closely each other's frequency within phase stability time frame and on model's timeframe. Notable exception to this rule is modeling of natural cycles, like day/night or weekly cycle.

### Game pool

Games proposed by providers are collected and shuffled in single pool.

### Players exchange

Depending on every game rules, it is connected to a player exchange. Exchange could have complex rules on how to exclude players. Different roles may be connected to different pools. It is important that pools are generated before games execution, based on pre-existing information.

Pools might have drainable and non-drainable behaviour and may be shared or not between games. They should probably have deterministic hash-id which would identify these for game engine.

Some pools might have degenerate nature like 1-player initiator pool.

## Execution

The model runs following events for every **tick**:

1. Propose games from all game providers and collect these in game pool.
2. Shuffle game pool (this is important to avoid turn order texture)
3. Populate players exchanges
4. Execute games sequentially while picking players and updating participating parties parameters sequentially
5. Recalculate common values


