# Basic entities

## Actor

Actor is defined by **resource** and **behaviour**

## Resource

Let's define resource as linear unsigned integer value that represents "wealth" of entity. In general, there could be many resources and **actor** and **tile** resources could vary, but this is basic model.

We define growth of resource as desired behavior and loss as undesired.

To evaluate desiredness, we use utility function; that is, logarithm of resource. This is mainly manifested through utility change (derivative value as 1/x), which would probably be useless in simplest model.

## Behaviour

Behaviour is probability of **actor** making a certain decision in certains situation. In simplest model, this is probability of **actor** being honest in a **game**. Inputs to decision making include internal actor's behaviour variables, but also external parameters. Behaviour is hidden from all parts of system except for actor handling mechanism, but information about it is revealed through **game** events. In basic variant, we should use simple probabilities of action. In slightly more convoluted system, we would use 2 probability sets for within and outside of tile participations for tile members.

## Tile

We define a tile as community of **actors**. Tile might have internal resource pool (let's omit it in simplest model). Tile is finite and thus we can generate a selection of actors.

## Reservoir

Reservoir is a large set of **actors** external to given **tile**. Reservoir could be effectively infinite and be described through (generally corellated) probability distributions of parameters.

## Game

A game event is devined by numbet of **actors**, their courses of action, and outcomes. A game event changes **resource** distribution and exposes information about chosen course of action thus revealing some evidence about behaviour parameters of actor.

### Basic two-actor game

Two **actors** approach the game. Both have symmetric options to collaborate or cheat, thus 4 outcomes are possible. Resource change in a **game** is model parameter that should be adjusted to experimental conditions.

### Basic one-actor game

One **actor** participates and has options to collaborate or cheat, **resource** change is a model parameter

### Declinable two-actor game

Two actors participate. Bothe have symmetric options to collaborate, cheat, or decline. If either declines, no resource change happens. Thus 5 outcomes are possible. Otherwise it is similar to basic 2-actor game.

## Time

In modeling run, actors chosen randomly perform games with certain frequency (which does not matter if we consider a single tile or reservoir). Running time or number of events should be adjusted experimentally until model sufficiently converges (changes in parameters are more significant than runs with different random seeds).

# Modeling flow

A number of simulations should be performed using deterministic pseudorandom with recorded seed for reproducibility analysis. Slight pertrubation in parameters should be introduced to ensure model stability. Final utility of tile members versus non-tile member baseline and utility change should be measured. Note, that in most modeled systems some level of stationary state is observed, thus we should aim at system parameters where reservoir utility does not change and discard or seriously doubt at least other parameter space sections.

# First experiments

1. Determine timescales required for model stabilization
2. Determine required difference in behaviour to make tile more successful than reservoir with basic 2-game, 1-game, and declinable 2-game
3. Determine where 2-game and diclinable 2-game diverge
4. Determine where tile-membership considering decision making diverges from uniform modeling
5. Observe whether tile behavior difference from reservoir or tile-membership considering behaviour has more influence

Later we would introduce more complex decision making, tile memory, tile memberships, tile memory modification rules, and much more, but this is basic stuff that we should start from.

