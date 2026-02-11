#!/usr/bin/env python3
"""Module docstring."""

import json

class Greeter:
    def __init__(self, name: str):
        self.name = name

    def greet(self) -> str:
        return f"Hello, {self.name}"


def main():
    data = {"count": 3, "items": [1, 2, 3]}
    for i in range(data["count"]):
        print(i)


    # comment
    if data["items"]:
        print(Greeter("world").greet())

    print(json.dumps(data))


if __name__ == "__main__":
    main()
