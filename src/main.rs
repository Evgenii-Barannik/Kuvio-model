use std::fs;
use std::time::Instant;
use std::vec;
use std::collections::{BTreeMap, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::fs::write;
use itertools::Itertools;
// use std::iter::zip;
use std::iter::IntoIterator;
use rand::distributions::{Distribution, WeightedIndex};
use rand::{Rng, rngs::StdRng, SeedableRng};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use plotters::{coord::Shift, prelude::*};
use toml::map::Map;
use toml::Value;
use std::path::PathBuf;
use walkdir::WalkDir;
use ordered_float::OrderedFloat; // Wrapper over f64 to support hashing
use rayon::prelude::*;
use std::cmp::min;
// use rayon::ThreadPoolBuilder;

type AgentID = usize;

#[derive(PartialEq, PartialOrd, Eq, Ord, Debug, Hash, Clone, EnumIter)]
enum Resource {
    Coins,
}

type Resources = BTreeMap<Resource, usize>;
type Action = fn(&mut Resources);
type ActionVecModification = fn(&mut Vec<Action>);
type Behaviour = fn(Vec<Action>, DecisionMakingData, &mut StdRng) -> Action;
type DecisionMakingData = Vec<f64>;

struct Agent {
    resources: Resources,
    actions: Vec<Action>,
    behaviour: Behaviour,
}

impl Agent {
    fn new(initial_resources: Resources, actions: Vec<Action>, behaviour: Behaviour) -> Agent {
        let mut zeroed_resources = Resource::iter().map(|r| (r, 0)).collect::<Resources>();
        for (resource, amount) in initial_resources {
            zeroed_resources.insert(resource, amount);
        }

        Agent {resources: zeroed_resources, actions, behaviour}
    }

}

struct Game  {
    mods: BTreeMap<AnyRole, ActionVecModification>,
}
type GameProvider = fn() -> Game;   

impl Game {
    fn transform_actions(&self, assigned_roles: BTreeMap<AnyRole, &Agent>) -> Vec<Action> {
        let mut all_modified_actions: Vec<Action> = Vec::new(); 

        for (assigned_role, agent) in assigned_roles.iter() {
            if let  Some(action_modifier) = self.mods.get(assigned_role) {
                let mut cloned_actions = agent.actions.clone();
                action_modifier(&mut cloned_actions);
                return cloned_actions
            } 
            // TODO Make work for multiple agents alltogether (with slices?)
        }
        panic!("Modied actions not created.")
    }
}

type ReputationMatrix = Vec<Vec<f64>>;

struct Tile {
    agents: Vec<Agent>,
    resources: Resources,
    reputations: ReputationMatrix,
}

impl Tile {
    fn new(agents: Vec<Agent>, resources: Resources, reputations: Vec<Vec<f64>>) -> Tile {
        let mut zeroed_resources = Resource::iter().map(|r| (r, 0)).collect::<Resources>();
        for (resource, amount) in resources {
            zeroed_resources.insert(resource, amount);
        }

        Tile{agents, resources: zeroed_resources, reputations}
    }
}

///
#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum AnyRole {
    KingdomRole(KingdomRole),
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum KingdomRole {
    King,
}

fn pass(res: &mut Resources) {}

const TRIVIAL_ACTIONS: [Action; 1] = [pass];

fn trivial_modification(actions: &mut Vec<Action>) -> Vec<Action> {
    actions.clone()
}

fn mint(res: &mut Resources) {
    *res.entry(Resource::Coins).or_insert(0) += 1;
}

fn add_mint(actions: &mut Vec<Action>) {
    actions.push(mint)
}

// const WEIGHTED_RNG_BEHAVIOUR: Behaviour = |actions: Vec<&Action>, data: DecisionMakingData, rng: &mut StdRng| -> Action {
//     let weighted_distribution = WeightedIndex::new(&data).unwrap();
//     let chosen_index = weighted_distribution.sample(rng);
//     actions[chosen_index].clone()
// };

// type Behaviour = fn(Vec<Action>, DecisionMakingData, &mut StdRng) -> Action;

fn weighted_rng_behaviour(actions: Vec<Action>, data: DecisionMakingData, rng: &mut StdRng) -> Action {
    let weighted_distribution = WeightedIndex::new(&data).unwrap();
    let chosen_index = weighted_distribution.sample(rng);
    actions[chosen_index].clone()
}

fn mint_game_provider() -> Game {
    let mut mods = BTreeMap::new();
    mods.insert(AnyRole::KingdomRole(KingdomRole::King), add_mint as ActionVecModification);
    Game {mods: mods}
}


// fn generate_normalized_vector(rng: &mut impl Rng, n: usize) -> Vec<f64> {
//     let vec: Vec<f64> = rng.sample_iter(Uniform::new(0.0, 1.0)).take(n).collect();
//     let sum: f64 = vec.iter().sum();
//     vec.into_iter().map(|x| x / sum).collect()
// }

fn main() {
    let timer = Instant::now();
    fs::create_dir_all("output").unwrap();

    let num_of_agents: usize = 10;
    let num_of_ticks: usize = 100;
    let kingdom_subselection_factor: usize = 1;
    let seed: usize = 2;

    let mut reputations = vec![vec![1f64; num_of_agents]; num_of_agents];
    let mut rng = StdRng::seed_from_u64(seed as u64);
    let mut tile = Tile::new(vec![], BTreeMap::new(), reputations);

    for i in 0..num_of_agents {
        let agent = Agent::new(BTreeMap::new(), TRIVIAL_ACTIONS.to_vec(), weighted_rng_behaviour);
        tile.agents.push(agent);
    }

    // let normalized_vector = generate_normalized_vector(&mut rng, 2);

    for tick in 0..num_of_ticks {
        let mut games: Vec<Game> = vec![];
        if (tick % kingdom_subselection_factor) == 0 {
            // games.push(MINT_GAME_PROVIDER());
       }
       
       for game in games {
            // game.
       }
    }

    println!("Execution time: {:.3} s", timer.elapsed().as_secs_f64());
}
