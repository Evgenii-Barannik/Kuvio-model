use std::fs;
use std::time::Instant;
use std::vec;
use std::collections::BTreeMap;
use std::fs::write;
use rand::{rngs::StdRng, SeedableRng};
use strum::IntoEnumIterator;
use plotters::prelude::*;
use rand::prelude::SliceRandom;

mod io;
mod implementation;

use io::*;
use implementation::{AnyResource, AnyRole};
use implementation::{get_initializer, get_pool_provider, get_agent_assigner};

type AgentID = usize;
type Resources = BTreeMap<AnyResource, usize>;
type DecisionAvailableData = BTreeMap<AgentID, Resources>;
type ActionFn = fn(&mut Tile, AgentID, &mut StdRng);
type TransformerFn = fn(Vec<ActionFn>) -> Vec<ActionFn>;
type DeciderFn = fn(&Tile, AgentID, Vec<ActionFn>, &DecisionAvailableData, &mut StdRng) -> ActionFn;
type AgentInitializerFn = fn(&Configs) -> Vec<Agent>;
type PoolProviderFn = fn(&mut Vec<Game>, usize);
type AgentAssignerFn = fn(game: &Game, available_agents: &mut Vec<Agent>) -> Option<BTreeMap<AgentID, AnyRole>>;
type ParticipationCheckerFn = fn(&Agent, &Game, &AnyRole) -> bool;

type ReputationMatrix = Vec<Vec<f64>>;

#[derive(Clone, Debug)]
pub struct Agent {
    resources: Resources,
    base_actions: Vec<ActionFn>,
    participation_checker: ParticipationCheckerFn,
    decider: DeciderFn,
    id: AgentID,
}

#[derive(Clone)]
pub struct Game {
    roles: BTreeMap<AnyRole, RoleDescription>,
    consequent_game: Option<Box<Game>>,
}

#[derive(Clone)]
pub struct RoleDescription {
    uniqueness: AnyUniqueness,
    transformer: TransformerFn,
}

#[derive(Clone, Debug)]
pub struct Tile {
    agents: Vec<Agent>,
    resources: Resources,
    _reputations: ReputationMatrix,
}

#[derive(PartialEq, Clone)]
pub enum AnyUniqueness { // TODO: make other name, see  Modality
    RequiredMultipletRole(usize, usize), // Contains min required and max possible multiplicity. Should be assigned for game to play
    OptionalMultipletRole(usize, usize), // Contains min required and max possible multiplicity. Can be assigned
}

impl Agent {
    fn new(initial_resources: Resources, base_actions: Vec<ActionFn>, decider: DeciderFn, participation_checker: ParticipationCheckerFn, id: AgentID) -> Agent {
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
                total_utility += f64::log10(amount as f64) + 1.0;
                    // We add constant to the resource amount because without it utility of agent with 1 resource will be 0.
                    // This is so because log10(1) == 0.
            }
        }
        total_utility
    }

}


impl Tile {
    fn new(agents: Vec<Agent>, resources: Resources, reputations: Vec<Vec<f64>>) -> Tile {
        let mut zeroed_resources = AnyResource::iter().map(|r| (r, 0)).collect::<Resources>();
        for (resource, amount) in resources {
            zeroed_resources.insert(resource, amount);
        }

        Tile{agents, resources: zeroed_resources, _reputations: reputations}
    }
}

pub trait GameProvider {
    fn provide_game(&self) -> Game;
    fn check_if_all_roles_are_described(&self, roles: &BTreeMap<AnyRole, RoleDescription>) -> Result<(), String>;
}

impl Game {
    fn prepare_actions(&self, assigned_roles: &BTreeMap<AgentID, AnyRole>, ordered_agents: &Vec<Agent>) -> BTreeMap<AgentID, Vec<ActionFn>> {
        let mut transient_actions: BTreeMap<AgentID, Vec<ActionFn>> = BTreeMap::new();

        for (id, role) in assigned_roles.iter() {
            let actions = ordered_agents[*id].base_actions.clone();
            let transformer_fn = self.roles.get(role).unwrap().transformer;
            let transformed_actions = transformer_fn(actions);
            transient_actions.insert(*id, transformed_actions);
        }
        transient_actions
    }

