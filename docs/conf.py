import os
import sys

project = "stricc"
author = "Diego J. Romero Lopez"
release = "0.1.0"

extensions = [
    "myst_parser",
    "sphinx.ext.viewcode",
]

myst_enable_extensions = [
    "colon_fence",
]

templates_path = ["_templates"]
exclude_patterns = []

html_theme = "sphinx_rtd_theme"
html_static_path = ["_static"]
