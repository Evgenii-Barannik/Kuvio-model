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
use implementation::{AnyAction, AnyParticipationChecker, AnyDecider, AnyResource, AnyRole, AnyTransformer};
use implementation::{get_initializer, get_pool_provider, get_agent_assigner};

type AgentID = usize;
type Resources = BTreeMap<AnyResource, usize>;
type DecisionAvailableData = BTreeMap<AgentID, Resources>;
type Action = fn(&mut Agent);
type ReputationMatrix = Vec<Vec<f64>>;

#[derive(Clone, Debug)]
pub struct Agent {
    resources: Resources,
    base_actions: Vec<AnyAction>,
    participation_checker: AnyParticipationChecker,
    decider: AnyDecider,
    id: AgentID,
}

#[derive(PartialEq, Clone)]
pub enum AnyUniqueness {
    RequiredMultipletRole(usize, usize), // Contains min required and max possible multiplicity. Should be assigned for game to play
    OptionalMultipletRole(usize, usize), // Contains min required and max possible multiplicity. Can be assigned
}

#[derive(Clone)]
pub struct RoleDescription {
    uniqueness: AnyUniqueness,
    transformer: AnyTransformer,
}

#[derive(Clone)]
pub struct Game {
    roles: BTreeMap<AnyRole, RoleDescription>,
    associated_game: Option<Box<Game>>,
}


impl Agent {
    fn new(initial_resources: Resources, base_actions: Vec<AnyAction>, decider: AnyDecider, participation_checker: AnyParticipationChecker, id: AgentID) -> Agent {
        let mut zeroed_resources = AnyResource::iter().map(|r| (r, 0)).collect::<Resources>();
        for (resource, amount) in initial_resources {
            zeroed_resources.insert(resource, amount);
        }
        Agent {resources: zeroed_resources, base_actions, decider, participation_checker, id}
    }

    fn get_utility(&self) -> f64 {
        let mut total_utility = 0.0;
        for (_resource, &amount) in &self.resources {
            if amount > 0 {
                total_utility += f64::ln(amount as f64) + 1.0;
                    // We add constant to the {log of resource amount} because without it resource change from 0 to 1 will not change utility.
                    // This is so because ln(1.0) == 0.0.
            }
        }
        total_utility
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


pub trait ActionDecider {
    fn decide(&self, agent: &Agent, transient_actions: Vec<AnyAction>, data: &DecisionAvailableData, rng: &mut StdRng) -> AnyAction;
}

pub trait ActionTransformer {
    fn transform(&self, base_actions: &mut Vec<AnyAction>) -> (); //Procedure
}

pub trait GameProvider {
    fn provide_game(&self) -> Game;
    fn check_if_all_roles_are_described(&self, roles: &BTreeMap<AnyRole, RoleDescription>) -> (); // Can panic
}

pub trait PoolProvider {
    fn provide_all_games(&self, gamepool: &mut Vec<Game>, tick: usize) -> (); // Procedure
}
pub trait AgentAssigner {
    fn assign_and_consume_agents(&self, game: &Game, available_agents: &mut Vec<Agent>) -> Option<BTreeMap<AgentID, AnyRole>>; 
}

pub trait AgentInitializer {
    fn initialize_agents(&self, configs: &Configs) -> Vec<Agent>;
}

pub trait ParticipationChecker {
    fn check_if_agent_participates(&self, agent: &Agent, game: &Game, _proposed_role: &AnyRole) -> bool;
}

pub trait AnyActionIntoInner {
    fn into_inner(self) -> Action;
}    


impl Game {
    fn prepare_actions(&self, assigned_roles: &BTreeMap<AgentID, AnyRole>, ordered_agents: &Vec<Agent>) -> BTreeMap<AgentID, Vec<AnyAction>> {
        let mut transformed_actions: BTreeMap<AgentID, Vec<AnyAction>> = BTreeMap::new(); 
        
        for (id, role) in assigned_roles.iter() {
            let role_description = self.roles.get(role).unwrap();
            let mut cloned_actions = ordered_agents[*id].base_actions.clone();
            role_description.transformer.transform(&mut cloned_actions);
            transformed_actions.insert(*id, cloned_actions);
        }
        transformed_actions
    }

    fn prepare_and_execute_actions(&self, assigned_roles: &BTreeMap<AgentID, AnyRole>, ordered_agents: &mut Vec<Agent>, rng: &mut StdRng) -> () {
        let transient_actions = self.prepare_actions(&assigned_roles, &*ordered_agents); // Agents in non-mutable represenation
        for (agent_id, actions) in transient_actions {
            let choosen_decider = &ordered_agents[agent_id].decider;

            let availiable_data: BTreeMap<AgentID, Resources> = ordered_agents
                .iter()
                .map(|agent| (agent.id, agent.resources.clone()))
                .collect();

            let choosen_action = choosen_decider.decide(&ordered_agents[agent_id], actions, &availiable_data, rng).into_inner(); 

            choosen_action(&mut ordered_agents[agent_id]) 
        } 
    }
    pub fn create_deep_associated_game(depth: usize, final_game: Game) -> Game {
        if depth == 0 {
            return final_game.clone();
        } else {
            let roles: BTreeMap<AnyRole, RoleDescription> = BTreeMap::new();
            let nested_game = Game::create_deep_associated_game(depth - 1, final_game);
            let game = Game {roles, associated_game: Some(Box::new(nested_game))};
            game
        }
    }

}


fn main() {
    let timer = Instant::now();
    fs::create_dir_all("output").unwrap();

    let configs = read_configs();
    let pool_provider = get_pool_provider();
    let agent_assigner = get_agent_assigner();
    let reputations = vec![vec![1f64; configs.agent_count]; configs.agent_count];
    
    let log_file_pathname = format!("output/{}.txt", "resources");
    let plot_file_pathname = format!("output/{}.gif", "resources");
    let mut root = BitMapBackend::gif(plot_file_pathname, (640, 480), 100).unwrap().into_drawing_area();
    
    let mut rng = StdRng::seed_from_u64(configs.seed as u64);
    let mut tile = Tile::new(get_initializer().initialize_agents(&configs), BTreeMap::new(), reputations);
    let mut gamepool: Vec<Game> = vec![];
    
    for tick in 0..configs.tick_count {
        let mut associated_games: Vec<Game> = vec![];
        pool_provider.provide_all_games(&mut gamepool, tick);
        gamepool.shuffle(&mut rng);

        let mut transient_consumable_agents = tile.agents.clone();
        // transient_consumable_agents.shuffle(& mut rng);

        for suggested_game in &gamepool{
            let maybe_assigned_agents = agent_assigner.assign_and_consume_agents(&suggested_game, &mut transient_consumable_agents);
            if let Some(assigned_agents) = maybe_assigned_agents {
                suggested_game.prepare_and_execute_actions(&assigned_agents, &mut tile.agents, &mut rng);
                if let Some(gamebox) = &suggested_game.associated_game {
                    associated_games.push(*gamebox.clone()); // If played game had an associated game, push it. 
                }
            }
        }
        gamepool.clear();
        gamepool.append(&mut associated_games);

        if configs.plot_graph && (tick % configs.plotting_frame_subselection_factor) == 0 {
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
