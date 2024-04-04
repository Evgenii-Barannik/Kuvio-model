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

pub mod engine;
pub mod io;
use engine::*;
use io::*;

#[derive(PartialEq, PartialOrd, Eq, Ord, Debug, Hash, Clone, EnumIter)]
pub enum AnyResource {
    Coins,
}

fn pass_action(_agent: &mut Agent) {}

fn mint_action(agent: &mut Agent) {
    *agent.resources.entry(AnyResource::Coins).or_insert(0) += 10;
}

fn work_action(agent: &mut Agent) {
    *agent.resources.entry(AnyResource::Coins).or_insert(0) += 1;
}

struct WeightedRngDecider;
impl ActionDecider for WeightedRngDecider {
    fn decide(&self, actions: Vec<Action>, data: DecisionMakingData, rng: &mut StdRng) -> Action {
        let weighted_distribution = WeightedIndex::new(&data).unwrap();
        let chosen_index = weighted_distribution.sample(rng);
        actions[chosen_index].clone()
    }
}

struct TrivialTransformer;
impl ActionTransformer for TrivialTransformer  {
    fn transform(&self, _actions: &mut Vec<Action>) {}
}

struct AddMintTransformer;
impl ActionTransformer for AddMintTransformer {
    fn transform(&self, actions: &mut Vec<Action>) {
        actions.push(mint_action)
    }
}
struct AddWorkTransformer;
impl ActionTransformer for AddWorkTransformer {
    fn transform(&self, actions: &mut Vec<Action>) {
        actions.push(work_action)
    }
}

pub enum AnyTransformer {
    AddMintTransformer,
    AddWorkTransformer,
    _TrivialTransformer,
}

impl AnyTransformer {
    fn transform(&self, actions: &mut Vec<Action>) {
        match self {
            AnyTransformer::AddMintTransformer => AddMintTransformer.transform(actions),
            AnyTransformer::AddWorkTransformer => AddWorkTransformer.transform(actions),
            AnyTransformer::_TrivialTransformer => TrivialTransformer.transform(actions),
        }
    }
}

struct KingdomGameProvider;
impl GameProvider for KingdomGameProvider {
    fn provide_game(&self) -> Game {
        let mut role_transformers = BTreeMap::new();
        role_transformers.insert(AnyRole::KingdomRole(KingdomRole::King), AnyTransformer::AddMintTransformer);
        role_transformers.insert(AnyRole::KingdomRole(KingdomRole::Peasant1), AnyTransformer::AddWorkTransformer);
        role_transformers.insert(AnyRole::KingdomRole(KingdomRole::Peasant2), AnyTransformer::AddWorkTransformer);

        // Runtime check if all role variants are included:
        self.check_if_roles_are_filled(&role_transformers);
        Game { role_transformers }
    }

    fn check_if_roles_are_filled(&self, role_transformers: &BTreeMap<AnyRole, AnyTransformer>) -> () {
        for role in KingdomRole::iter() { 
            if !role_transformers.contains_key(&AnyRole::KingdomRole(role.clone())) {
                panic!("No transformer for role: {:?}", &role);
            }
        } 
    }
}


struct FirstPossibleIndicesAssigner;
impl AgentAssigner for FirstPossibleIndicesAssigner {
    fn assign_agents(&self, game: &Game, available_agents: &mut Vec<Agent>) -> Option<BTreeMap<AnyRole, AgentID>> {
        // println!("Availiable agents before assignment: {:?}", &available_agents.iter().map(|agent| agent._id).collect::<Vec<usize>>());
        let mut assigned_agents: BTreeMap<AnyRole, AgentID> = BTreeMap::new();
        let mut agent_ids_to_remove: Vec<AgentID> = vec![];
        
        for (role, agent) in game.role_transformers.keys().zip(&*available_agents) {
            assigned_agents.insert(role.to_owned(), agent._id);
            agent_ids_to_remove.push(agent._id);
        }
        
        if assigned_agents.len() != game.role_transformers.len() {
            return None; 
        } else {
            available_agents.retain(|agent| !agent_ids_to_remove.contains(&agent._id));
            // println!("Availiable agents after assignment:  {:?}", &available_agents.iter().map(|agent| agent._id).collect::<Vec<usize>>());
            Some(assigned_agents)
        }
    }
}

