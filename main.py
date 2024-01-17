from typing import Optional, Dict
from dataclasses import dataclass, field
from math import log

UTILITY_SHIFT = 1

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
            for name, new_resource_amount in changed_resources.items():
                current_resource_amount = self._resources.get(name, 0)
                
                changes_string = "Resource changes: {"
                if current_resource_amount != new_resource_amount:
                    self._resources[name] = new_resource_amount
                    changes_string = changes_string + "{}: {} -> {}".format(name, current_resource_amount, new_resource_amount)
                changes_string = changes_string + " }\n"
                print(changes_string)

    def shift(self, name: str, delta: int) -> None:
        current_resource_amount = self._resources.get(name, 0)
        new_resource_amount = current_resource_amount + delta
        self._resources[name] = new_resource_amount
        print("Resource changes: {{ {}: {} -> {} }}\n".format(name, current_resource_amount, new_resource_amount))

    # We add UTILITY_SHIFT because without it resource change 0 -> 1 will not change utility value.
    # It is so because log(1) == 0.0.
    def utility(self) -> float:
        total_utility = 0.0
        for resource_name, amount in self._resources.items():
            total_utility += (log(amount) + UTILITY_SHIFT if amount > 0 else 0.0) 
        return total_utility


if __name__ == "__main__": 
    test_resources = Resources({
        "gold": 5,
        # "wood": 0,
        # "ore": 0,
        # "mercury": 0,
        # "sulfur": 0,
        # "crystal": 0,
        # "gem": 0,
    })
    print(test_resources)

    test_resources.shift("gold", 10)
    print(test_resources)

    test_resources.update({"gems": 1, "mercury": 1})
    print(test_resources)
    