# Basic entities

## Agent

An agent is defined by two separate components: a list of **resources** and a list of possible **behaviours**, each with a corresponding probability of occurring.

## Resource

Let's define a resource as a label that corresponds to an unsigned integer value (allowing us to refer to resources as "scalar resources"). Any resource represents the "wealth" of an **agent** in some respect. To help with a suitable intuition, we will use the name "gold" for the first defined resource. Generally, there could be many resources.

We define the growth of a resource as desired and its loss as undesired.

To evaluate desiredness, we use utility function; that is, logarithm of resource. This is mainly manifested through utility change (derivative value as 1/x), which would probably be useless in simplest model.

## Behaviour

Behaviour is a specific action that can transform the state of a **tile**, but in the most cases it is focused on the state of the acting **agent**. In our simplest model, each behaviour is associated with a corresponding probability of being chosen at each **tick** of a **game**. Behaviours can be simple but may also contain arbitrarily complex logic, including pointers to other behaviours (tho control must flow along directed acyclic graph in the end). Inputs to decision-making may include internal agent behaviour variables and resources, as well as external parameters like reputations. Behaviour should be hidden from all parts of the system except for the agent handling mechanism, but information about it is revealed through the **game** unfolding. In a slightly more complex system, we might consider using several behaviour+probability lists for acting on different tiles.

## Tile

We define a tile as a community of **agents** with a list of **reputation** values between agents (allowing us to refer to reputations as "relational resources"). A tile is finite and thus can only be inhabited by a limited number of agents.

## Reservoir

A reservoir is a large set of **agents** external to a given **tile**. The reservoir could be effectively infinite and be described through (generally correlated) probability distributions of possible agent-related parameters.

## Game

A game event is defined by the number and parameters of **agents**, their courses of actions, and outcomes. A game event changes **resource** distribution, **reputation**, and exposes information about the chosen course of action, thus revealing information about agents' behaviours.

## Time

In a modeling run, agents perform behaviours with a certain frequency (which is relevant whether we consider a single tile or reservoir). Running time or the number of events should be adjusted experimentally until the model sufficiently converges (changes in parameters produce more significant results than changes in starting game seeds).

# Modeling flow

A number of simulations should be performed using deterministic pseudorandom rules with a recorded seed for reproducibility. A slight perturbation in parameters should be introduced to ensure model stability. The final resources and reputations of tile members versus the non-tile member baseline change should be measured. Note that in most modeled systems, some level of stationary state is observed; thus, we should aim for system parameters where the reservoir does not change and discard or seriously doubt at least other parameter space sections.

# First experiments

1. Determine the timescales required for model stabilization.
2. Determine where tile-membership decision-making allows for final results to significantly diverge.
3. Observe whether tile behaviour difference from the reservoir or tile-membership considering decision-making has more influence.

Later, we would introduce more complex decision-making, tile memory in addition to the reputation system, tile memberships, tile memory modification rules, and much more. But this is the basic stuff that we should start with.
