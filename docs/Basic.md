# Basic entities

## Agent

An agent is defined by two separate components: a list of **resources** and a list of possible **behaviours**, each with a corresponding probability of occurring.

## Resource

Let's define a resource as a label that corresponds to an unsigned integer value (allowing us to refer to resources as "scalar resources"). Any resource represents the "wealth" of an **agent** in some respect. To help with a suitable intuition, we will use the name "gold" for the first defined resource. Generally, there could be many resources.

We define the growth of a resource as desired and its loss as undesired.

To evaluate desiredness, we use utility function; that is, logarithm of resource. This is mainly manifested through utility change (derivative value as 1/x), which would probably be useless in simplest model.

## Behaviour

Behaviour is a decision making algorithm of an **agent**, essentially probability distribution of agent making certain decisions in generating and playing **games**. Behaviours can be simple but may also contain arbitrarily complex logic, probably even including pointers to other behaviours (tho control must flow along directed acyclic graph in the end). Inputs to decision-making may include internal agent behaviour variables and resources, as well as external parameters like reputations. Behaviour should be hidden from all parts of the system except for the agent handling mechanism, but information about it is revealed through the **games** unfolding.

## Tile

We define a tile as a community of **agents**, possibly with some rules, a list of **reputation** values between agents, and treasuries to store communal resources. A tile is finite and thus could be subsampled by selection of actors.

## Reservoir

A reservoir is a large set of **agents** external to a given **tile**. The reservoir could be effectively infinite and be described through (generally correlated) probability distributions of possible agent-related parameters.

## Game

A game event is defined by the number and parameters of **agents**, their courses of actions, and outcomes. A game event changes **resource** distribution, **reputation**, and exposes information about the chosen course of action, thus revealing information about agents' behaviours.

### Basic two-actor game

Two **actors** approach the game. Both have symmetric options to collaborate or cheat, thus 4 outcomes are possible. Resource change in a **game** is model parameter that should be adjusted to experimental conditions.

### Basic one-actor game

One **actor** participates and has options to collaborate or cheat, **resource** change is a model parameter

### Declinable two-actor game

Two actors participate. Bothe have symmetric options to collaborate, cheat, or decline. If either declines, no resource change happens. Thus 5 outcomes are possible. Otherwise it is similar to basic 2-actor game.

## Time

In a modeling run, agents perform behaviours with a certain frequency (which is relevant whether we consider a single tile or reservoir). Running time or the number of events should be adjusted experimentally until the model sufficiently converges (changes in parameters produce more significant results than changes in starting game seeds).

# Modeling flow

A number of simulations should be performed using deterministic pseudorandom rules with a recorded seed for reproducibility. A slight perturbation in parameters should be introduced to ensure model stability. The final resources and reputations of tile members versus the non-tile member baseline change should be measured. Note that in most modeled systems, some level of stationary state is observed; thus, we should aim for system parameters where the reservoir does not change and discard or seriously doubt at least other parameter space sections.

# First experiments

1. Determine the timescales required for model stabilization.
2. Determine where tile-membership decision-making allows for final results to significantly diverge.
3. Observe whether tile behaviour difference from the reservoir or tile-membership considering decision-making has more influence.
4. Determine required difference in behaviour to make tile more successful than reservoir with basic 2-game, 1-game, and declinable 2-game
5. Determine where 2-game and diclinable 2-game diverge

Later, we would introduce more complex decision-making, tile memory in addition to the reputation system, tile memberships, tile memory modification rules, and much more. But this is the basic stuff that we should start with.
