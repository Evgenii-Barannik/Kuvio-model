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

use super::*;

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

pub struct WeightedRngDecider;
impl Decider for WeightedRngDecider {
    fn decide(&self, actions: Vec<Action>, data: DecisionMakingData, rng: &mut StdRng) -> Action {
        let weighted_distribution = WeightedIndex::new(&data).unwrap();
        let chosen_index = weighted_distribution.sample(rng);
        actions[chosen_index].clone()
    }
}

struct TrivialTransformer;
impl Transformer for TrivialTransformer  {
    fn transform(&self, _actions: &mut Vec<Action>) {}
}

pub struct AddMintTransformer;
impl Transformer for AddMintTransformer {
    fn transform(&self, actions: &mut Vec<Action>) {
        actions.push(mint_action)
    }
}
pub struct AddWorkTransformer;
impl Transformer for AddWorkTransformer {
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
    pub fn transform(&self, actions: &mut Vec<Action>) {
        match self {
            AnyTransformer::AddMintTransformer => AddMintTransformer.transform(actions),
            AnyTransformer::AddWorkTransformer => AddWorkTransformer.transform(actions),
            AnyTransformer::_TrivialTransformer => TrivialTransformer.transform(actions),
        }
    }
}

pub struct KingdomGameProvider;
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


pub struct FirstPossibleIndicesAssigner;
impl Assigner for FirstPossibleIndicesAssigner {
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

pub struct TrivialInitializer;
impl Initializer for TrivialInitializer {
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

pub struct TrivialPoolProvider;
impl PoolProvider for TrivialPoolProvider {
    fn provide_pool(&self, providers: &Vec<impl GameProvider>, _tick: usize) -> Vec<Game> {
        let mut game_pool: Vec<Game> = vec![];
        for provider in providers {
            game_pool.push(provider.provide_game());
        }
        game_pool
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

//// With these function we pass choosen trait implementing structs to main():
pub fn get_initializer() -> impl Initializer {
    TrivialInitializer
}

pub fn get_decider() -> impl Decider {
    WeightedRngDecider
}

pub fn get_agent_assigner() -> impl Assigner {
    FirstPossibleIndicesAssigner
}

pub fn get_pool_provider() -> impl PoolProvider {
    TrivialPoolProvider
}

pub fn get_game_providers() -> Vec<impl GameProvider> {
    vec![KingdomGameProvider, KingdomGameProvider]
}