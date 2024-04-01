use std::borrow::BorrowMut;
use std::fs;
use std::time::Instant;
use std::vec;
use std::collections::{BTreeMap, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::fs::write;
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

#[derive(PartialEq, PartialOrd, Eq, Ord, Debug, Hash, Clone, EnumIter)]
enum Resource {
    Coins,
}

type AgentID = usize;
type Resources = BTreeMap<Resource, usize>;
type Action = fn(&mut Agent);
type DecisionMakingData = Vec<f64>;

#[derive(Clone)]
struct Agent {
    resources: Resources,
    actions: Vec<Action>,
    _id: AgentID,
}

impl Agent {
    fn new(initial_resources: Resources, actions: Vec<Action>, _id: AgentID) -> Agent {
        let mut zeroed_resources = Resource::iter().map(|r| (r, 0)).collect::<Resources>();
        for (resource, amount) in initial_resources {
            zeroed_resources.insert(resource, amount);
        }

        Agent {resources: zeroed_resources, actions, _id}
    }

}

trait Decider {
    fn decide(&self, actions: Vec<Action>, data: DecisionMakingData, rng: &mut StdRng) -> Action;
}

struct WeightedRngDecider;
impl Decider for WeightedRngDecider {
    fn decide(&self, actions: Vec<Action>, data: DecisionMakingData, rng: &mut StdRng) -> Action {
        let weighted_distribution = WeightedIndex::new(&data).unwrap();
        let chosen_index = weighted_distribution.sample(rng);
        actions[chosen_index].clone()
    }
}

fn pass_action(_agent: &mut Agent) {}

fn mint_action(agent: &mut Agent) {
    *agent.resources.entry(Resource::Coins).or_insert(0) += 10;
}

fn work_action(agent: &mut Agent) {
    *agent.resources.entry(Resource::Coins).or_insert(0) += 1;
}

trait Transformer {
    fn transform(&self, actions: &mut Vec<Action>) -> ();
}


struct TrivialTransformer;
impl Transformer for TrivialTransformer  {
    fn transform(&self, _actions: &mut Vec<Action>) {}
}

struct AddMintTransformer;
impl Transformer for AddMintTransformer {
    fn transform(&self, actions: &mut Vec<Action>) {
        actions.push(mint_action)
    }

}
struct AddWorkTransformer;
impl Transformer for AddWorkTransformer {
    fn transform(&self, actions: &mut Vec<Action>) {
        actions.push(work_action)
    }
}

enum AnyTransformer {
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

trait GameProvider {
    fn provide_game(&self) -> Game;
}

struct KingdomGameProvider;
impl GameProvider for KingdomGameProvider {
    fn provide_game(&self) -> Game {
        let mut role_transformers = BTreeMap::new();
        role_transformers.insert(AnyRole::KingdomRole(KingdomRole::King), AnyTransformer::AddMintTransformer);
        role_transformers.insert(AnyRole::KingdomRole(KingdomRole::Peasant), AnyTransformer::AddWorkTransformer);
        Game { role_transformers }
    }
}

trait AgentAssigner {
    fn assign_agents(&self, game: &Game, agents: &Vec<Agent>) -> BTreeMap<AnyRole, AgentID>;
}

struct TrivialAssigner;
impl AgentAssigner for TrivialAssigner {
    fn assign_agents(&self, game: &Game, agents: &Vec<Agent>) -> BTreeMap<AnyRole, AgentID> {
        let mut assigned_agents: BTreeMap<AnyRole, AgentID> = BTreeMap::new();
        for (role, index) in game.role_transformers.keys().zip(0..agents.len()) {
            let agent_id = agents[index]._id;
            assigned_agents.insert(role.to_owned(), agent_id);
        }
        
        assigned_agents
    }
}

struct Game  {
    role_transformers: BTreeMap<AnyRole, AnyTransformer>,
}

impl Game {
    fn prepare(&self, assigned_roles: &BTreeMap<AnyRole, AgentID>, ordered_agents: &Vec<Agent>) -> BTreeMap<AgentID, Vec<Action>> {
        let mut all_transformed_actions: BTreeMap<AgentID, Vec<Action>> = BTreeMap::new(); 
        
        for (assigned_role, agent_id) in assigned_roles.iter() {
            if let Some(action_transformer) = self.role_transformers.get(assigned_role) {
                let mut cloned_actions = ordered_agents[*agent_id].actions.clone();
                action_transformer.transform(&mut cloned_actions);
                all_transformed_actions.insert(*agent_id, cloned_actions);
            } 
        }
        all_transformed_actions
    }

    fn prepare_and_execute(&self, assigned_roles: &BTreeMap<AnyRole, AgentID>, ordered_agents: &mut Vec<Agent>, rng: &mut StdRng) -> () {        
        let nm_ordered_agents = &*ordered_agents; // Without mutability
        let new_actions = self.prepare(&assigned_roles, nm_ordered_agents);
        for (agent_id, actions) in new_actions {
            let decider_data = generate_normalized_vector(rng, actions.len());
            let choosen_action = WeightedRngDecider.decide(actions, decider_data, rng); 
            choosen_action(&mut ordered_agents[agent_id]); 
        } 
    }

}

type ReputationMatrix = Vec<Vec<f64>>;

struct Tile {
    agents: Vec<Agent>,
    _resources: Resources,
    _reputations: ReputationMatrix,
}

impl Tile {
    fn new(agents: Vec<Agent>, resources: Resources, reputations: Vec<Vec<f64>>) -> Tile {
        let mut zeroed_resources = Resource::iter().map(|r| (r, 0)).collect::<Resources>();
        for (resource, amount) in resources {
            zeroed_resources.insert(resource, amount);
        }
        
        Tile{agents, _resources: zeroed_resources, _reputations: reputations}
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
    Peasant,
}

fn generate_normalized_vector(rng: &mut StdRng, n: usize) -> Vec<f64> {
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
    let seed: usize = 2;

    let reputations = vec![vec![1f64; num_of_agents]; num_of_agents];
    let mut rng = StdRng::seed_from_u64(seed as u64);
    let mut tile = Tile::new(vec![], BTreeMap::new(), reputations);
    
    for i in 0..num_of_agents {
        let agent = Agent::new(
            BTreeMap::new(),
            [pass_action as Action; 1].to_vec(), // Do nothing action
            i as AgentID,
        );
        tile.agents.push(agent);
    }
    
    for tick in 0..num_of_ticks {
        let mut games: Vec<Game> = vec![];
        games.push(KingdomGameProvider.provide_game());
        
        let shuffled_agents = {
            let mut agents = tile.agents.clone();
            agents.shuffle(&mut rng);
            agents
        };

        for game in games {
            let assigned_agents = TrivialAssigner.assign_agents(&game, &shuffled_agents);
            game.prepare_and_execute(&assigned_agents, &mut tile.agents, &mut rng)
       }

       let mut summary_log = String::new();
       log_resources(&tile.agents, &mut summary_log);
       let log_file_pathname = format!("output/{}.txt", "Test");
       write(&log_file_pathname, summary_log).unwrap();
    }

    println!("Execution time: {:.3} s", timer.elapsed().as_secs_f64());
}
