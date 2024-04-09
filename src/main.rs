use std::borrow::BorrowMut;
use std::fs;
use std::time::Instant;
use std::vec;
use std::collections::{BTreeMap, VecDeque};
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
use implementation::{AnyAction, AnyDecider, AnyResource, AnyRole, AnyTransformer};
use implementation::{get_initializer, get_pool_provider, get_agent_assigner, get_game_providers};

type AgentID = usize;
type Resources = BTreeMap<AnyResource, usize>;
type AvailableData = BTreeMap<AgentID, Resources>;
type Action = fn(&mut Agent);
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
    fn decide(&self, actions: Vec<AnyAction>, data: &AvailableData, rng: &mut StdRng) -> AnyAction;
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

pub trait Initializer { // Deciders are stored separately from agents to limit information available for agents 
    fn initialize_agents(&self, configs: &Configs) -> (Vec<Agent>, BTreeMap<AgentID, AnyDecider>);
}

pub trait AnyActionIntoInner {
    fn into_inner(self) -> Action;
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

    fn prepare_and_execute(&self, assigned_roles: &BTreeMap<AnyRole, AgentID>, all_agents: &mut Vec<Agent>, rng: &mut StdRng, deciders: &BTreeMap<AgentID, AnyDecider>) -> () {        
        let new_actions = self.prepare(&assigned_roles, &*all_agents); // Agents in non-mutable represenation

        for (agent_id, actions) in new_actions {
            let choosen_decider = deciders.get(&agent_id).unwrap();

            let availiable_data: BTreeMap<AgentID, Resources> = all_agents
                .iter()
                .map(|agent| (agent.id, agent.resources.clone()))
                .collect();

            let choosen_action = choosen_decider.decide(actions, &availiable_data, rng).into_inner(); 

            choosen_action(&mut all_agents[agent_id]) 
        } 
    }

}

fn main() {
    let timer = Instant::now();
    fs::create_dir_all("output").unwrap();

    let configs = read_configs();
    let pool_provider = get_pool_provider();
    let game_providers = get_game_providers();
    let assigner = get_agent_assigner();

    let reputations = vec![vec![1f64; configs.agent_count]; configs.agent_count];
    let mut rng = StdRng::seed_from_u64(configs.seed as u64);
    let mut tile = Tile::new(vec![], BTreeMap::new(), reputations);
    
    let (mut agents, deciders) = get_initializer().initialize_agents(&configs);
    tile.agents.append(&mut agents);
    drop(agents);
    
    let log_file_pathname = format!("output/{}.txt", "resources");
    let plot_file_pathname = format!("output/{}.gif", "resources");
    let mut root = BitMapBackend::gif(plot_file_pathname, (640, 480), 100).unwrap().into_drawing_area();
    
    for tick in 0..configs.tick_count {
        let mut transient_consumable_agents = tile.agents.clone();
        // transient_consumable_agents.shuffle(& mut rng);
        let mut game_pool = pool_provider.provide_pool(&game_providers, tick);
        game_pool.shuffle(&mut rng);

        for game in game_pool{
            let maybe_assigned_agents = assigner.assign_and_consume_agents(&game, &mut transient_consumable_agents);
            if let Some(assigned_agents) = maybe_assigned_agents {
                game.prepare_and_execute(&assigned_agents, &mut tile.agents, &mut rng, &deciders)
            }

        }
        if (tick % configs.plotting_frame_subselection_factor) == 0 {
            println!("Plotting frame for tick {}", tick);
            plot_resource_distribution(&tile.agents, &mut root, tick);
        }
    }
       let mut summary_log = String::new();
       summary_log.push_str(&format!("{:#?}\n\n", configs));
       log_resources(&tile.agents, &mut summary_log);
       log_reputations(&tile._reputations, &mut summary_log);

       write(&log_file_pathname, summary_log).unwrap();

    println!("Execution time: {:.3} s", timer.elapsed().as_secs_f64());
}
