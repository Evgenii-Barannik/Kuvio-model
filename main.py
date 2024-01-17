from typing import Optional, Dict
from dataclasses import dataclass, field
from math import log

RESOURCE_BIAS = 1

RESOURCE_WEIGHTS = {
    "gold": 1,
    "wood": 1,
    "ore": 1,
    "mercury": 5,
    "sulfur": 5,
    "crystal": 5,
    "gem": 5,
}

@dataclass
class Resources:
    _resources: Dict[str, int] = field(default_factory=dict)

    def __init__(self, initial_resources: Optional[Dict[str, int]] = None) -> None:
        if initial_resources is not None:
            self._resources = initial_resources

    def __getattr__(self, name: str) -> int:
        return self._resources.get(name, 0) # Second argument is the default value

    def __str__(self) -> str:
        resource_strings = ["{}: {}".format(name, amount) for name, amount in self._resources.items()]
        return "Resources: {{ {} }}\nUtility: {:.4}\n".format(
            ", ".join(resource_strings),
            self.utility()
        )

    def update(self, changed_resources:  Optional[Dict[str, int]] = None) -> None:
        if changed_resources is not None:
            changes_strings = []
            for name, new_resource_amount in changed_resources.items():
                current_resource_amount = self._resources.get(name, 0)
                
                if current_resource_amount != new_resource_amount:
                    self._resources[name] = new_resource_amount
                    changes_strings.append("{}: {} -> {}".format(name, current_resource_amount, new_resource_amount))
            print("Resources changed: {{ {} }}\n".format(", ".join(changes_strings)))

    def shift(self, name: str, delta: int) -> None:
        current_resource_amount = self._resources.get(name, 0)
        new_resource_amount = current_resource_amount + delta
        self._resources[name] = new_resource_amount
        print("Resources changed: {{ {}: {} -> {} }}\n".format(name, current_resource_amount, new_resource_amount))

    # We add RESOURCE_BIAS because without it resource change 0 -> 1 will not change utility.
    # It is so because log(1) == 0.0.
    def utility(self) -> float:
        total_utility = 0.0
        for resource_name, resource_amount in self._resources.items():
            resource_weight = RESOURCE_WEIGHTS.get(resource_name, 1)
            if resource_amount > 0:
                total_utility += (resource_weight*(log(resource_amount) + RESOURCE_BIAS))
        return total_utility


if __name__ == "__main__": 
    test_resources = Resources({
        "gold": 1,
        # "wood": 1,
        # "ore": 3,
        # "mercury": 0,
        # "sulfur": 0,
        # "crystal": 0,
        # "gem": 0,
    })
    print(test_resources)

    test_resources.shift("gold", 9)
    print(test_resources)

    test_resources.update({"gem": 1, "mercury": 1})
    print(test_resources)
    