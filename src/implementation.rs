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

fn pass_action(_agent: &mut Agent) {} // Action that does nothing

fn mint_action(agent: &mut Agent) { 
    *agent.resources.entry(AnyResource::Coins).or_insert(0) += 10;
}

fn work_action(agent: &mut Agent) { 
    *agent.resources.entry(AnyResource::Coins).or_insert(0) += 1;
}

impl AnyActionIntoInner for AnyAction {
    fn into_inner(self) -> Action {
        match self {
            AnyAction::pass_action => pass_action,
            AnyAction::mint_action => mint_action,
            AnyAction::work_action => work_action,
        }
    }
}

fn generate_normalized_vector(rng: &mut StdRng, n: usize) -> Vec<f64> {
    let vec: Vec<f64> = rng.sample_iter(Uniform::new(0.0, 1.0)).take(n).collect();
    let sum: f64 = vec.iter().sum();
    vec.into_iter().map(|x| x / sum).collect()
}

struct WeightedRngDecider;
impl Decider for WeightedRngDecider {
    fn decide(&self, actors_actions: Vec<AnyAction>, _data: &AvailableData, rng: &mut StdRng) -> AnyAction {
        let normalized_vector = generate_normalized_vector(rng, actors_actions.len());
        let weighted_distribution = WeightedIndex::new(normalized_vector).unwrap();
        let chosen_index = weighted_distribution.sample(rng);
        actors_actions[chosen_index].clone()
    }
}

impl Decider for AnyDecider {
    fn decide(&self, actors_actions: Vec<AnyAction>, data: &AvailableData, rng: &mut StdRng) -> AnyAction {
        match self {
            AnyDecider::WeightedRngDecider => {
                WeightedRngDecider.decide(actors_actions, data, rng)
            }
        }
    }
}

struct TrivialTransformer;
impl Transformer for TrivialTransformer  {
    fn transform(&self, _actions: &mut Vec<AnyAction>) {}
}

struct AddMintTransformer;
impl Transformer for AddMintTransformer {
    fn transform(&self, actions: &mut Vec<AnyAction>) {
        actions.push(AnyAction::mint_action)
    }
}
struct AddWorkTransformer;
impl Transformer for AddWorkTransformer {
    fn transform(&self, actions: &mut Vec<AnyAction>) {
        actions.push(AnyAction::work_action)
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
impl Assigner for FirstPossibleIndicesAssigner {
    fn assign_and_consume_agents(&self, game: &Game, available_agents: &mut Vec<Agent>) -> Option<BTreeMap<AnyRole, AgentID>> {
        let mut assigned_agents: BTreeMap<AnyRole, AgentID> = BTreeMap::new();
        let mut agent_ids_to_consume: Vec<AgentID> = vec![];
        
        for (role, agent) in game.role_transformers.keys().zip(&*available_agents) {
            assigned_agents.insert(role.to_owned(), agent.id);
            agent_ids_to_consume.push(agent.id);
        }
        
        if assigned_agents.len() != game.role_transformers.len() {
            return None; 
        } else {
            available_agents.retain(|agent| !agent_ids_to_consume.contains(&agent.id));
            Some(assigned_agents)
        }
    }
}

struct TrivialInitializer;
impl Initializer for TrivialInitializer {
    fn initialize_agents(&self, configs: &Configs) -> (Vec<Agent>, BTreeMap<AgentID, AnyDecider>) {
        let mut agents = vec![];
        let mut deciders: BTreeMap<AgentID, AnyDecider> = BTreeMap::new();

        for i in 0..configs.agent_count {
            agents.push(
                Agent::new(
                    BTreeMap::new(),
                vec![AnyAction::pass_action],
                i as AgentID,
                )
            );
            deciders.insert(i as AgentID, AnyDecider::WeightedRngDecider);
        }
        (agents, deciders)
    }
}

struct TrivialPoolProvider;
impl PoolProvider for TrivialPoolProvider {
    fn provide_pool(&self, providers: &Vec<impl GameProvider>, _tick: usize) -> Vec<Game> {
        let mut game_pool: Vec<Game> = vec![];
        for provider in providers {
            game_pool.push(provider.provide_game());
        }
        game_pool
    }    
}    


/// Public interface
/// Use get_* functions to pass trait-implementing-structs to the main fn.

#[derive(PartialEq, PartialOrd, Eq, Ord, Debug, Hash, Clone, EnumIter)]
pub enum AnyResource { 
    Coins,
}    

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum AnyRole { 
    KingdomRole(KingdomRole),
}    

#[allow(non_camel_case_types)]
#[derive(Clone, Debug)]
pub enum AnyAction {
    pass_action,
    mint_action,
    work_action,
}    

pub enum AnyDecider {
    WeightedRngDecider
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, EnumIter, Debug)]
pub enum KingdomRole {
    King,
    Peasant1,
    Peasant2,
}
    
pub enum AnyTransformer { 
    AddMintTransformer,
    AddWorkTransformer,
    _TrivialTransformer,
}

impl AnyTransformer {
    pub fn transform(&self, actions: &mut Vec<AnyAction>) {
        match self {
            AnyTransformer::AddMintTransformer => AddMintTransformer.transform(actions),
            AnyTransformer::AddWorkTransformer => AddWorkTransformer.transform(actions),
            AnyTransformer::_TrivialTransformer => TrivialTransformer.transform(actions),
        }
    }
}

pub fn get_initializer() -> impl Initializer {
    TrivialInitializer
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