impl Game {
    fn prepare(&self, assigned_roles: &BTreeMap<AnyRole, AgentID>, ordered_agents: &Vec<Agent>) -> BTreeMap<AgentID, Vec<Action>> {
        let mut transformed_actions_for_actors: BTreeMap<AgentID, Vec<Action>> = BTreeMap::new(); 
        
        for (assigned_role, agent_id) in assigned_roles.iter() {
            if let Some(action_transformer) = self.role_transformers.get(assigned_role) {
                let mut cloned_actions = ordered_agents[*agent_id].actions.clone();
                action_transformer.transform(&mut cloned_actions);
                transformed_actions_for_actors.insert(*agent_id, cloned_actions);
            } 
        }
        transformed_actions_for_actors
    }

    fn prepare_and_execute(&self, assigned_roles: &BTreeMap<AnyRole, AgentID>, ordered_agents: &mut Vec<Agent>, rng: &mut StdRng) -> () {        
        let new_actions = self.prepare(&assigned_roles, &*ordered_agents); // Agents used in non-mutable represenation

        for (agent_id, actions) in new_actions {
            let decider_data = generate_normalized_vector(rng, actions.len());
            let choosen_action = WeightedRngDecider.decide(actions, decider_data, rng); 
            choosen_action(&mut ordered_agents[agent_id]); 
        } 
    }

}

///
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum AnyRole {
    KingdomRole(KingdomRole),
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, EnumIter, Debug)]
pub enum KingdomRole {
    King,
    Peasant1,
    Peasant2,
}

fn generate_normalized_vector(rng: &mut StdRng, n: usize) -> Vec<f64> {
    let vec: Vec<f64> = rng.sample_iter(Uniform::new(0.0, 1.0)).take(n).collect();
    let sum: f64 = vec.iter().sum();
    vec.into_iter().map(|x| x / sum).collect()
}

struct TrivialInitializer;
impl AgentInitializer for TrivialInitializer {
    fn initialize_agents(&self, params: Params) -> Vec<Agent> {
        let mut agents = vec![];
        for i in 0..params.num_of_agents {
            agents.push(
                Agent::new(
                BTreeMap::new(),
                [pass_action as Action; 1].to_vec(), // Do nothing action
                i as AgentID,
                )
            )
        }
        agents
    }
}



struct TrivialPoolProvider;
impl GamePoolProvider for TrivialPoolProvider {
    fn provide_pool(&self, providers: Vec<impl GameProvider>, tick: usize) -> Vec<Game> {
        let mut game_pool: Vec<Game> = vec![];
        for provider in providers {
            game_pool.push(provider.provide_game());
        }
        game_pool
    }
}

fn main() {
    let timer = Instant::now();
    fs::create_dir_all("output").unwrap();

    let num_of_ticks: usize = 1000;
    let seed: usize = 2;

    let params = Params {
        num_of_agents: 6
    };

    let reputations = vec![vec![1f64; params.num_of_agents]; params.num_of_agents];
    let mut rng = StdRng::seed_from_u64(seed as u64);
    let mut tile = Tile::new(vec![], BTreeMap::new(), reputations);
    
    tile.agents.append(&mut TrivialInitializer.initialize_agents(params));

    for tick in 0..num_of_ticks {
        let mut agents_in_temporal_order = tile.agents.clone();

        let mut game_pool = TrivialPoolProvider.provide_pool(
            vec![KingdomGameProvider, KingdomGameProvider],
            tick);
            
        game_pool.shuffle(&mut rng);

        for game in game_pool{
            let maybe_assigned_agents = FirstPossibleIndicesAssigner.assign_agents(&game, &mut agents_in_temporal_order);
            if let Some(assigned_agents) = maybe_assigned_agents {
                game.prepare_and_execute(&assigned_agents, &mut tile.agents, &mut rng)
            }
       }

       let mut summary_log = String::new();
       log_resources(&tile.agents, &mut summary_log);
       let log_file_pathname = format!("output/{}.txt", "Test");
       write(&log_file_pathname, summary_log).unwrap();
    }

    println!("Execution time: {:.3} s", timer.elapsed().as_secs_f64());
}
