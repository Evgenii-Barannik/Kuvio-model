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

use super::{AnyRole, AnyTransformer, AnyResource};

pub type AgentID = usize;
pub type Resources = BTreeMap<AnyResource, usize>;
pub type Action = fn(&mut Agent);
pub type DecisionMakingData = Vec<f64>;
pub type ReputationMatrix = Vec<Vec<f64>>;

pub struct Game  {
    pub role_transformers: BTreeMap<AnyRole, AnyTransformer>,
}

#[derive(Clone, Debug)]
pub struct Agent {
    pub resources: Resources,
    pub actions: Vec<Action>,
    pub _id: AgentID,
}

pub struct Params {
    pub num_of_agents: usize
}

impl Agent {
    pub fn new(initial_resources: Resources, actions: Vec<Action>, _id: AgentID) -> Agent {
        let mut zeroed_resources = AnyResource::iter().map(|r| (r, 0)).collect::<Resources>();
        for (resource, amount) in initial_resources {
            zeroed_resources.insert(resource, amount);
        }
        Agent {resources: zeroed_resources, actions, _id}
    }
}

pub struct Tile {
    pub agents: Vec<Agent>,
    pub _resources: Resources,
    pub _reputations: ReputationMatrix,
}

impl Tile {
    pub fn new(agents: Vec<Agent>, resources: Resources, reputations: Vec<Vec<f64>>) -> Tile {
        let mut zeroed_resources = AnyResource::iter().map(|r| (r, 0)).collect::<Resources>();
        for (resource, amount) in resources {
            zeroed_resources.insert(resource, amount);
        }
        
        Tile{agents, _resources: zeroed_resources, _reputations: reputations}
    }
}


pub trait ActionDecider {
    fn decide(&self, actions: Vec<Action>, data: DecisionMakingData, rng: &mut StdRng) -> Action;
}

pub trait ActionTransformer {
    fn transform(&self, actions: &mut Vec<Action>) -> ();
}

pub trait GameProvider {
    fn provide_game(&self) -> Game;
    fn check_if_roles_are_filled(&self, role_transformers: &BTreeMap<AnyRole, AnyTransformer>) -> ();
}

pub trait GamePoolProvider {
    fn provide_pool(&self, providers: Vec<impl GameProvider>, tick: usize) -> Vec<Game>;
}
pub trait AgentAssigner {
    fn assign_agents(&self, game: &Game, available_agents: &mut Vec<Agent>) -> Option<BTreeMap<AnyRole, AgentID>>;
}

pub trait AgentInitializer {
    fn initialize_agents(&self, params: Params) -> Vec<Agent>;
}
