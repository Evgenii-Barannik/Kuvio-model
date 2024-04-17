use std::any::Any;
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
use lazy_static::lazy_static;
use std::any::TypeId;

use super::*;

struct RngDecider;
impl ActionDecider for RngDecider {
    fn decide(&self, _tile: &Tile, _agent_id: AgentID, transient_actions: Vec<AnyAction>, _data: &DecisionAvailableData, rng: &mut StdRng) -> AnyAction {
        let random_index = Uniform::new(0, transient_actions.len()).sample(rng);
        transient_actions[random_index].clone()
    }
}
struct UtilityComputingDecider;
impl ActionDecider for UtilityComputingDecider {
    fn decide(&self, tile: &Tile, agent_id: AgentID, transient_actions: Vec<AnyAction>, _data: &DecisionAvailableData, _rng: &mut StdRng) -> AnyAction {
        let possible_future_utilities = transient_actions.iter()
            .map(|action| (*action).clone().into_inner())
            .map(|f| { 
                let mut tile_clone = tile.clone();
                f(&mut tile_clone, agent_id);
                tile.agents[agent_id].get_utility()
            } )
            .collect::<Vec<f64>>();
            
        let choosen_index = possible_future_utilities.iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Less))
            .map(|(index, _)| index)
            .unwrap();

        transient_actions[choosen_index].clone()
    }   
}

