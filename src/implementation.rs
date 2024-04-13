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
    fn decide(&self, actors_actions: Vec<AnyAction>, _data: &DecisionAvailableData, rng: &mut StdRng) -> AnyAction {
        // Rng usage here is superfluous, it is for demonstration.
        let random_normalized_vector = generate_normalized_vector(rng, actors_actions.len()); 
        let weighted_distribution = WeightedIndex::new(random_normalized_vector).unwrap();
        let chosen_index = weighted_distribution.sample(rng);
        actors_actions[chosen_index].clone()
    }
}

impl Decider for AnyDecider {
    fn decide(&self, actors_actions: Vec<AnyAction>, data: &DecisionAvailableData, rng: &mut StdRng) -> AnyAction {
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
        let mut roles = BTreeMap::new();

        roles.insert(
            AnyRole::KingdomRole(KingdomRole::King), 
            RoleDescription {
                uniqueness: AnyUniqueness::RequiredMultipletRole(1usize, 2usize), // Contains min required and max possible multiplicity 
                transformer: AnyTransformer::AddMintTransformer,
            }
        );

        roles.insert(
            AnyRole::KingdomRole(KingdomRole::Peasant), 
            RoleDescription {
                uniqueness: AnyUniqueness::OptionalMultipletRole(0usize, usize::MAX), // Contains min required and max possible multiplicity
                transformer: AnyTransformer::AddWorkTransformer,
            }
        );
        
        self.check_if_all_roles_are_described(&roles);
        Game { roles }
    }

    fn check_if_all_roles_are_described(&self, roles: &BTreeMap<AnyRole, RoleDescription>) -> () {
        for role in KingdomRole::iter() { 
            if !roles.contains_key(&AnyRole::KingdomRole(role.clone())) {
                panic!("No description (uniqueness and transformer) for this role: {:?}", &role);
            }
        } 
    }
}

struct TrivialParticipationChecker;
impl ParticipationChecker for TrivialParticipationChecker {
    fn check_if_agent_participates(&self, _agent: &Agent, _game: &Game, _proposed_role: &AnyRole) -> bool {
        true
    }
} 

impl ParticipationChecker for AnyParticipationChecker {
    fn check_if_agent_participates(&self, agent: &Agent, game: &Game, _proposed_role: &AnyRole) -> bool {
        match self {
            &AnyParticipationChecker::TrivialParticipationChecker => {
                TrivialParticipationChecker.check_if_agent_participates(agent, game, _proposed_role)
            }
        }
    }
}

struct FirstAvailableAgentsAssigner;
impl Assigner for FirstAvailableAgentsAssigner {
    fn assign_and_consume_agents(&self, game: &Game, available_agents: &mut Vec<Agent>) -> Option<BTreeMap<AgentID, AnyRole>> {
        let mut assigned_agents: BTreeMap<AgentID, AnyRole> = BTreeMap::new();
        
        // Required roles assignments
        let required_roles = game.roles.clone().into_iter()
        .filter_map(|(k, v)| {
            if let AnyUniqueness::RequiredMultipletRole(min_multiplicity, max_multiplicity) = v.uniqueness {
                Some((k as AnyRole, min_multiplicity, max_multiplicity))
            } else { None }
        })
        .collect::<Vec<(AnyRole, usize, usize)>>();
    
    for (role, min_multiplicity, max_multiplicity) in required_roles.iter() {
        assert!(*max_multiplicity > 0usize); // TODO: Move to the init phase?
        assert!(max_multiplicity >= min_multiplicity); // TODO: Move to the init phase?
        let mut multiplicity_remaining = max_multiplicity.clone();
        let mut agents_to_consume: Vec<AgentID> = vec![];
        let mut suggested_agents: BTreeMap<AgentID, AnyRole> = BTreeMap::new();

            'agent_loop: for agent in available_agents.iter() {
                if agent.participation_checker.check_if_agent_participates(agent, game, role) {
                    suggested_agents.insert(agent.id, role.to_owned());
                    agents_to_consume.push(agent.id);

                    multiplicity_remaining -= 1;
                    if multiplicity_remaining == 0 {
                        break 'agent_loop
                    }
                }
            }
            if agents_to_consume.len() >= *min_multiplicity {
                available_agents.retain(|agent| !agents_to_consume.contains(&agent.id));
                assigned_agents.append(&mut suggested_agents);
            } else {
                return None; // Assignment to one required role failed, so the game will not be played.
            };
        } 
        
        // Optional roles assignments
        let optional_roles = game.roles.clone().into_iter()
        .filter_map(|(k, v)| {
            if let AnyUniqueness::OptionalMultipletRole(min_multiplicity, max_multiplicity) = v.uniqueness {
                Some((k as AnyRole, min_multiplicity, max_multiplicity))
            } else { None }
        })
        .collect::<Vec<(AnyRole, usize, usize)>>();
    
    for (role, min_multiplicity, max_multiplicity) in optional_roles.iter() {
        assert!(*max_multiplicity > 0usize); // TODO: Move to the init phase?
        assert!(max_multiplicity >= min_multiplicity); // TODO: Move to the init phase?
        let mut multiplicity_remaining = max_multiplicity.clone();
        let mut agents_to_consume: Vec<AgentID> = vec![];
        let mut suggested_agents: BTreeMap<AgentID, AnyRole> = BTreeMap::new();

            'agent_loop: for agent in available_agents.iter() {
                if agent.participation_checker.check_if_agent_participates(agent, game, role) {
                    suggested_agents.insert(agent.id, role.to_owned());
                    agents_to_consume.push(agent.id);

                    multiplicity_remaining -= 1;
                    if multiplicity_remaining == 0 {
                        break 'agent_loop
                    }
                }
            }
            if agents_to_consume.len() >= *min_multiplicity {
                available_agents.retain(|agent| !agents_to_consume.contains(&agent.id));
                assigned_agents.append(&mut suggested_agents);
            }
        } 
        Some(assigned_agents)

    }
}

struct TrivialInitializer;
impl Initializer for TrivialInitializer {
    fn initialize_agents(&self, configs: &Configs) -> Vec<Agent> {
        let mut agents = vec![];

        for i in 0..configs.agent_count {
            agents.push(
                Agent::new(
                BTreeMap::new(),
                vec![AnyAction::pass_action],
                AnyDecider::WeightedRngDecider,
                AnyParticipationChecker::TrivialParticipationChecker,
                i as AgentID,
                )
            );
        }
        agents
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

#[allow(non_camel_case_types)]
#[derive(Clone, Debug)]
pub enum AnyAction { 
    pass_action,
    mint_action,
    work_action,
}    

#[derive(Clone, Debug)]
pub enum AnyDecider {
    WeightedRngDecider
}

#[derive(Clone, Debug)]
pub enum AnyParticipationChecker {
    TrivialParticipationChecker
}

#[derive(Clone)]
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
    FirstAvailableAgentsAssigner
}

pub fn get_pool_provider() -> impl PoolProvider {
    TrivialPoolProvider
}

pub fn get_game_providers() -> Vec<impl GameProvider> {
    vec![KingdomGameProvider]
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum AnyRole { 
    KingdomRole(KingdomRole),
}    

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, EnumIter, Debug)]
pub enum KingdomRole {
    King,
    Peasant,
}