    fn prepare_and_execute_actions(&self, tile: &mut Tile, assigned_roles: &BTreeMap<AgentID, AnyRole>, rng: &mut StdRng) -> () {
        let immutable_ordered_agents = &tile.agents.clone();
        let immutable_tile = &tile.clone();
        let transient_actions = self.prepare_actions(&assigned_roles, immutable_ordered_agents);
        for (agent_id, actions) in transient_actions {
            let decider_fn = &immutable_ordered_agents[agent_id].decider;

            let availiable_data: BTreeMap<AgentID, Resources> = immutable_ordered_agents
                .iter()
                .map(|agent| (agent.id, agent.resources.clone()))
                .collect();

            let choosen_action = decider_fn(immutable_tile, agent_id, actions, &availiable_data, rng);
            choosen_action(tile, agent_id, rng) // Tile is mutated here
        }
    }
    pub fn create_delayed_consequent_game(delay: usize, game: Game) -> Game {
        if delay == 0 {
            return game.clone();
        } else {
            let roles: BTreeMap<AnyRole, RoleDescription> = BTreeMap::new();
            let delayed_game = Game::create_delayed_consequent_game(delay - 1, game);
            let game = Game {roles, consequent_game: Some(Box::new(delayed_game))};
            game
        }
    }

}


fn main() {
    let timer: Instant = Instant::now();

    fs::create_dir_all("output").unwrap();
    let log_file_pathname = format!("output/{}.txt", "final_state");
    let plot_file_pathname = format!("output/{}.gif", "resources_distribution");
    let mut root = BitMapBackend::gif(&plot_file_pathname, (640, 480), 100).unwrap().into_drawing_area();

    let configs = read_configs();
    let pool_provider = get_pool_provider();
    let agent_assigner = get_agent_assigner();
    let reputations = vec![vec![1f64; configs.agent_count]; configs.agent_count];
    let mut rng = StdRng::seed_from_u64(configs.seed as u64);
    let mut tile = Tile::new(get_initializer()(&configs), BTreeMap::new(), reputations);
    let mut games: Vec<Game> = vec![];

    for tick in 0..configs.tick_count {
        let mut consequent_games: Vec<Game> = vec![];
        pool_provider(&mut games, tick);
        games.shuffle(&mut rng);

        let mut transient_consumable_agents = tile.agents.clone();
        // transient_consumable_agents.shuffle(& mut rng);

        for suggested_game in &games{
            let maybe_assigned_agents = agent_assigner(&suggested_game, &mut transient_consumable_agents);
            if let Some(assigned_agents) = maybe_assigned_agents {
                suggested_game.prepare_and_execute_actions(&mut tile, &assigned_agents, &mut rng);
                if let Some(gamebox) = &suggested_game.consequent_game {
                    consequent_games.push(*gamebox.clone()); // If played game had a consequent game, push a consequent game to the pool (will be used for the next tick).
                }
            }
        }
        games.clear();
        games.append(&mut consequent_games);

        if configs.plot_graph && (tick % configs.plotting_frame_subselection_factor) == 0 {
            println!("Plotting frame for tick {}", tick);
            plot_resource_distribution(&tile, &mut root, tick);
        }
    }

    let mut summary_log = String::new();
    summary_log.push_str(&format!("{:#?}\n\n", configs));
    summary_log.push_str(&format!("Tile Resources{:#?}\n\n", tile.resources));
    summary_log.push_str(&format!("Tile Agents {:#?}\n\n", tile.agents));
    write(&log_file_pathname, summary_log).unwrap();

    println!("\nSee final state: {}", log_file_pathname);
    println!("See plot: {}", plot_file_pathname);
    println!("Execution time: {:.3} s", timer.elapsed().as_secs_f64());
}
