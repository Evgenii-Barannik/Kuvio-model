type AgentID = usize;

struct Agent {
    resources: BTreeMap<Resource, usize>,
    behavior_probs: BTreeMap<&GameType, Vec<BehaviourProb>>
}

enum Resource {
    Coins,
}

enum GameType {
    OneAgentBasicGame,
    TwoAgentBasicGame,
    TwoAgentDeclinableGame,
}

struct BehaviourProb {
    behaviour: BehaviourFn,
    probability: f64,
}

type BehaviourFn = fn(AgentID, &mut StdRng) -> Result<String, ()>;

enum Role {
    OneAgentBasicGameRole,
    TwoAgentBasicGameRole,
    TwoAgentDeclinableGameRole,
}

type AssignedRoles = BTreeMap<&Role, Vec<AgentID>>;

struct Tile {
    participating_agents: Vec<AgentID>,
    tile_resources: Resources,
}



struct OneAgentBasicGame {
    roles: BTreeMap<&Role, Vec<Behaviours>>
}

type GameProvider = fn() -> Game;