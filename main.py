from typing import Optional, Dict
from dataclasses import dataclass, field

@dataclass
class Resources:
    _resources: Dict[str, int] = field(default_factory=dict)

    def __init__(self, initial_resources: Optional[Dict[str, int]] = None):
        if initial_resources is not None:
            self._resources = initial_resources

    def __getattr__(self, name: str) -> int:
        return self._resources.get(name, 0) # Second argument is the default value

    def update(self, changed_resources:  Optional[Dict[str, int]] = None):
        if changed_resources is not None:
            for name, new_resource_amount in changed_resources.items():
                current_resource_amount = self._resources.get(name, 0)
                if current_resource_amount != new_resource_amount:
                    self._resources[name] = new_resource_amount
                    print("{}: {} -> {}".format(name, current_resource_amount, new_resource_amount))

    def shift(self, name: str, delta: int):
        current_resource_amount = self._resources.get(name, 0)
        new_resource_amount = current_resource_amount + delta
        self._resources[name] = new_resource_amount
        print("{}: {} -> {}".format(name, current_resource_amount, new_resource_amount))

if __name__ == "__main__": 
    test_resources = Resources({
        "gold": 4,
        "wood": 0,
        "ore": 0,
        "mercury": 0,
        "sulfur": 0,
        "crystal": 0,
    })

    test_resources.shift("gold", 10)
    test_resources.update({"gems": 2, "mercury": 5})
    test_resources.update({"gems": 2, "mercury": 4})