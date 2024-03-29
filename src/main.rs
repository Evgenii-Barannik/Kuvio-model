use std::borrow::BorrowMut;
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
use std::cmp::min;
use rand::distributions::Uniform; 
use rayon::prelude::*;
// use rayon::ThreadPoolBuilder;
use std::iter::zip;

// type AgentID = usize;

#[derive(PartialEq, PartialOrd, Eq, Ord, Debug, Hash, Clone, EnumIter)]
enum Resource {
    Coins,
}

type Resources = BTreeMap<Resource, usize>;
type Action = fn(&mut Resources);
type ActionVecModification = fn(&mut Vec<Action>);
type GameProvider = fn() -> Game;   
type GameAssigner = fn(&Game, Vec<Agent>) -> BTreeMap<AnyRole, &Agent>;
type Decider = fn(Vec<Action>, DecisionMakingData, &mut StdRng) -> Action;
type DecisionMakingData = Vec<f64>;

#[derive(Clone)]
struct Agent {
    resources: Resources,
    actions: Vec<Action>,
    decider: Decider,
}

impl Agent {
    fn new(initial_resources: Resources, actions: Vec<Action>, decider: Decider) -> Agent {
        let mut zeroed_resources = Resource::iter().map(|r| (r, 0)).collect::<Resources>();
        for (resource, amount) in initial_resources {
            zeroed_resources.insert(resource, amount);
        }

        Agent {resources: zeroed_resources, actions, decider}
    }

}

struct Game  {
    mods: BTreeMap<AnyRole, ActionVecModification>,
}

impl Game {
    fn transform_actions(&self, assigned_roles: &BTreeMap<AnyRole, usize>, agents: Vec<Agent>) -> Vec<Action> {
        let mut all_modified_actions: Vec<Action> = Vec::new(); 

        for (assigned_role, agent_id) in assigned_roles.iter() {
            if let  Some(action_modifier) = self.mods.get(assigned_role) {
                let mut cloned_actions = agents[*agent_id].actions.clone();
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
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone)]
enum AnyRole {
    KingdomRole(KingdomRole),
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone)]
enum KingdomRole {
    King,
}

fn pass(res: &mut Resources) {}

const TRIVIAL_ACTIONS: [Action; 1] = [pass];

fn trivial_modification(actions: &mut Vec<Action>) {
}

fn mint(res: &mut Resources) {
    *res.entry(Resource::Coins).or_insert(0) += 1;
}

fn add_mint(actions: &mut Vec<Action>) {
    actions.push(mint)
}

fn weighted_rng_decider(actions: Vec<Action>, data: DecisionMakingData, rng: &mut StdRng) -> Action {
    let weighted_distribution = WeightedIndex::new(&data).unwrap();
    let chosen_index = weighted_distribution.sample(rng);
    actions[chosen_index].clone()
}

fn mint_game_provider() -> Game {
    let mut mods = BTreeMap::new();
    mods.insert(AnyRole::KingdomRole(KingdomRole::King), add_mint as ActionVecModification);
    Game {mods: mods}
}

fn trivial_assigner(game: &Game, agents: Vec<Agent>) -> BTreeMap<AnyRole, usize> {
    let mut assigned_agents: BTreeMap<AnyRole, usize> = BTreeMap::new();
    
    for (role, index) in game.mods.keys().zip(0..agents.len()) {
        assigned_agents.insert(role.to_owned(), index);
    }
    
    assigned_agents
}

fn generate_normalized_vector(rng: &mut impl Rng, n: usize) -> Vec<f64> {
    let vec: Vec<f64> = rng.sample_iter(Uniform::new(0.0, 1.0)).take(n).collect();
    let sum: f64 = vec.iter().sum();
    vec.into_iter().map(|x| x / sum).collect()
}

fn log_resources (agents: &Vec<Agent>, log: &mut String) {
    log.push_str("IDs and final resources:\n");
    for (id, agent) in agents.iter().enumerate() {
        log.push_str(&format!("{:2}  {:?}\n", id, &agent.resources));
    }
    log.push_str("\n");
}

fn main() {
    let timer = Instant::now();
    fs::create_dir_all("output").unwrap();

    let num_of_agents: usize = 10;
    let num_of_ticks: usize = 1000;
    let kingdom_subselection_factor: usize = 1;
    let seed: usize = 2;

    let mut reputations = vec![vec![1f64; num_of_agents]; num_of_agents];
    let mut rng = StdRng::seed_from_u64(seed as u64);
    let mut tile = Tile::new(vec![], BTreeMap::new(), reputations);

    for i in 0..num_of_agents {
        let agent = Agent::new(
            BTreeMap::new(),
            TRIVIAL_ACTIONS.to_vec(),
            weighted_rng_decider,
        );
        tile.agents.push(agent);
    }

    
    for tick in 0..num_of_ticks {
        let mut games: Vec<Game> = vec![];
        if (tick % kingdom_subselection_factor) == 0 {
            games.push(mint_game_provider());
        }
        
        for game in games {
            let assigned_agents = trivial_assigner(&game, tile.agents.clone());
            let new_actions = game.transform_actions(&assigned_agents, tile.agents.clone());
            let normalized_vector = generate_normalized_vector(&mut rng, new_actions.len());
            let choosen_action = weighted_rng_decider(new_actions, normalized_vector, &mut rng); 

            let agent_index = *assigned_agents.first_key_value().unwrap().1; // Dereference to get usize directly
            choosen_action(&mut tile.agents[agent_index].resources); // Use usize to index
       }

       let mut summary_log = String::new();
       log_resources(&tile.agents, &mut summary_log);
       let log_file_pathname = format!("output/{}.txt", "Test");
       write(&log_file_pathname, summary_log).unwrap();
    }

    println!("Execution time: {:.3} s", timer.elapsed().as_secs_f64());
}