impl ActionDecider for AnyDecider {
    fn decide(&self, tile: &Tile, agent_id: AgentID, transient_actions: Vec<AnyAction>, data: &DecisionAvailableData, rng: &mut StdRng) -> AnyAction {
        match self {
            AnyDecider::RngDecider => {
                RngDecider.decide(tile, agent_id, transient_actions, data, rng)
            }
            AnyDecider::UtilityComputingDecider => {
                UtilityComputingDecider.decide (tile, agent_id, transient_actions, data, rng)
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

struct FirstAvailableAgentAssigner;
impl AgentAssigner for FirstAvailableAgentAssigner {
    fn assign_and_consume_agents(&self, game: &Game, available_agents: &mut Vec<Agent>) -> Option<BTreeMap<AgentID, AnyRole>> {
        let mut assigned_agents: BTreeMap<AgentID, AnyRole> = BTreeMap::new();
        let all_roles = game.roles.clone().into_iter()
        .map(|(role, description)| {
            match description.uniqueness {
                AnyUniqueness::RequiredMultipletRole(min, max) => 
                (AnyUniqueness::RequiredMultipletRole.type_id(), role as AnyRole, min, max),
                AnyUniqueness::OptionalMultipletRole(min, max) => 
                (AnyUniqueness::OptionalMultipletRole.type_id(), role as AnyRole, min, max)
            }
        })
        .collect::<Vec<(TypeId, AnyRole, usize, usize)>>();
    
    for (typeid, role, min_multiplicity, max_multiplicity) in all_roles.iter() {
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
                if typeid == &AnyUniqueness::RequiredMultipletRole.type_id() {
                    return None; // Assignment to one required role failed, so the game will not be played.
                }
            };
        } 
        
        Some(assigned_agents)

    }
}


// How to add new action to some Game:
// 1) Write new action function;
// 2) Add this action function to the AnyAction enum;
// 3) Change AnyAction trait implementation to describe new variant;
// 4) Add new or change one of the existing Transformers. Or add your new action to the agent initialization as one of the base actions (see impl AgentInitializer of some structs).
// 4B) If you choose to create new Transformer implementing struct, add it to the AnyTransformer enum and change AnyTransformer traits implementation.
// 4C) Use your Transformer inside Game that is created by method of the some GameProvider implementing struct.

fn trivial_action(tile: &mut Tile, agent_id: AgentID) {} // Action that does nothing

fn mint_action(tile: &mut Tile, agent_id: AgentID) { 
    *tile.agents[agent_id].resources.entry(AnyResource::Coins).or_insert(0) += 10;
}

fn work_action(tile: &mut Tile, agent_id: AgentID) { 
    *tile.agents[agent_id].resources.entry(AnyResource::Coins).or_insert(0) += 1;
}

fn remove_coins_action (tile: &mut Tile, agent_id: AgentID) {
    if *tile.agents[agent_id].resources.entry(AnyResource::Coins).or_insert(0) >= 1 {
        *tile.agents[agent_id].resources.entry(AnyResource::Coins).or_insert(0) -= 1;
    }
}

impl AnyActionIntoInner for AnyAction {
    fn into_inner(self) -> Action {
        match self {
            AnyAction::trivial_action => trivial_action,
            AnyAction::mint_action => mint_action,
            AnyAction::work_action => work_action,
            AnyAction::remove_coins_action => remove_coins_action
        }
    }
}

struct TrivialTransformer;
impl ActionTransformer for TrivialTransformer  {
    fn transform(&self, _actions: &mut Vec<AnyAction>) {}
}

struct AddMintTransformer;
impl ActionTransformer for AddMintTransformer {
    fn transform(&self, actions: &mut Vec<AnyAction>) {
        actions.push(AnyAction::mint_action)
    }
}
struct AddWorkTransformer;
impl ActionTransformer for AddWorkTransformer {
    fn transform(&self, actions: &mut Vec<AnyAction>) {
        actions.push(AnyAction::work_action)
    }
}

struct RemoveCoinsTransformer;
impl ActionTransformer for RemoveCoinsTransformer {
    fn transform(&self, actions: &mut Vec<AnyAction>) {
        actions.clear();
        actions.push(AnyAction::remove_coins_action);
    }
}

lazy_static! {
    static ref THE_END_GAME: Game = {
        let role = AnyRole::TheEndRole(TheEndRole::Anyone);
        let description = RoleDescription {
            uniqueness: AnyUniqueness::RequiredMultipletRole(1, usize::MAX),
            transformer: AnyTransformer::RemoveCoinsTransformer,
        };

        Game {
            roles: BTreeMap::from([(role, description)]),
            consequent_game: None,
        }
    };

    static ref KINGDOM_GAME: Game = {
        let mut roles = BTreeMap::new();
        roles.insert(
            AnyRole::KingdomRole(KingdomRole::King), 
            RoleDescription {
                uniqueness: AnyUniqueness::RequiredMultipletRole(1usize, 1usize), 
                transformer: AnyTransformer::AddMintTransformer,
            }
        );

        roles.insert(
            AnyRole::KingdomRole(KingdomRole::Peasant), 
            RoleDescription {
                uniqueness: AnyUniqueness::OptionalMultipletRole(0usize, usize::MAX), 
                transformer: AnyTransformer::AddWorkTransformer,
            }
        );

        let consequent_game = Some(Box::from(Game::create_delayed_consequent_game(30, THE_END_GAME.clone())));
        Game {roles, consequent_game}
    };
}

#[derive(Clone)]
struct KingdomGameProvider;
impl GameProvider for KingdomGameProvider {
    fn provide_game(&self) -> Game {
        let game = KINGDOM_GAME.clone();
        self.check_if_all_roles_are_described(&game.roles); // TODO: Move checks outside impl, check consequent games.
        game
    }

    fn check_if_all_roles_are_described(&self, roles: &BTreeMap<AnyRole, RoleDescription>) -> () {
        for role in KingdomRole::iter() { 
            if !roles.contains_key(&AnyRole::KingdomRole(role.clone())) {
                panic!("No description (uniqueness and transformer) for this role: {:?}", &role);
            }
        } 
    }
}

struct BasicInitializer;
impl AgentInitializer for BasicInitializer {
    fn initialize_agents(&self, configs: &Configs) -> Vec<Agent> {
        let mut agents = vec![];

        let mid_index = configs.agent_count.div_ceil(2);
        for i in 0..configs.agent_count {
            let decider = if i < mid_index {
                AnyDecider::RngDecider
            } else {
                AnyDecider::UtilityComputingDecider
            };

            agents.push(
                Agent::new(
                    BTreeMap::new(),
                    vec![AnyAction::trivial_action],
                    decider,
                    AnyParticipationChecker::TrivialParticipationChecker,
                    i as AgentID,
                )
            );
        }

        agents

    }
}

struct KingdomPoolProvider;
impl PoolProvider for KingdomPoolProvider {
    fn provide_all_games(&self, gamepool: &mut Vec<Game>, _tick: usize) -> () {
        gamepool.push(KingdomGameProvider.provide_game());
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
    trivial_action,
    mint_action,
    work_action,
    remove_coins_action,
}    

#[derive(Clone, Debug)]
pub enum AnyDecider {
    RngDecider,
    UtilityComputingDecider 
}

#[derive(Clone, Debug)]
pub enum AnyParticipationChecker {
    TrivialParticipationChecker
}

#[derive(Clone)]
pub enum AnyTransformer { 
    AddMintTransformer,
    AddWorkTransformer,
    RemoveCoinsTransformer,
    _TrivialTransformer,

}

impl AnyTransformer {
    pub fn transform(&self, actions: &mut Vec<AnyAction>) {
        match self {
            AnyTransformer::AddMintTransformer => AddMintTransformer.transform(actions),
            AnyTransformer::AddWorkTransformer => AddWorkTransformer.transform(actions),
            AnyTransformer::_TrivialTransformer => TrivialTransformer.transform(actions),
            AnyTransformer::RemoveCoinsTransformer => RemoveCoinsTransformer.transform(actions),
        }
    }
}

pub fn get_initializer() -> impl AgentInitializer {
    BasicInitializer
}

pub fn get_agent_assigner() -> impl AgentAssigner {
    FirstAvailableAgentAssigner
}

pub fn get_pool_provider() -> impl PoolProvider {
    KingdomPoolProvider
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum AnyRole {
    KingdomRole(KingdomRole),
    TheEndRole(TheEndRole),
}    

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, EnumIter, Debug)]
pub enum KingdomRole {
    King,
    Peasant,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, EnumIter, Debug)]
pub enum TheEndRole {
    Anyone
}