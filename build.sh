#!/bin/bash

if ! mypy main.py --strict; then
    exit 1
else
    python main.py
fi

