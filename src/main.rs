use std::borrow::BorrowMut;
use std::fs;
use std::time::Instant;
use std::vec;
use std::collections::{BTreeMap};
use std::hash::{Hash, Hasher};
use std::fs::write;
use std::vec::Drain;
use itertools::Itertools;
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
use rand::prelude::SliceRandom;
// use strum::IntoEnumIterator;

mod io;
mod implementation;

use io::*;
use implementation::{AnyResource, AnyTransformer, AnyRole, AnyAction, AnyActionIntoInner};
use implementation::{get_initializer, get_decider, get_pool_provider, get_agent_assigner, get_game_providers};

type AgentID = usize;
type Resources = BTreeMap<AnyResource, usize>;
type Action = fn(&mut Agent);
type DecisionMakingData = Vec<f64>;
type ReputationMatrix = Vec<Vec<f64>>;

#[derive(Clone, Debug)]
pub struct Agent {
    resources: Resources,
    actions: Vec<AnyAction>,
    id: AgentID,
}

pub struct Game  {
    role_transformers: BTreeMap<AnyRole, AnyTransformer>,
}

pub struct Params {
    num_of_agents: usize
}

impl Agent {
    fn new(initial_resources: Resources, actions: Vec<AnyAction>, id: AgentID) -> Agent {
        let mut zeroed_resources = AnyResource::iter().map(|r| (r, 0)).collect::<Resources>();
        for (resource, amount) in initial_resources {
            zeroed_resources.insert(resource, amount);
        }
        Agent {resources: zeroed_resources, actions, id}
    }
}

struct Tile {
    agents: Vec<Agent>,
    _resources: Resources,
    _reputations: ReputationMatrix,
}

impl Tile {
    fn new(agents: Vec<Agent>, resources: Resources, reputations: Vec<Vec<f64>>) -> Tile {
        let mut zeroed_resources = AnyResource::iter().map(|r| (r, 0)).collect::<Resources>();
        for (resource, amount) in resources {
            zeroed_resources.insert(resource, amount);
        }
        
        Tile{agents, _resources: zeroed_resources, _reputations: reputations}
    }
}


pub trait Decider {
    fn decide(&self, actions: Vec<AnyAction>, data: DecisionMakingData, rng: &mut StdRng) -> AnyAction;
}

pub trait Transformer {
    fn transform(&self, actions: &mut Vec<AnyAction>) -> ();
}

pub trait GameProvider {
    fn provide_game(&self) -> Game;
    fn check_if_roles_are_filled(&self, role_transformers: &BTreeMap<AnyRole, AnyTransformer>) -> ();
}

pub trait PoolProvider {
    fn provide_pool(&self, providers: &Vec<impl GameProvider>, tick: usize) -> Vec<Game>;
}
pub trait Assigner {
    fn assign_and_consume_agents(&self, game: &Game, available_agents: &mut Vec<Agent>) -> Option<BTreeMap<AnyRole, AgentID>>;
}

pub trait Initializer {
    fn initialize_agents(&self, params: Params) -> Vec<Agent>;
}


impl Game {
    fn prepare(&self, assigned_roles: &BTreeMap<AnyRole, AgentID>, ordered_agents: &Vec<Agent>) -> BTreeMap<AgentID, Vec<AnyAction>> {
        let mut transformed_actions_for_actors: BTreeMap<AgentID, Vec<AnyAction>> = BTreeMap::new(); 
        
        for (assigned_role, agent_id) in assigned_roles.iter() {
            if let Some(action_transformer) = self.role_transformers.get(assigned_role) {
                let mut cloned_actions = ordered_agents[*agent_id].actions.clone();
                action_transformer.transform(&mut cloned_actions);
                transformed_actions_for_actors.insert(*agent_id, cloned_actions);
            } 
        }
        transformed_actions_for_actors
    }

    fn prepare_and_execute(&self, assigned_roles: &BTreeMap<AnyRole, AgentID>, all_agents: &mut Vec<Agent>, rng: &mut StdRng, decider: &impl Decider) -> () {        
        let new_actions = self.prepare(&assigned_roles, &*all_agents); // Agents used in non-mutable represenation

        for (agent_id, actions) in new_actions {
            let decider_data = generate_normalized_vector(rng, actions.len());
            let choosen_action = decider.decide(actions, decider_data, rng).into_inner();

            choosen_action(&mut all_agents[agent_id]) 
        } 
    }

}

fn generate_normalized_vector(rng: &mut StdRng, n: usize) -> Vec<f64> {
    let vec: Vec<f64> = rng.sample_iter(Uniform::new(0.0, 1.0)).take(n).collect();
    let sum: f64 = vec.iter().sum();
    vec.into_iter().map(|x| x / sum).collect()
}

fn main() {
    let timer = Instant::now();
    fs::create_dir_all("output").unwrap();

    let num_of_ticks: usize = 1000;
    let seed: usize = 5;

    let params = Params {
        num_of_agents: 10
    };

    let decider = get_decider();
    let pool_provider = get_pool_provider();
    let game_providers = get_game_providers();
    let assigner = get_agent_assigner();

    let reputations = vec![vec![1f64; params.num_of_agents]; params.num_of_agents];
    let mut rng = StdRng::seed_from_u64(seed as u64);
    let mut tile = Tile::new(vec![], BTreeMap::new(), reputations);
    
    tile.agents.append(&mut get_initializer().initialize_agents(params));

    for tick in 0..num_of_ticks {
        let mut transient_consumable_agents = tile.agents.clone();
        // transient_consumable_agents.shuffle(& mut rng);
        let mut game_pool = pool_provider.provide_pool(&game_providers, tick);
        game_pool.shuffle(&mut rng);

        for game in game_pool{
            let maybe_assigned_agents = assigner.assign_and_consume_agents(&game, &mut transient_consumable_agents);
            if let Some(assigned_agents) = maybe_assigned_agents {
                game.prepare_and_execute(&assigned_agents, &mut tile.agents, &mut rng, &decider)
            }
       }

       let mut summary_log = String::new();
       log_resources(&tile.agents, &mut summary_log);
       let log_file_pathname = format!("output/{}.txt", "resources");
       write(&log_file_pathname, summary_log).unwrap();
    }

    println!("Execution time: {:.3} s", timer.elapsed().as_secs_f64());
}
